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
  pub arity: Arity,
}

/// How the generated code invokes `process` on a unit.
#[derive(Debug, Clone)]
pub enum CallStyle {
  /// `unit.process(&input, &mut output)`
  Converter,
  /// `unit.process(&io::Inputs::new(..), &mut io::Outputs::new(..))`
  Module { io_module_path: String },
  /// `unit.process(&[&input_0, &input_1, ..], &mut output)`
  VariadicSlice,
}

#[derive(Debug, Clone)]
pub struct UnitSpec {
  pub type_path: String,
  pub call_style: CallStyle,
  pub inputs: Vec<PortSpec>,
  pub outputs: Vec<PortSpec>,
}

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

  pub fn optional(name: &str, kind: ValueKind) -> Self {
    Self::with_arity(name, kind, Arity::Optional)
  }

  pub fn variadic(name: &str, kind: ValueKind) -> Self {
    Self::with_arity(name, kind, Arity::Variadic)
  }

  fn with_arity(name: &str, kind: ValueKind, arity: Arity) -> Self {
    Self {
      name: name.to_string(),
      kind,
      arity,
    }
  }
}

pub fn builtin_catalog() -> StaticCatalog {
  StaticCatalog::new([
    (
      "kahuna_std::MidiEventToFrequencyParameter".to_string(),
      UnitSpec {
        type_path: "kahuna_std::mapping::midi_event_to_frequency_parameter::MidiEventToFrequencyParameter".to_string(),
        call_style: CallStyle::Converter,
        inputs: vec![PortSpec::required("midi_events", ValueKind::Events)],
        outputs: vec![PortSpec::required("out", ValueKind::Float)],
      },
    ),
    (
      "kahuna_std::MidiEventToGateParameter".to_string(),
      UnitSpec {
        type_path: "kahuna_std::mapping::midi_event_to_gate_parameter::MidiEventToGateParameter".to_string(),
        call_style: CallStyle::Converter,
        inputs: vec![PortSpec::required("midi_events", ValueKind::Events)],
        outputs: vec![PortSpec::required("out", ValueKind::Bool)],
      },
    ),
    (
      "self::SineOsc".to_string(),
      UnitSpec {
        type_path: "crate::modules::sine_osc::SineOsc".to_string(),
        call_style: CallStyle::Module {
          io_module_path: "crate::modules::sine_osc".to_string(),
        },
        inputs: vec![PortSpec::required("frequency", ValueKind::Float)],
        outputs: vec![PortSpec::required("out", ValueKind::Signal)],
      },
    ),
    (
      "self::AdsrEnvelope".to_string(),
      UnitSpec {
        type_path: "crate::modules::adsr_envelope::AdsrEnvelope".to_string(),
        call_style: CallStyle::Module {
          io_module_path: "crate::modules::adsr_envelope".to_string(),
        },
        inputs: vec![
          PortSpec::required("gate", ValueKind::Bool),
          PortSpec::optional("attack", ValueKind::Float),
          PortSpec::optional("decay", ValueKind::Float),
          PortSpec::optional("sustain", ValueKind::Float),
          PortSpec::optional("release", ValueKind::Float),
        ],
        outputs: vec![PortSpec::required("envelope", ValueKind::Signal)],
      },
    ),
    (
      "self::Multiply".to_string(),
      UnitSpec {
        type_path: "kahuna_std::operators::Multiply".to_string(),
        call_style: CallStyle::VariadicSlice,
        inputs: vec![PortSpec::variadic("in", ValueKind::Signal)],
        outputs: vec![PortSpec::required("out", ValueKind::Signal)],
      },
    ),
  ])
}
