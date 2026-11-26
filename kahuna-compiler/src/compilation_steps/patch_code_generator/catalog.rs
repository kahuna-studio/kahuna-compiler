use std::collections::HashMap;

pub trait UnitCatalog {
  fn find(&self, unit: &str) -> Option<&UnitSpec>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
  Events,
  Signal,
  Float,
  Bool,
  /// A `#[kahuna_enum]` parameter; which enum is in `enum_type` of the port or buffer.
  Enum,
  /// Ports of value operators only: an audio signal or a parameter, whichever is connected.
  /// Never the kind of a buffer; an operator's output resolves to `Signal` or `Float`.
  Number,
}

impl ValueKind {
  /// Whether a port of this kind takes a connection from a buffer of `source` kind.
  pub fn accepts(self, source: ValueKind) -> bool {
    self == source || (self == ValueKind::Number && matches!(source, ValueKind::Signal | ValueKind::Float))
  }
}

/// An enum a parameter carries, as generated code names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumType {
  /// From the library's crate on, e.g. `kahuna_std::modules::noise::NoiseColor`.
  pub path: String,
  /// The value the enum's buffers start with.
  pub first_variant: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arity {
  Required,
  Optional,
  Variadic,
}

#[derive(Debug, Clone)]
pub struct PortSpec {
  pub name: String,
  pub kind: ValueKind,
  /// Present exactly when `kind` is `Enum`.
  pub enum_type: Option<EnumType>,
  pub arity: Arity,
  /// Variadic ports only.
  pub min_connections: usize,
  /// Value operator inputs only: what the generated code passes when the port is unconnected.
  /// (Modules resolve their own defaults.)
  pub default: Option<f32>,
}

/// How the generated code invokes `process` on a unit.
#[derive(Debug, Clone)]
pub enum CallStyle {
  /// `unit.process(&input, &mut output)`
  Converter,
  /// `unit.process(&<Unit as KahunaModule>::Inputs::new(..), &mut <Unit as KahunaModule>::Outputs::new(..))`
  Module,
  /// No instance: `out[frame] = operator(input_0[frame], ..)`, once per sample for an audio
  /// signal output, once per block for a parameter output. A folded operator is applied
  /// pairwise over all connections of its one variadic input.
  ValueOperator { fold: bool },
}

#[derive(Debug, Clone)]
pub struct UnitSpec {
  /// What the IDE builds node ids from, e.g. `adsr` for `adsr_0`: the unit's id, or the id with its
  /// library and an underscore as prefix where two libraries have a unit with that id
  /// (`self_sine`, `kahuna-std_sine`).
  // Read by the IDE; the compiler itself only resolves it.
  #[allow(dead_code)]
  pub short_name: String,
  pub type_path: String,
  pub call_style: CallStyle,
  pub inputs: Vec<PortSpec>,
  pub outputs: Vec<PortSpec>,
}

#[derive(Debug)]
pub struct StaticCatalog {
  units: HashMap<String, UnitSpec>,
}

impl StaticCatalog {
  pub fn new(units: impl IntoIterator<Item = (String, UnitSpec)>) -> Self {
    Self {
      units: units.into_iter().collect(),
    }
  }
}

impl UnitCatalog for StaticCatalog {
  fn find(&self, unit: &str) -> Option<&UnitSpec> {
    self.units.get(unit)
  }
}

impl PortSpec {
  pub fn required(name: &str, kind: ValueKind) -> Self {
    Self::with_arity(name, kind, Arity::Required)
  }

  fn with_arity(name: &str, kind: ValueKind, arity: Arity) -> Self {
    Self {
      name: name.to_string(),
      kind,
      enum_type: None,
      arity,
      min_connections: 0,
      default: None,
    }
  }
}
