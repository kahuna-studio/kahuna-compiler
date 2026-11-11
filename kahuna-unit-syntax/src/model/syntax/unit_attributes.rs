use proc_macro2::TokenStream;
use syn::LitStr;
use syn::parse::Parser;
use crate::is_valid_unit_id;

/// The arguments of a unit's attribute, e.g. `#[kahuna_module(id = "...")]`.
#[derive(Default)]
pub struct UnitAttributes {
  pub id: Option<LitStr>,
  /// Value operators only: the name of the variadic port the function is folded over.
  pub fold: Option<LitStr>,
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
            "unit ids must match [a-z][a-z0-9]*(-[a-z0-9]+)*: start with a lowercase letter, then lowercase letters, digits, and single hyphens",
          ));
        }
        attributes.id = Some(id);
        Ok(())
      } else if meta.path.is_ident("fold") {
        attributes.fold = Some(meta.value()?.parse()?);
        Ok(())
      } else {
        Err(meta.error("unknown key; expected `id` or `fold`"))
      }
    });
    parser.parse2(tokens)?;
    Ok(attributes)
  }
}
