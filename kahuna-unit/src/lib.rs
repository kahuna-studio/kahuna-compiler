use kahuna_core::{Rate, StreamKind};
use kahuna_unit_syntax::{
    parse_unit, parse_value_operator, strip_input_attributes, Direction, PortConfiguration, UnitAttributes, UnitKind,
};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{DeriveInput, ItemFn};

struct GeneratedPort {
    field_declaration: proc_macro2::TokenStream,
    constructor_parameter: proc_macro2::TokenStream,
    constructor_initializer: proc_macro2::TokenStream,
    specification: proc_macro2::TokenStream,
}

#[proc_macro_attribute]
pub fn kahuna_module(attribute: TokenStream, item: TokenStream) -> TokenStream {
    match expand(attribute.into(), item.into()) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[proc_macro_attribute]
pub fn kahuna_converter(attribute: TokenStream, item: TokenStream) -> TokenStream {
    match expand_converter(attribute.into(), item.into()) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[proc_macro_attribute]
pub fn kahuna_value_operator(attribute: TokenStream, item: TokenStream) -> TokenStream {
    match expand_value_operator(attribute.into(), item.into()) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand(attribute: proc_macro2::TokenStream, item: proc_macro2::TokenStream) -> syn::Result<proc_macro2::TokenStream> {
    let attributes = UnitAttributes::parse(attribute)?;
    let input: DeriveInput = syn::parse2(item)?;
    let struct_name = &input.ident;
    let struct_attributes = &input.attrs;
    let visibility = &input.vis;

    // parse: shared with the compiler's manifest generator, so both agree on id and ports
    let module = parse_unit(UnitKind::Module, &input, &attributes)?;
    let unit_id = &module.id;
    let state_fields = &module.state_fields;

    // generate: pure, infallible — parse_unit already validated everything
    let (input_ports, output_ports): (Vec<&PortConfiguration>, Vec<&PortConfiguration>) = module.ports.iter()
      .partition(|port| port.direction == Direction::Input);

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
            const ID: &'static str = #unit_id;
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

fn expand_converter(attribute: proc_macro2::TokenStream, item: proc_macro2::TokenStream) -> syn::Result<proc_macro2::TokenStream> {
    let attributes = UnitAttributes::parse(attribute)?;
    let input: DeriveInput = syn::parse2(item)?;
    let struct_name = &input.ident;
    let struct_attributes = &input.attrs;
    let visibility = &input.vis;

    // parse: shared with the compiler's manifest generator, so both agree on id and ports
    let converter = parse_unit(UnitKind::Converter, &input, &attributes)?;
    let unit_id = &converter.id;
    let state_fields = &converter.state_fields;

    // only the port descriptions: process() takes the input and output values directly
    let specifications = converter.ports(Direction::Input).map(|port| generate_input_port(port).specification)
      .chain(converter.ports(Direction::Output).map(|port| generate_output_port(port).specification));

    Ok(quote! {
        #(#struct_attributes)*
        #visibility struct #struct_name {
            #(#state_fields),*
        }

        impl kahuna_core::KahunaConverter for #struct_name {
            const ID: &'static str = #unit_id;
            const PORTS: &'static [kahuna_core::PortSpecification] = &[
                #(#specifications)*
            ];
            fn new(env: &kahuna_core::ModuleEnv) -> Self {
                Self::init(env)
            }
        }
    })
}

fn expand_value_operator(attribute: proc_macro2::TokenStream, item: proc_macro2::TokenStream) -> syn::Result<proc_macro2::TokenStream> {
    let attributes = UnitAttributes::parse(attribute)?;
    let mut function: ItemFn = syn::parse2(item)?;

    // parse: only to validate; the manifest generator reads the same declaration
    parse_value_operator(&function, &attributes)?;

    strip_input_attributes(&mut function);
    // generated patch code calls the operator once per sample, from another crate
    if !function.attrs.iter().any(|attribute| attribute.path().is_ident("inline")) {
        function.attrs.push(syn::parse_quote!(#[inline]));
    }

    Ok(quote! { #function })
}

fn generate_input_port(port: &PortConfiguration) -> GeneratedPort {
    let field_identifier = &port.identifier;
    let value_type = &port.value_type;
    let port_name = port.identifier.to_string();
    let stream_kind_tokens = stream_kind_to_tokens(port.stream_kind);
    let rate_tokens = rate_tokens_for(port);

    let required_tokens = required_to_tokens(port.required());

    let (field_type, constructor_parameter_type, constructor_initializer) =
      match port.stream_kind {
          StreamKind::Parameter => match &port.default_value {
              // Has a default: the Inputs field stays a concrete T (never Option).
              // The constructor takes Option<T> and resolves the default itself,
              // so process() bodies never see an Option again.
              Some(default_value) => {
                  let default_expression = &default_value.expression;
                  (
                      quote! { #value_type },
                      quote! { Option<#value_type> },
                      quote! { #field_identifier: #field_identifier.unwrap_or(#default_expression), },
                  )
              }
              None => (
                  quote! { #value_type },
                  quote! { #value_type },
                  quote! { #field_identifier, },
              ),
          },
          StreamKind::Signal | StreamKind::Event => (
              quote! { &'a [#value_type] },
              quote! { &'a [#value_type] },
              quote! { #field_identifier, },
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

fn required_to_tokens(required: Option<bool>) -> proc_macro2::TokenStream {
    match required {
        Some(required) => quote! { Some(#required) },
        None => quote! { None },
    }
}

fn rate_tokens_for(port: &PortConfiguration) -> proc_macro2::TokenStream {
    match port.rate() {
        Some(rate) => {
            let rate_tokens = rate_to_tokens(rate);
            quote! { Some(#rate_tokens) }
        }
        None => quote! { None },
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
