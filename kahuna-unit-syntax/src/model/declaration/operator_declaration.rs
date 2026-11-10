use proc_macro2::Ident;
use crate::model::declaration::unit_kind::UnitKind;
use crate::operator::OperatorPort;

pub struct OperatorDeclaration {
  pub kind: UnitKind,
  pub id: String,
  /// Each one ties together all ports of that type: whatever is connected must match.
  pub type_parameters: Vec<Ident>,
  /// The inputs in declaration order, then the output.
  pub ports: Vec<OperatorPort>,
  /// The function takes two values and is applied pairwise over all connections of its one input.
  pub fold: bool,
}
