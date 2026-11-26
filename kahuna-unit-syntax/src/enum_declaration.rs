use syn::{Data, DeriveInput, Fields, Ident};

/// The attribute that marks an enum as the value type of parameter ports.
pub const ENUM_MARKER: &str = "kahuna_enum";

/// A `#[kahuna_enum]` enum: a choice between named variants, without payloads.
pub struct EnumDeclaration<'a> {
    pub name: &'a Ident,
    /// In source order; the first one is what the compiler starts the enum's wires with.
    pub variants: Vec<&'a Ident>,
}

pub fn parse_enum(input: &DeriveInput) -> syn::Result<EnumDeclaration<'_>> {
    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            format!("#[{ENUM_MARKER}] applies to enums, but `{}` is not one", input.ident),
        ));
    };
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            format!("#[{ENUM_MARKER}] enums cannot have generic parameters"),
        ));
    }
    if data.variants.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.ident,
            format!("#[{ENUM_MARKER}] enums need at least one variant"),
        ));
    }
    if let Some(variant) = data.variants.iter().find(|variant| !matches!(variant.fields, Fields::Unit)) {
        return Err(syn::Error::new_spanned(
            variant,
            format!("variant `{}` carries data; #[{ENUM_MARKER}] enums have unit variants only", variant.ident),
        ));
    }

    Ok(EnumDeclaration {
        name: &input.ident,
        variants: data.variants.iter().map(|variant| &variant.ident).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> syn::Result<Vec<String>> {
        let input: DeriveInput = syn::parse_str(source).unwrap();
        parse_enum(&input).map(|declaration| declaration.variants.iter().map(|variant| variant.to_string()).collect())
    }

    #[test]
    fn reads_the_variants_in_source_order() {
        assert_eq!(parse("enum Color { White, Pink, Brown }").unwrap(), ["White", "Pink", "Brown"]);
    }

    #[test]
    fn rejects_everything_that_is_not_a_plain_choice() {
        for invalid in [
            "struct Color;",
            "enum Color {}",
            "enum Color { White, Tinted(f32) }",
            "enum Color { White, Tinted { amount: f32 } }",
            "enum Color<T> { White(T) }",
            "enum Color<'a> { White }",
        ] {
            assert!(parse(invalid).is_err(), "{invalid}");
        }
    }
}
