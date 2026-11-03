use serde::{Deserialize, Serialize};

/// Lives at the root of a unit crate, next to its Cargo.toml.
pub const UNIT_MANIFEST_FILE: &str = "kahuna-units.json";

pub const FORMAT_VERSION: u32 = 1;

/// Describes every unit of one crate. Which crate it belongs to is not stored: that follows
/// from where the file lives, which Cargo already knows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnitManifest {
  pub format_version: u32,
  pub units: Vec<UnitEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnitEntry {
  /// Unique within the crate; what patches refer to.
  pub id: String,
  pub kind: UnitKind,
  /// Relative to the crate root, without the crate name.
  pub rust_path: String,
  pub ports: Vec<PortEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnitKind {
  Module,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortEntry {
  pub direction: PortDirection,
  pub name: String,
  pub stream_type: StreamType,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub rate: Option<Rate>,
  /// The value type as written in the unit's source, e.g. `f32` or `Unipolar`.
  pub value_type: String,
  /// Inputs only.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub required: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub default: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PortDirection {
  Input,
  Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamType {
  Signal,
  Parameter,
  Event,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rate {
  Audio,
  Control,
}
