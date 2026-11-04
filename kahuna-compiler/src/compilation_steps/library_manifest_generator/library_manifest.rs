use serde::{Deserialize, Serialize};

/// Lives at the root of a unit library, next to its Cargo.toml.
pub const LIBRARY_MANIFEST_FILE: &str = "kahuna-library.json";

pub const FORMAT_VERSION: u32 = 1;

/// Describes every unit of one library. The library's name is not stored: it follows from
/// where the file lives, which Cargo already knows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibraryManifest {
  pub format_version: u32,
  pub units: Vec<UnitEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnitEntry {
  /// Unique within the library; what patches refer to.
  pub id: String,
  pub kind: UnitKind,
  /// Relative to the library's root module, without the library's name.
  pub rust_path: String,
  /// Value operators only: each ties together all ports whose `value_type` is its name.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub type_parameters: Vec<String>,
  /// Value operators only: the function takes two values and is applied pairwise over all
  /// connections of the one variadic input.
  #[serde(default, skip_serializing_if = "is_false")]
  pub fold: bool,
  pub ports: Vec<PortEntry>,
}

fn is_false(value: &bool) -> bool {
  !value
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UnitKind {
  Module,
  Converter,
  ValueOperator,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortEntry {
  pub direction: PortDirection,
  pub name: String,
  /// Absent for value operators: there it follows from what is connected.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub stream_type: Option<StreamType>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub rate: Option<Rate>,
  /// The value type as written in the unit's source, e.g. `f32`, `Unipolar`, or a type parameter like `T`.
  pub value_type: String,
  /// Present on variadic inputs only: how many connections they need at least.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub min_connections: Option<usize>,
  /// Single inputs only.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub required: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub default: Option<serde_json::Value>,
  /// A hint for the UI, `[low, high]`; values outside are still valid.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub range: Option<[f64; 2]>,
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
