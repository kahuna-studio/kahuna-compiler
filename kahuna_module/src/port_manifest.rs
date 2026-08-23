use crate::{PortDir, PortKind, PortSpec, Range, Rate};

#[derive(serde::Serialize)]
pub struct PortManifest {
  direction: &'static str,
  name: &'static str,
  required: Option<bool>,
  stream_type: &'static str,
  rate: Option<&'static str>,
  range: Option<(f64, f64)>,
  value_type: &'static str,
  enum_variants: Option<&'static [&'static str]>,
}

impl From<&PortSpec> for PortManifest {
  fn from(p: &PortSpec) -> Self {
    Self {
      direction: match p.direction { PortDir::Input => "input", PortDir::Output => "output" },
      name: p.name,
      required: match p.direction { PortDir::Input => Some(p.required), PortDir::Output => None },
      stream_type: match p.stream_type { PortKind::Signal => "signal", PortKind::Parameter => "parameter", PortKind::Event => "event" },
      rate: p.rate.map(|r| match r { Rate::AudioRate => "audio", Rate::ControlRate => "control" }),
      range: p.range.map(|r| match r { Range::Float(lo, hi) => (lo as f64, hi as f64), Range::Int(lo, hi) => (lo as f64, hi as f64) }),
      value_type: p.value_type.short_name(),
      enum_variants: p.value_type.enum_variants,
    }
  }
}
