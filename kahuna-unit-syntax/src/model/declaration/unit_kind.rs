#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitKind {
  Module,
  Converter,
  ValueOperator,
}

impl UnitKind {
  const ALL: [UnitKind; 3] = [UnitKind::Module, UnitKind::Converter, UnitKind::ValueOperator];

  /// The attribute that marks a struct (modules, converters) or a function (operators) as this kind of unit.
  pub fn marker(self) -> &'static str {
    match self {
      UnitKind::Module => "kahuna_module",
      UnitKind::Converter => "kahuna_converter",
      UnitKind::ValueOperator => "kahuna_value_operator",
    }
  }

  pub fn from_marker(marker: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|kind| kind.marker() == marker)
  }

  /// Operators are declared as functions, all other units as structs.
  pub fn is_operator(self) -> bool {
    self == UnitKind::ValueOperator
  }
}
