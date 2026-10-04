use proc_macro::TokenStream;
use syn::parse_macro_input;

mod ast;
mod codegen;

use ast::PatchDef;

#[proc_macro]
pub fn kahuna_patch(input: TokenStream) -> TokenStream {
    let patch = parse_macro_input!(input as PatchDef);

    match codegen::generate(&patch) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
