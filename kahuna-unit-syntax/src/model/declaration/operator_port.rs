use proc_macro2::Ident;
use crate::model::declaration::literal::DefaultValue;
use crate::model::declaration::port_configuration::Direction;

pub struct OperatorPort {
  pub direction: Direction,
  pub identifier: Ident,
  pub port_type: OperatorPortType,
  /// `Some` for variadic inputs (a slice parameter, or the folded port): how many connections it needs at least.
  pub min_connections: Option<usize>,
  pub default_value: Option<DefaultValue>,
  /// A hint for the UI, e.g. for drawing a knob; values outside are still valid.
  pub range: Option<(f64, f64)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorPortType {
  Number,
  Bool,
  Variable(Ident),
}

impl OperatorPortType {
  /// The name used in manifests, e.g. `f32` or `T`.
  pub fn name(&self) -> String {
    match self {
      OperatorPortType::Number => "f32".to_string(),
      OperatorPortType::Bool => "bool".to_string(),
      OperatorPortType::Variable(identifier) => identifier.to_string(),
    }
  }
}
