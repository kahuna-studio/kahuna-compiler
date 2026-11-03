//! Parses unit declarations (`#[kahuna_module]` structs) from Rust syntax.
//!
//! Shared by the `kahuna-module` macro, which generates code from it, and the compiler's
//! manifest generator, which reads unit libraries without compiling them. Keeping one parser
//! guarantees that both always agree on a unit's id and ports.

use heck::ToKebabCase;
use kahuna_core::{Rate, StreamKind};
use proc_macro2::TokenStream;
use syn::parse::{ParseStream, Parser};
use syn::{Data, DeriveInput, Expr, Field, Fields, GenericArgument, Ident, Lit, LitStr, PathArguments, Type, UnOp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Input,
    Output,
}

pub struct PortConfiguration {
    pub direction: Direction,
    pub identifier: Ident,
    pub stream_kind: StreamKind,
    pub rate_by_type: Option<Rate>,
    pub rate_override: Option<Rate>,
    pub default_value: Option<DefaultValue>,
    pub value_type: Type,
}

impl PortConfiguration {
    pub fn rate(&self) -> Option<Rate> {
        self.rate_override.or(self.rate_by_type)
    }

    /// `None` for outputs, where the concept doesn't apply.
    pub fn required(&self) -> Option<bool> {
        match (self.direction, self.stream_kind) {
            (Direction::Output, _) => None,
            (Direction::Input, StreamKind::Parameter) => Some(self.default_value.is_none()),
            (Direction::Input, StreamKind::Signal) => Some(true),
            (Direction::Input, StreamKind::Event) => Some(false),
        }
    }
}

/// A `default = ...` value. Only literals are allowed, so tools that read the source without
/// compiling it see exactly the value the compiled module uses.
pub struct DefaultValue {
    pub expression: Expr,
    pub literal: LiteralValue,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LiteralValue {
    Bool(bool),
    Integer(i64),
    Float(f64),
}

/// The arguments of `#[kahuna_module(...)]`.
#[derive(Default)]
pub struct UnitAttributes {
    pub id: Option<LitStr>,
}

impl UnitAttributes {
    pub fn parse(tokens: TokenStream) -> syn::Result<Self> {
        let mut attributes = UnitAttributes::default();
        let parser = syn::meta::parser(|meta| {
            if meta.path.is_ident("id") {
                let id: LitStr = meta.value()?.parse()?;
                if !is_valid_unit_id(&id.value()) {
                    return Err(syn::Error::new_spanned(
                        &id,
                        "unit ids must be kebab-case: lowercase letters, digits, and single hyphens",
                    ));
                }
                attributes.id = Some(id);
                Ok(())
            } else {
                Err(meta.error("unknown key; expected `id`"))
            }
        });
        parser.parse2(tokens)?;
        Ok(attributes)
    }
}

pub struct ModuleDeclaration<'a> {
    pub id: String,
    pub state_fields: Vec<&'a Field>,
    pub ports: Vec<PortConfiguration>,
}

pub fn parse_module<'a>(input: &'a DeriveInput, attributes: &UnitAttributes) -> syn::Result<ModuleDeclaration<'a>> {
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(input, "#[kahuna_module] only supports structs"));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(input, "#[kahuna_module] requires named fields"));
    };

    let mut state_fields = Vec::new();
    let mut ports = Vec::new();
    for field in &fields.named {
        match classify_field(field)? {
            None => state_fields.push(field),
            Some(direction) => ports.push(parse_port(direction, field)?),
        }
    }

    let id = match &attributes.id {
        Some(id) => id.value(),
        None => default_unit_id(&input.ident),
    };

    Ok(ModuleDeclaration { id, state_fields, ports })
}

/// The id a unit gets when none is set explicitly: its struct name in kebab-case.
pub fn default_unit_id(struct_name: &Ident) -> String {
    struct_name.to_string().to_kebab_case()
}

fn is_valid_unit_id(id: &str) -> bool {
    !id.is_empty()
      && id.split('-').all(|part| {
          !part.is_empty() && part.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
      })
}

/// `None` means a state field; every field is exactly one of state, input, or output.
fn classify_field(field: &Field) -> syn::Result<Option<Direction>> {
    let has_input_attribute = field.attrs.iter().any(|attribute| attribute.path().is_ident("input"));
    let has_output_attribute = field.attrs.iter().any(|attribute| attribute.path().is_ident("output"));

    match (has_input_attribute, has_output_attribute) {
        (false, false) => Ok(None),
        (true, false) => Ok(Some(Direction::Input)),
        (false, true) => Ok(Some(Direction::Output)),
        (true, true) => Err(syn::Error::new_spanned(
            field,
            "a field cannot be both #[input] and #[output]",
        )),
    }
}

fn parse_port(direction: Direction, field: &Field) -> syn::Result<PortConfiguration> {
    let field_identifier = field.ident.clone().unwrap();
    let field_type = &field.ty;

    let attribute = field
      .attrs
      .iter()
      .find(|attribute| attribute.path().is_ident("input") || attribute.path().is_ident("output"))
      .expect("classify_field already guaranteed this field has #[input] or #[output]");

    let last_type_segment = match field_type {
        Type::Path(type_path) => {
            let segments = &type_path.path.segments;
            if segments.len() > 1 {
                return Err(syn::Error::new_spanned(attribute, "Qualified struct names are not supported"));
            }
            segments.last().unwrap()
        }
        _ => return Err(syn::Error::new_spanned(
            attribute,
            "Only types Audio, Control, Midi, Signal<T>, Parameter<T>, and Event<T> are allowed.",
        )),
    };

    let (stream_kind, rate_by_type, value_type) = match last_type_segment.ident.to_string().as_str() {
        "Audio" => (StreamKind::Signal, Some(Rate::AudioRate), syn::parse_quote!(kahuna_core::Bipolar)),
        "Control" => (StreamKind::Parameter, Some(Rate::ControlRate), syn::parse_quote!(kahuna_core::Unipolar)),
        "Midi" => (StreamKind::Event, None, syn::parse_quote!(kahuna_core::MidiEvent)),

        "Signal" => (StreamKind::Signal, Some(Rate::AudioRate), extract_value_type(last_type_segment)?),
        "Parameter" => (StreamKind::Parameter, Some(Rate::ControlRate), extract_value_type(last_type_segment)?),
        "Event" => (StreamKind::Event, None, extract_value_type(last_type_segment)?),

        _ => return Err(syn::Error::new_spanned(
            attribute,
            "Only types Audio, Control, Midi, Signal<T>, Parameter<T>, and Event<T> are allowed.",
        )),
    };

    let mut rate_override: Option<Rate> = None;
    let mut default_value: Option<DefaultValue> = None;
    if matches!(attribute.meta, syn::Meta::List(_)) {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("rate") {
                rate_override = Some(parse_rate_expression(meta.value()?)?);
                Ok(())
            } else if meta.path.is_ident("default") {
                default_value = Some(parse_default_value(meta.value()?.parse()?)?);
                Ok(())
            } else {
                Err(meta.error("unknown key; expected `rate` or `default`"))
            }
        })?;
    }

    if stream_kind == StreamKind::Event && rate_override.is_some() {
        return Err(syn::Error::new_spanned(
            attribute,
            "Event ports have no rate; `rate = ...` is not applicable here.",
        ));
    }
    if default_value.is_some() && direction == Direction::Output {
        return Err(syn::Error::new_spanned(attribute, "`default` is not applicable to #[output] ports."));
    }
    if default_value.is_some() && stream_kind == StreamKind::Event {
        return Err(syn::Error::new_spanned(
            attribute,
            "Event ports have no default; `default = ...` is not applicable here.",
        ));
    }

    Ok(PortConfiguration {
        direction,
        identifier: field_identifier,
        stream_kind,
        rate_by_type,
        rate_override,
        default_value,
        value_type,
    })
}

fn parse_default_value(expression: Expr) -> syn::Result<DefaultValue> {
    let literal = literal_value(&expression).ok_or_else(|| {
        syn::Error::new_spanned(&expression, "`default` must be a literal, e.g. `0.5`, `-1`, or `true`")
    })?;
    Ok(DefaultValue { expression, literal })
}

fn literal_value(expression: &Expr) -> Option<LiteralValue> {
    match expression {
        Expr::Lit(literal) => match &literal.lit {
            Lit::Bool(value) => Some(LiteralValue::Bool(value.value)),
            Lit::Int(value) => value.base10_parse().ok().map(LiteralValue::Integer),
            Lit::Float(value) => value.base10_parse().ok().map(LiteralValue::Float),
            _ => None,
        },
        Expr::Unary(unary) if matches!(unary.op, UnOp::Neg(_)) => match literal_value(&unary.expr)? {
            LiteralValue::Integer(value) => Some(LiteralValue::Integer(-value)),
            LiteralValue::Float(value) => Some(LiteralValue::Float(-value)),
            LiteralValue::Bool(_) => None,
        },
        _ => None,
    }
}

fn parse_rate_expression(input: ParseStream) -> syn::Result<Rate> {
    let path: syn::Path = input.parse()?;
    let identifier = path
      .segments
      .last()
      .ok_or_else(|| syn::Error::new_spanned(&path, "expected a Rate variant"))?
      .ident
      .to_string();

    match identifier.as_str() {
        "AudioRate" => Ok(Rate::AudioRate),
        "ControlRate" => Ok(Rate::ControlRate),
        _ => Err(syn::Error::new_spanned(&path, "expected `Rate::AudioRate` or `Rate::ControlRate`")),
    }
}

fn extract_value_type(segment: &syn::PathSegment) -> syn::Result<Type> {
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(syn::Error::new_spanned(segment, "expected a generic type argument, e.g. Signal<f32>"));
    };

    let mut type_arguments = arguments.args.iter().filter_map(|argument| match argument {
        GenericArgument::Type(argument_type) => Some(argument_type),
        _ => None,
    });

    match (type_arguments.next(), type_arguments.next()) {
        (Some(argument_type), None) => Ok(argument_type.clone()),
        _ => Err(syn::Error::new_spanned(segment, "expected exactly one generic type argument, e.g. Signal<f32>")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_struct(source: &str) -> DeriveInput {
        syn::parse_str(source).unwrap()
    }

    #[test]
    fn default_id_is_kebab_case_and_keeps_acronyms_together() {
        let input = parse_struct("struct ADSREnvelope {}");
        let module = parse_module(&input, &UnitAttributes::default()).unwrap();
        assert_eq!(module.id, "adsr-envelope");
    }

    #[test]
    fn explicit_id_wins() {
        let input = parse_struct("struct SineOsc {}");
        let attributes = UnitAttributes::parse("id = \"sine\"".parse().unwrap()).unwrap();
        let module = parse_module(&input, &attributes).unwrap();
        assert_eq!(module.id, "sine");
    }

    #[test]
    fn rejects_ids_that_are_not_kebab_case() {
        for id in ["Sine", "sine_osc", "-sine", "sine--osc", ""] {
            let tokens = format!("id = \"{id}\"").parse().unwrap();
            assert!(UnitAttributes::parse(tokens).is_err(), "{id} should be rejected");
        }
    }

    #[test]
    fn reads_literal_defaults_including_negative_numbers() {
        let input = parse_struct(
            "struct Unit { #[input(default = -0.5)] a: Parameter<f32>, #[input(default = true)] b: Parameter<bool> }",
        );
        let module = parse_module(&input, &UnitAttributes::default()).unwrap();
        let defaults: Vec<_> = module.ports.iter().map(|port| port.default_value.as_ref().unwrap().literal).collect();
        assert_eq!(defaults, [LiteralValue::Float(-0.5), LiteralValue::Bool(true)]);
    }

    #[test]
    fn rejects_non_literal_defaults() {
        let input = parse_struct("struct Unit { #[input(default = SOME_CONST)] a: Parameter<f32> }");
        assert!(parse_module(&input, &UnitAttributes::default()).is_err());
    }
}
