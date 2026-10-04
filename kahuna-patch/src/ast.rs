use proc_macro2::Span;
use syn::{
  Ident, Type, Token,
  parse::{Parse, ParseStream},
};

mod kw {
  syn::custom_keyword!(io);
  syn::custom_keyword!(module);
  syn::custom_keyword!(connect_to);
}

pub struct PatchDef {
  pub io_nodes: Vec<IoNodeDecl>,
  pub modules: Vec<ModuleDecl>,
  pub connections: Vec<ConnectionDecl>,
}

pub struct IoNodeDecl {
  pub name: Ident,
  pub ty: Type,
}

pub struct ModuleDecl {
  pub name: Ident,
  pub ty: Type,
}

#[derive(Clone)]
pub struct PortPath {
  pub node: Ident,
  pub port: Ident,
  pub span: Span,
}

pub struct ConnectionDecl {
  pub source: PortPath,
  pub target: PortPath,
  pub span: Span,
}

impl Parse for PortPath {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    let node: Ident = input.parse()?;
    input.parse::<Token![.]>()?;
    let port: Ident = input.parse()?;
    let span = node.span().join(port.span()).unwrap_or(node.span());
    Ok(PortPath { node, port, span })
  }
}

impl Parse for IoNodeDecl {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    input.parse::<kw::io>()?;
    let name: Ident = input.parse()?;
    input.parse::<Token![:]>()?;
    let ty: Type = input.parse()?;
    input.parse::<Token![;]>()?;
    Ok(IoNodeDecl { name, ty })
  }
}

impl Parse for ModuleDecl {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    input.parse::<kw::module>()?;
    let name: Ident = input.parse()?;
    input.parse::<Token![:]>()?;
    let ty: Type = input.parse()?;
    input.parse::<Token![;]>()?;
    Ok(ModuleDecl { name, ty })
  }
}

impl Parse for ConnectionDecl {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    let source: PortPath = input.parse()?;
    input.parse::<Token![.]>()?;

    let call_span = input.span();
    input.parse::<kw::connect_to>()?;

    let content;
    syn::parenthesized!(content in input);
    let target: PortPath = content.parse()?;

    if !content.is_empty() {
      return Err(syn::Error::new(
        content.span(),
        "in dieser vereinfachten Form sind zusätzliche Argumente \
                 (z. B. `via = ...`) noch nicht unterstützt",
      ));
    }

    input.parse::<Token![;]>()?;

    Ok(ConnectionDecl { source, target, span: call_span })
  }
}

impl Parse for PatchDef {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    let mut io_nodes = Vec::new();
    let mut modules = Vec::new();
    let mut connections = Vec::new();

    while !input.is_empty() {
      if input.peek(kw::io) {
        io_nodes.push(input.parse()?);
      } else if input.peek(kw::module) {
        modules.push(input.parse()?);
      } else if input.peek(Ident) {
        connections.push(input.parse()?);
      } else {
        return Err(input.error(
          "erwartet `io`, `module` oder eine `connect_to`-Verbindung",
        ));
      }
    }

    Ok(PatchDef { io_nodes, modules, connections })
  }
}
