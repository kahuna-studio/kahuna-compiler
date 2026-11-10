use syn::Expr;

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
