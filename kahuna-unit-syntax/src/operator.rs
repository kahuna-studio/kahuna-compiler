//! Value operators: pure functions on single values, e.g. `fn multiply(a: f32, b: f32) -> f32`.
//!
//! Every parameter is an input port and the return value is the output port `out`. The compiler
//! calls the function once per sample; whether a port carries a signal or a parameter follows from
//! what is connected, so it is never declared here.

use syn::{
    Attribute, Expr, FnArg, GenericParam, Ident, ItemFn, LitStr, Pat, PatType, ReturnType, Type, TypeReference,
};

pub(crate) use crate::model::declaration::operator_declaration::OperatorDeclaration;
pub(crate) use crate::model::declaration::operator_port::{OperatorPort, OperatorPortType};
use crate::{Direction, LiteralValue, UnitAttributes, UnitKind, literal_value};

/// The name of a value operator's output port.
pub const OUTPUT_PORT: &str = "out";

/// The minimum number of connections of a folded port: folding a single input would be a no-op.
const FOLD_MIN_CONNECTIONS: usize = 2;

const SUPPORTED_TYPES: &str = "f32, bool, a type parameter of the function, or a slice `&[_]` of these";

pub fn parse_value_operator(function: &ItemFn, attributes: &UnitAttributes) -> syn::Result<OperatorDeclaration> {
    let signature = &function.sig;
    let type_parameters = type_parameters(function)?;

    let output_type = match &signature.output {
        ReturnType::Type(_, output_type) => port_type(output_type, &type_parameters)?,
        ReturnType::Default => {
            return Err(syn::Error::new_spanned(&signature.ident, "a value operator must return its result"));
        }
    };

    let mut ports = match &attributes.fold {
        Some(fold_port) => vec![folded_input(function, fold_port, &output_type, &type_parameters)?],
        None => signature
            .inputs
            .iter()
            .map(|argument| input_port(argument, &type_parameters))
            .collect::<syn::Result<Vec<_>>>()?,
    };

    if let OperatorPortType::Variable(variable) = &output_type
        && !ports.iter().any(|port| port.port_type == output_type)
    {
        return Err(syn::Error::new_spanned(
            variable,
            "the output's type parameter must also be the type of an input, so it can follow what is connected",
        ));
    }

    ports.push(OperatorPort {
        direction: Direction::Output,
        identifier: Ident::new(OUTPUT_PORT, signature.ident.span()),
        port_type: output_type,
        min_connections: None,
        default_value: None,
        range: None,
    });

    let id = crate::unit_id(attributes, &signature.ident)?;

    Ok(OperatorDeclaration {
        kind: UnitKind::ValueOperator,
        id,
        type_parameters,
        ports,
        fold: attributes.fold.is_some(),
    })
}

/// Removes the `#[input(...)]` attributes from the parameters; Rust itself doesn't know them.
pub fn strip_input_attributes(function: &mut ItemFn) {
    for argument in &mut function.sig.inputs {
        if let FnArg::Typed(parameter) = argument {
            parameter.attrs.retain(|attribute| !attribute.path().is_ident("input"));
        }
    }
}

fn type_parameters(function: &ItemFn) -> syn::Result<Vec<Ident>> {
    function
        .sig
        .generics
        .params
        .iter()
        .filter_map(|parameter| match parameter {
            GenericParam::Type(type_parameter) => Some(Ok(type_parameter.ident.clone())),
            GenericParam::Lifetime(_) => None,
            GenericParam::Const(constant) => Some(Err(syn::Error::new_spanned(
                constant,
                "value operators support type parameters, but no const parameters",
            ))),
        })
        .collect()
}

/// `fold = "in"`: two parameters of the output's type become one variadic port.
fn folded_input(
    function: &ItemFn,
    fold_port: &LitStr,
    output_type: &OperatorPortType,
    type_parameters: &[Ident],
) -> syn::Result<OperatorPort> {
    let parameters = function.sig.inputs.iter().collect::<Vec<_>>();
    let [first, second] = parameters.as_slice() else {
        return Err(syn::Error::new_spanned(
            &function.sig.inputs,
            "a folded operator takes exactly two parameters, e.g. `fn multiply(a: f32, b: f32) -> f32`",
        ));
    };

    for parameter in [first, second] {
        let parameter = typed(parameter)?;
        if let Some(attribute) = input_attribute(&parameter.attrs) {
            return Err(syn::Error::new_spanned(attribute, "the parameters of a folded operator take no #[input(...)]"));
        }
        if port_type(&parameter.ty, type_parameters)? != *output_type {
            return Err(syn::Error::new_spanned(
                &parameter.ty,
                "both parameters of a folded operator must have the type it returns",
            ));
        }
    }

    Ok(OperatorPort {
        direction: Direction::Input,
        identifier: Ident::new(&fold_port.value(), fold_port.span()),
        port_type: output_type.clone(),
        min_connections: Some(FOLD_MIN_CONNECTIONS),
        default_value: None,
        range: None,
    })
}

fn input_port(argument: &FnArg, type_parameters: &[Ident]) -> syn::Result<OperatorPort> {
    let parameter = typed(argument)?;
    let Pat::Ident(pattern) = parameter.pat.as_ref() else {
        return Err(syn::Error::new_spanned(&parameter.pat, "operator parameters must be plain names, e.g. `gain: f32`"));
    };

    let (port_type, is_list) = match parameter.ty.as_ref() {
        Type::Reference(TypeReference { mutability: None, elem, .. }) => match elem.as_ref() {
            Type::Slice(slice) => (port_type(&slice.elem, type_parameters)?, true),
            _ => return Err(unsupported_type(&parameter.ty)),
        },
        other => (port_type(other, type_parameters)?, false),
    };

    let mut port = OperatorPort {
        direction: Direction::Input,
        identifier: pattern.ident.clone(),
        port_type,
        min_connections: is_list.then_some(1),
        default_value: None,
        range: None,
    };
    if let Some(attribute) = input_attribute(&parameter.attrs) {
        read_input_attribute(attribute, &mut port)?;
    }
    Ok(port)
}

fn read_input_attribute(attribute: &Attribute, port: &mut OperatorPort) -> syn::Result<()> {
    if !matches!(attribute.meta, syn::Meta::List(_)) {
        return Ok(());
    }
    let is_list = port.min_connections.is_some();

    attribute.parse_nested_meta(|meta| {
        if meta.path.is_ident("default") {
            if is_list {
                return Err(meta.error("a variadic input has no default; it needs connections instead"));
            }
            let expression: Expr = meta.value()?.parse()?;
            let default = crate::parse_default_value(expression)?;
            let matches_type = matches!(
                (&port.port_type, default.literal),
                (OperatorPortType::Number, LiteralValue::Float(_) | LiteralValue::Integer(_))
                    | (OperatorPortType::Bool, LiteralValue::Bool(_))
            );
            if !matches_type {
                return Err(syn::Error::new_spanned(&default.expression, "the default doesn't match the port's type"));
            }
            port.default_value = Some(default);
            Ok(())
        } else if meta.path.is_ident("range") {
            if port.port_type != OperatorPortType::Number || is_list {
                return Err(meta.error("`range` only applies to single f32 inputs"));
            }
            port.range = Some(parse_range(&meta.value()?.parse()?)?);
            Ok(())
        } else if meta.path.is_ident("min") {
            if !is_list {
                return Err(meta.error("`min` only applies to variadic inputs (slices)"));
            }
            let min: syn::LitInt = meta.value()?.parse()?;
            let min = min.base10_parse::<usize>()?;
            if min == 0 {
                return Err(meta.error("`min` must be at least 1"));
            }
            port.min_connections = Some(min);
            Ok(())
        } else {
            Err(meta.error("unknown key; expected `default`, `range`, or `min`"))
        }
    })
}

/// `0.0..=1.0`, with literal bounds.
fn parse_range(expression: &Expr) -> syn::Result<(f64, f64)> {
    let bound = |bound: Option<&Expr>| match bound.and_then(literal_value) {
        Some(LiteralValue::Float(value)) => Some(value),
        Some(LiteralValue::Integer(value)) => Some(value as f64),
        _ => None,
    };
    let Expr::Range(range) = expression else {
        return Err(syn::Error::new_spanned(expression, "expected a range with literal bounds, e.g. `0.0..=1.0`"));
    };
    match (bound(range.start.as_deref()), bound(range.end.as_deref())) {
        (Some(low), Some(high)) if low < high => Ok((low, high)),
        _ => Err(syn::Error::new_spanned(expression, "expected a range with literal bounds, low below high, e.g. `0.0..=1.0`")),
    }
}

fn port_type(rust_type: &Type, type_parameters: &[Ident]) -> syn::Result<OperatorPortType> {
    let Type::Path(path) = rust_type else {
        return Err(unsupported_type(rust_type));
    };
    let Some(identifier) = path.path.get_ident() else {
        return Err(unsupported_type(rust_type));
    };
    if identifier == "f32" {
        Ok(OperatorPortType::Number)
    } else if identifier == "bool" {
        Ok(OperatorPortType::Bool)
    } else if type_parameters.contains(identifier) {
        Ok(OperatorPortType::Variable(identifier.clone()))
    } else {
        Err(unsupported_type(rust_type))
    }
}

fn typed(argument: &FnArg) -> syn::Result<&PatType> {
    match argument {
        FnArg::Typed(parameter) => Ok(parameter),
        FnArg::Receiver(receiver) => Err(syn::Error::new_spanned(receiver, "operators are free functions without `self`")),
    }
}

fn input_attribute(attributes: &[Attribute]) -> Option<&Attribute> {
    attributes.iter().find(|attribute| attribute.path().is_ident("input"))
}

fn unsupported_type(rust_type: &Type) -> syn::Error {
    syn::Error::new_spanned(rust_type, format!("unsupported port type; expected {SUPPORTED_TYPES}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(attributes: &str, function: &str) -> syn::Result<OperatorDeclaration> {
        let function: ItemFn = syn::parse_str(function).unwrap();
        let attributes = UnitAttributes::parse(attributes.parse().unwrap()).unwrap();
        parse_value_operator(&function, &attributes)
    }

    fn port<'a>(operator: &'a OperatorDeclaration, name: &str) -> &'a OperatorPort {
        operator.ports.iter().find(|port| port.identifier == name).unwrap()
    }

    #[test]
    fn folded_operator_has_one_variadic_input() {
        let operator = parse("fold = \"in\"", "fn multiply(a: f32, b: f32) -> f32 { a * b }").unwrap();

        assert_eq!(operator.id, "multiply");
        assert!(operator.fold);
        assert_eq!(operator.ports.len(), 2);
        assert_eq!(port(&operator, "in").min_connections, Some(2));
        assert_eq!(port(&operator, "out").port_type, OperatorPortType::Number);
    }

    #[test]
    fn reads_defaults_ranges_and_kebab_case_ids() {
        let operator = parse(
            "",
            "fn scale_offset(input: f32, #[input(default = 1.0, range = -2.0..=2.0)] scale: f32) -> f32 { input * scale }",
        )
        .unwrap();

        assert_eq!(operator.id, "scale-offset");
        assert!(port(&operator, "input").default_value.is_none());
        assert_eq!(port(&operator, "scale").default_value.as_ref().unwrap().literal, LiteralValue::Float(1.0));
        assert_eq!(port(&operator, "scale").range, Some((-2.0, 2.0)));
    }

    #[test]
    fn slices_are_variadic_and_type_parameters_tie_ports_together() {
        let operator = parse(
            "",
            "fn select<T: Copy>(position: f32, #[input(min = 2)] inputs: &[T]) -> T { inputs[0] }",
        )
        .unwrap();

        assert_eq!(operator.type_parameters.len(), 1);
        assert_eq!(port(&operator, "inputs").min_connections, Some(2));
        assert_eq!(port(&operator, "inputs").port_type.name(), "T");
        assert_eq!(port(&operator, "out").port_type.name(), "T");
    }

    #[test]
    fn rejects_invalid_declarations() {
        for (attributes, function) in [
            ("fold = \"in\"", "fn f(a: f32, b: f32, c: f32) -> f32 { a }"),
            ("fold = \"in\"", "fn f(a: f32, b: bool) -> f32 { a }"),
            ("", "fn f(a: f32) {}"),
            ("", "fn f(a: i32) -> f32 { 0.0 }"),
            ("", "fn f<T>(a: f32) -> T { todo!() }"),
            ("", "fn f(#[input(default = true)] a: f32) -> f32 { a }"),
            ("", "fn f(#[input(default = 1.0)] a: &[f32]) -> f32 { a[0] }"),
            ("", "fn f(#[input(min = 2)] a: f32) -> f32 { a }"),
            ("", "fn f(#[input(range = 1.0..=0.0)] a: f32) -> f32 { a }"),
        ] {
            assert!(parse(attributes, function).is_err(), "{function}");
        }
    }
}
