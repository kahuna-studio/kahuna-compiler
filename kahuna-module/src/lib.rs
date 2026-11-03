use kahuna_core::{Rate, StreamKind};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::ParseStream;
use syn::{Data, DeriveInput, Field, Fields, GenericArgument, Ident, PathArguments, Type};

enum Direction {
    Input,
    Output,
}

enum FieldRole<'a> {
    State(&'a Field),
    Input(&'a Field),
    Output(&'a Field),
}

struct PortConfiguration {
    direction: Direction,
    identifier: Ident,
    stream_kind: StreamKind,
    rate_by_type: Option<Rate>,
    rate_override: Option<Rate>,
    default_value: Option<syn::Expr>,
    value_type: Type,
}

struct GeneratedPort {
    field_declaration: proc_macro2::TokenStream,
    constructor_parameter: proc_macro2::TokenStream,
    constructor_initializer: proc_macro2::TokenStream,
    specification: proc_macro2::TokenStream,
}

#[proc_macro_attribute]
pub fn kahuna_module(_attribute: TokenStream, item: TokenStream) -> TokenStream {
    match expand(item.into()) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand(item: proc_macro2::TokenStream) -> syn::Result<proc_macro2::TokenStream> {
    let input: DeriveInput = syn::parse2(item)?;
    let struct_name = &input.ident;
    let struct_attributes = &input.attrs;
    let visibility = &input.vis;

    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(&input, "#[kahuna_module] only supports structs"));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(&input, "#[kahuna_module] requires named fields"));
    };

    // classify: every field becomes exactly one role, or a compile error
    let field_roles = fields.named.iter().map(classify_field).collect::<syn::Result<Vec<_>>>()?;

    let state_fields: Vec<&Field> = field_roles.iter()
      .filter_map(|role| match role {
          FieldRole::State(field) => Some(*field),
          _ => None,
      })
      .collect();

    // parse: every input/output field becomes a validated PortConfiguration, or a compile error
    let ports: Vec<PortConfiguration> = field_roles.iter()
      .filter_map(|role| match role {
          FieldRole::Input(field) => Some((Direction::Input, *field)),
          FieldRole::Output(field) => Some((Direction::Output, *field)),
          FieldRole::State(_) => None,
      })
      .map(|(direction, field)| parse_port(direction, field))
      .collect::<syn::Result<Vec<_>>>()?;

    // generate: pure, infallible — parse_port already validated everything
    let (input_ports, output_ports): (Vec<&PortConfiguration>, Vec<&PortConfiguration>) = ports.iter()
      .partition(|port| matches!(port.direction, Direction::Input));

    let generated_input_ports: Vec<GeneratedPort> = input_ports.into_iter().map(generate_input_port).collect();
    let generated_output_ports: Vec<GeneratedPort> = output_ports.into_iter().map(generate_output_port).collect();

    // assemble: project the fields quote needs to repeat over
    let input_field_declarations = generated_input_ports.iter().map(|port| &port.field_declaration);
    let input_constructor_parameters = generated_input_ports.iter().map(|port| &port.constructor_parameter);
    let input_constructor_initializers = generated_input_ports.iter().map(|port| &port.constructor_initializer);
    let input_specifications = generated_input_ports.iter().map(|port| &port.specification);

    let output_field_declarations = generated_output_ports.iter().map(|port| &port.field_declaration);
    let output_constructor_parameters = generated_output_ports.iter().map(|port| &port.constructor_parameter);
    let output_constructor_initializers = generated_output_ports.iter().map(|port| &port.constructor_initializer);
    let output_specifications = generated_output_ports.iter().map(|port| &port.specification);

    // prefixed with the struct name so several modules can share one Rust module
    let inputs_name = format_ident!("{}Inputs", struct_name);
    let outputs_name = format_ident!("{}Outputs", struct_name);

    Ok(quote! {
        #(#struct_attributes)*
        #visibility struct #struct_name {
            #(#state_fields),*
        }

        pub struct #inputs_name<'a> {
            #(#input_field_declarations)*
            pub _phantom: ::std::marker::PhantomData<&'a ()>,
        }
        impl<'a> #inputs_name<'a> {
            pub fn new(#(#input_constructor_parameters)*) -> Self {
                Self { #(#input_constructor_initializers)* _phantom: ::std::marker::PhantomData }
            }
        }

        pub struct #outputs_name<'a> {
            #(#output_field_declarations)*
            pub _phantom: ::std::marker::PhantomData<&'a ()>,
        }
        impl<'a> #outputs_name<'a> {
            pub fn new(#(#output_constructor_parameters)*) -> Self {
                Self { #(#output_constructor_initializers)* _phantom: ::std::marker::PhantomData }
            }
        }

        impl kahuna_core::KahunaModule for #struct_name {
            type Inputs<'a> = #inputs_name<'a>;
            type Outputs<'a> = #outputs_name<'a>;
            const PORTS: &'static [kahuna_core::PortSpecification] = &[
                #(#input_specifications)*
                #(#output_specifications)*
            ];
            fn new(env: &kahuna_core::ModuleEnv) -> Self {
                Self::init(env)
            }
        }
    })
}

fn classify_field(field: &Field) -> syn::Result<FieldRole<'_>> {
    let has_input_attribute = field.attrs.iter().any(|attribute| attribute.path().is_ident("input"));
    let has_output_attribute = field.attrs.iter().any(|attribute| attribute.path().is_ident("output"));

    match (has_input_attribute, has_output_attribute) {
        (false, false) => Ok(FieldRole::State(field)),
        (true, false) => Ok(FieldRole::Input(field)),
        (false, true) => Ok(FieldRole::Output(field)),
        (true, true) => Err(syn::Error::new_spanned(
            field,
            "a field cannot be both #[input] and #[output]",
        )),
    }
}

fn generate_input_port(port: &PortConfiguration) -> GeneratedPort {
    let field_identifier = &port.identifier;
    let value_type = &port.value_type;
    let port_name = port.identifier.to_string();
    let stream_kind_tokens = stream_kind_to_tokens(port.stream_kind);
    let rate_tokens = rate_tokens_for(port);

    let (field_type, constructor_parameter_type, constructor_initializer, required_tokens) =
      match port.stream_kind {
          StreamKind::Parameter => match &port.default_value {
              // Has a default: the Inputs field stays a concrete T (never Option).
              // The constructor takes Option<T> and resolves the default itself,
              // so process() bodies never see an Option again.
              Some(default_value) => (
                  quote! { #value_type },
                  quote! { Option<#value_type> },
                  quote! { #field_identifier: #field_identifier.unwrap_or(#default_value), },
                  quote! { Some(false) },
              ),
              None => (
                  quote! { #value_type },
                  quote! { #value_type },
                  quote! { #field_identifier, },
                  quote! { Some(true) },
              ),
          },
          StreamKind::Signal => (
              quote! { &'a [#value_type] },
              quote! { &'a [#value_type] },
              quote! { #field_identifier, },
              quote! { Some(true) },
          ),
          StreamKind::Event => (
              quote! { &'a [#value_type] },
              quote! { &'a [#value_type] },
              quote! { #field_identifier, },
              quote! { Some(false) },
          ),
      };

    GeneratedPort {
        field_declaration: quote! { pub #field_identifier: #field_type, },
        constructor_parameter: quote! { #field_identifier: #constructor_parameter_type, },
        constructor_initializer,
        specification: quote! {
            kahuna_core::PortSpecification {
                name: #port_name,
                direction: kahuna_core::PortDirection::Input,
                stream_kind: #stream_kind_tokens,
                rate: #rate_tokens,
                required: #required_tokens,
                value_type: kahuna_core::ValueType::of::<#value_type>(),
            },
        },
    }
}

fn generate_output_port(port: &PortConfiguration) -> GeneratedPort {
    let field_identifier = &port.identifier;
    let value_type = &port.value_type;
    let port_name = port.identifier.to_string();
    let stream_kind_tokens = stream_kind_to_tokens(port.stream_kind);
    let rate_tokens = rate_tokens_for(port);

    let field_type = match port.stream_kind {
        StreamKind::Parameter => quote! { &'a mut #value_type },
        StreamKind::Signal => quote! { &'a mut [#value_type] },
        StreamKind::Event => quote! { &'a mut Vec<#value_type> },
    };

    GeneratedPort {
        field_declaration: quote! { pub #field_identifier: #field_type, },
        constructor_parameter: quote! { #field_identifier: #field_type, },
        constructor_initializer: quote! { #field_identifier, },
        specification: quote! {
            kahuna_core::PortSpecification {
                name: #port_name,
                direction: kahuna_core::PortDirection::Output,
                stream_kind: #stream_kind_tokens,
                rate: #rate_tokens,
                required: None,
                value_type: kahuna_core::ValueType::of::<#value_type>(),
            },
        },
    }
}

fn rate_tokens_for(port: &PortConfiguration) -> proc_macro2::TokenStream {
    match port.rate_override.or(port.rate_by_type) {
        Some(rate) => {
            let rate_tokens = rate_to_tokens(rate);
            quote! { Some(#rate_tokens) }
        }
        None => quote! { None },
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
    let mut default_value: Option<syn::Expr> = None;
    if matches!(attribute.meta, syn::Meta::List(_)) {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("rate") {
                rate_override = Some(parse_rate_expression(meta.value()?)?);
                Ok(())
            } else if meta.path.is_ident("default") {
                default_value = Some(meta.value()?.parse()?);
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
    if default_value.is_some() && matches!(direction, Direction::Output) {
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

fn rate_to_tokens(rate: Rate) -> proc_macro2::TokenStream {
    match rate {
        Rate::AudioRate => quote! { kahuna_core::Rate::AudioRate },
        Rate::ControlRate => quote! { kahuna_core::Rate::ControlRate },
    }
}

fn stream_kind_to_tokens(stream_kind: StreamKind) -> proc_macro2::TokenStream {
    match stream_kind {
        StreamKind::Signal => quote! { kahuna_core::StreamKind::Signal },
        StreamKind::Parameter => quote! { kahuna_core::StreamKind::Parameter },
        StreamKind::Event => quote! { kahuna_core::StreamKind::Event },
    }
}
