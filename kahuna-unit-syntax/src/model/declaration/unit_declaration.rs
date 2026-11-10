use syn::Field;
use crate::UnitKind;
use crate::model::declaration::port_configuration::{Direction, PortConfiguration};

pub struct UnitDeclaration<'a> {
  pub kind: UnitKind,
  pub id: String,
  pub state_fields: Vec<&'a Field>,
  /// In declaration order. A converter has exactly one input and one output.
  pub ports: Vec<PortConfiguration>,
}

impl UnitDeclaration<'_> {
  pub fn ports(&self, direction: Direction) -> impl Iterator<Item = &PortConfiguration> {
    self.ports.iter().filter(move |port| port.direction == direction)
  }
}
