use crate::{PortDir, PortKind, PortSpec, Range, Rate};

#[derive(serde::Serialize)]
pub struct PortManifest {
  name: &'static str,
  dir: &'static str,
  kind: &'static str,
  rate: Option<&'static str>,
  range: Option<(f64, f64)>, // or your own repr
  type_name: &'static str,
  enum_variants: Option<&'static [&'static str]>,
}

impl From<&PortSpec> for PortManifest {
  fn from(p: &PortSpec) -> Self {
    Self {
      name: p.name,
      dir: match p.dir { PortDir::Input => "input", PortDir::Output => "output" },
      kind: match p.kind { PortKind::Signal => "signal", PortKind::Parameter => "parameter", PortKind::Event => "event" },
      rate: p.rate.map(|r| match r { Rate::AudioRate => "audio", Rate::ControlRate => "control" }),
      range: p.range.map(|r| match r { Range::Float(lo, hi) => (lo as f64, hi as f64), Range::Int(lo, hi) => (lo as f64, hi as f64) }),
      type_name: p.port_type.short_name(),
      enum_variants: p.port_type.enum_variants,
    }
  }
}
