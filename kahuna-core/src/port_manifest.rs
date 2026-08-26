use crate::{PortDirection, StreamKind, PortSpecification, Rate};

#[derive(serde::Serialize)]
pub struct PortManifest {
  direction: &'static str,
  name: &'static str,
  required: Option<bool>,
  stream_type: &'static str,
  rate: Option<&'static str>,
  value_type: &'static str,
  enum_variants: Option<&'static [&'static str]>,
}

impl From<&PortSpecification> for PortManifest {
  fn from(p: &PortSpecification) -> Self {
    Self {
      direction: match p.direction { PortDirection::Input => "input", PortDirection::Output => "output" },
      name: p.name,
      required: p.required,
      stream_type: match p.stream_kind { StreamKind::Signal => "signal", StreamKind::Parameter => "parameter", StreamKind::Event => "event" },
      rate: p.rate.map(|r| match r { Rate::AudioRate => "audio", Rate::ControlRate => "control" }),
      value_type: p.value_type.short_name(),
      enum_variants: p.value_type.enum_variants,
    }
  }
}
