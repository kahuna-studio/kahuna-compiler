use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, GenericArgument, PathArguments, Type};

#[derive(Clone, Copy)]
enum Kind { Parameter, Signal, Event }

struct PortAttrs {
    kind: Kind,
    rate: Option<syn::Expr>,
    enum_variants: Option<syn::Expr>,
}

fn parse_port_attrs(attr: &syn::Attribute) -> syn::Result<PortAttrs> {
    let mut kind = None;
    let mut rate = None;
    let mut enum_variants = None;

    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("parameter") {
            kind = Some(Kind::Parameter);
        } else if meta.path.is_ident("signal") {
            kind = Some(Kind::Signal);
        } else if meta.path.is_ident("event") {
            kind = Some(Kind::Event);
        } else if meta.path.is_ident("rate") {
            rate = Some(meta.value()?.parse()?);
        } else if meta.path.is_ident("enum_variants") {
            enum_variants = Some(meta.value()?.parse()?);
        } else {
            return Err(meta.error("expected parameter, signal, event, rate, or enum_variants"));
        }
        Ok(())
    })?;

    let kind = kind.ok_or_else(|| {
        syn::Error::new_spanned(attr, "must specify parameter, signal, or event")
    })?;

    Ok(PortAttrs { kind, rate, enum_variants })
}

/// If `ty` is `Option<T>`, returns `Some(T)`; otherwise `None`.
fn unwrap_option(ty: &Type) -> Option<Type> {
    let Type::Path(p) = ty else { return None };
    let seg = p.path.segments.last()?;
    if seg.ident != "Option" { return None; }
    let PathArguments::AngleBracketed(args) = &seg.arguments else { return None };
    args.args.iter().find_map(|a| match a {
        GenericArgument::Type(t) => Some(t.clone()),
        _ => None,
    })
}

#[proc_macro_attribute]
pub fn kahuna_module(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let name = &input.ident;
    let struct_attrs = &input.attrs;
    let vis = &input.vis;

    let Data::Struct(data) = &input.data else {
        return syn::Error::new_spanned(&input, "#[kahuna_module] only supports structs")
          .to_compile_error().into();
    };
    let Fields::Named(fields) = &data.fields else {
        return syn::Error::new_spanned(&input, "#[kahuna_module] requires named fields")
          .to_compile_error().into();
    };

    let mut state_fields = Vec::new();
    let (mut in_fields, mut in_ctor_params, mut in_ctor_inits, mut in_specs) =
      (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut out_fields, mut out_ctor_params, mut out_ctor_inits, mut out_specs) =
      (Vec::new(), Vec::new(), Vec::new(), Vec::new());

    for field in &fields.named {
        let fname = field.ident.clone().unwrap();
        let fty = &field.ty;
        let name_str = fname.to_string();

        let input_attr = field.attrs.iter().find(|a| a.path().is_ident("input"));
        let output_attr = field.attrs.iter().find(|a| a.path().is_ident("output"));

        if let Some(attr) = input_attr {
            let p = match parse_port_attrs(attr) { Ok(p) => p, Err(e) => return e.to_compile_error().into() };
            let optional = unwrap_option(fty);
            let base_ty = optional.clone().unwrap_or_else(|| fty.clone());
            let is_opt = optional.is_some();

            let rate = match (&p.kind, &p.rate) {
                (_, Some(r)) => quote! { Some(#r) },
                (Kind::Parameter, None) => quote! { Some(kahuna_core::Rate::ControlRate) },
                (Kind::Signal, None) => quote! { Some(kahuna_core::Rate::AudioRate) },
                (Kind::Event, None) => quote! { None },
            };
            let value_type = match &p.enum_variants {
                Some(v) => quote! { kahuna_core::PortType::of_enum::<#base_ty>(#v) },
                None => quote! { kahuna_core::PortType::of::<#base_ty>() },
            };

            let (field_ty, required, stream_kind) = match p.kind {
                Kind::Parameter => {
                    let ty = if is_opt { quote!{ Option<#base_ty> } } else { quote!{ #base_ty } };
                    let req = if is_opt { quote!{ Some(false) } } else { quote!{ Some(true) } };
                    (ty, req, quote!{ kahuna_core::PortKind::Parameter })
                }
                Kind::Signal => {
                    let ty = if is_opt { quote!{ Option<&'a [#base_ty]> } } else { quote!{ &'a [#base_ty] } };
                    let req = if is_opt { quote!{ Some(false) } } else { quote!{ Some(true) } };
                    (ty, req, quote!{ kahuna_core::PortKind::Signal })
                }
                Kind::Event => (quote!{ &'a [#base_ty] }, quote!{ Some(false) }, quote!{ kahuna_core::PortKind::Event }),
            };

            in_fields.push(quote! { pub #fname: #field_ty, });
            in_ctor_params.push(quote! { #fname: #field_ty, });
            in_ctor_inits.push(quote! { #fname, });
            in_specs.push(quote! {
                kahuna_core::PortSpec {
                    name: #name_str, direction: kahuna_core::PortDir::Input,
                    stream_type: #stream_kind, rate: #rate,
                    required: #required, value_type: #value_type,
                },
            });
        } else if let Some(attr) = output_attr {
            let p = match parse_port_attrs(attr) { Ok(p) => p, Err(e) => return e.to_compile_error().into() };
            let base_ty = fty.clone();

            let rate = match (&p.kind, &p.rate) {
                (_, Some(r)) => quote! { Some(#r) },
                (Kind::Parameter, None) => quote! { Some(kahuna_core::Rate::ControlRate) },
                (Kind::Signal, None) => quote! { Some(kahuna_core::Rate::AudioRate) },
                (Kind::Event, None) => quote! { None },
            };
            let value_type = match &p.enum_variants {
                Some(v) => quote! { kahuna_core::PortType::of_enum::<#base_ty>(#v) },
                None => quote! { kahuna_core::PortType::of::<#base_ty>() },
            };

            let (field_ty, stream_kind) = match p.kind {
                Kind::Parameter => (quote!{ &'a mut #base_ty }, quote!{ kahuna_core::PortKind::Parameter }),
                Kind::Signal => (quote!{ &'a mut [#base_ty] }, quote!{ kahuna_core::PortKind::Signal }),
                Kind::Event => (quote!{ &'a mut Vec<#base_ty> }, quote!{ kahuna_core::PortKind::Event }),
            };

            out_fields.push(quote! { pub #fname: #field_ty, });
            out_ctor_params.push(quote! { #fname: #field_ty, });
            out_ctor_inits.push(quote! { #fname, });
            out_specs.push(quote! {
                kahuna_core::PortSpec {
                    name: #name_str, direction: kahuna_core::PortDir::Output,
                    stream_type: #stream_kind, rate: #rate,
                    required: None, value_type: #value_type,
                },
            });
        } else {
            state_fields.push(field.clone());
        }
    }

    quote! {
        #(#struct_attrs)*
        #vis struct #name {
            #(#state_fields),*
        }

        pub struct Inputs<'a> {
            #(#in_fields)*
            pub _phantom: ::std::marker::PhantomData<&'a ()>,
        }
        impl<'a> Inputs<'a> {
            pub fn new(#(#in_ctor_params)*) -> Self {
                Self { #(#in_ctor_inits)* _phantom: ::std::marker::PhantomData }
            }
        }

        pub struct Outputs<'a> {
            #(#out_fields)*
            pub _phantom: ::std::marker::PhantomData<&'a ()>,
        }
        impl<'a> Outputs<'a> {
            pub fn new(#(#out_ctor_params)*) -> Self {
                Self { #(#out_ctor_inits)* _phantom: ::std::marker::PhantomData }
            }
        }

        impl kahuna_core::KahunaModule for #name {
            const PORTS: &'static [kahuna_core::PortSpec] = &[
                #(#in_specs)*
                #(#out_specs)*
            ];
            fn new(env: &kahuna_core::ModuleEnv) -> Self {
                Self::init(env)
            }
        }
    }.into()
}
