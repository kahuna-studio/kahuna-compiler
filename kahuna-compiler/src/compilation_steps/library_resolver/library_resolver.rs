//! Finds the unit libraries an instrument uses and builds a unit catalog from their manifests.
//!
//! The compiler writes the generated `{instrument}` crate's Cargo.toml with every library as a
//! dependency, then asks `cargo metadata` where Cargo put each of them. Nothing is compiled.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use eyre::{bail, ensure, eyre, Result, WrapErr};
use serde_json::Value as Json;

use crate::compilation_steps::instrument_file_parser::instrument_metadata::{InstrumentMetadata, SELF_LIBRARY};
use crate::compilation_steps::library_manifest_generator::library_manifest::{
  LibraryManifest, PortDirection, PortEntry, StreamType, UnitEntry, UnitKind, FORMAT_VERSION, LIBRARY_MANIFEST_FILE,
};
use crate::compilation_steps::patch_code_generator::catalog::{Arity, CallStyle, PortSpec, StaticCatalog, UnitSpec, ValueKind};

/// Where generated crates go, inside the instrument directory.
pub const GENERATED_DIR: &str = ".kahuna";

/// The instrument's own library, found automatically.
const OWN_LIBRARY_DIR: &str = "units";

/// A unit library as the generated crate depends on it.
#[derive(Debug, Clone, PartialEq)]
struct LibraryDependency {
  /// What patches write before the slash: the key from `[libraries]`, or `self`.
  reference: String,
  /// The dependency key in the generated Cargo.toml.
  key: String,
  /// The Cargo dependency specification, with relative paths made absolute.
  specification: toml::Value,
}

/// Where `cargo metadata` found a library.
struct LocatedLibrary {
  /// The name generated code uses for the library, e.g. `kahuna_std`.
  extern_name: String,
  directory: PathBuf,
}

#[derive(Debug)]
pub struct ResolvedLibraries {
  /// Every usable unit, keyed `{library}/{unit id}`, e.g. `kahuna-std/adsr-envelope`.
  pub catalog: StaticCatalog,
  /// Units left out of the catalog because patches can't use them yet, with the reason.
  pub unsupported_units: Vec<String>,
}

/// Writes `{output_dir}/{instrument}/Cargo.toml`, locates every library, and reads their manifests.
pub fn resolve_libraries(instrument_dir: &Path, metadata: &InstrumentMetadata, output_dir: &Path) -> Result<ResolvedLibraries> {
  let dependencies = library_dependencies(instrument_dir, metadata)?;

  let crate_dir = output_dir.join(&metadata.name);
  write_generated_crate(&crate_dir, metadata, &dependencies)?;
  let located = locate_dependencies(&crate_dir, &dependencies)?;

  let mut entries = Vec::new();
  let mut unsupported_units = Vec::new();
  for (dependency, located) in dependencies.iter().zip(&located) {
    let manifest = read_library_manifest(&located.directory)?;
    for unit in &manifest.units {
      let reference = format!("{}/{}", dependency.reference, unit.id);
      // One unusable unit must not make the rest of its library unusable.
      match unit_spec(unit, &located.extern_name) {
        Ok(spec) => entries.push((reference, spec)),
        Err(reason) => unsupported_units.push(format!("{reference}: {reason}")),
      }
    }
  }

  Ok(ResolvedLibraries {
    catalog: StaticCatalog::new(entries),
    unsupported_units,
  })
}

fn library_dependencies(instrument_dir: &Path, metadata: &InstrumentMetadata) -> Result<Vec<LibraryDependency>> {
  let mut dependencies = metadata
    .libraries
    .iter()
    .map(|(key, specification)| {
      Ok(LibraryDependency {
        reference: key.clone(),
        key: key.clone(),
        specification: with_absolute_path(specification, instrument_dir)
          .wrap_err_with(|| format!("[libraries] entry '{key}'"))?,
      })
    })
    .collect::<Result<Vec<_>>>()?;

  let self_units_dir = instrument_dir.join(OWN_LIBRARY_DIR);
  if self_units_dir.join("Cargo.toml").is_file() {
    // Read fresh every time, so renaming the crate never breaks a patch.
    let key = package_name(&self_units_dir)?;
    ensure!(
      !metadata.libraries.contains_key(&key),
      "[libraries] lists '{key}', which is the instrument's own library in {OWN_LIBRARY_DIR}/; remove it from [libraries]"
    );
    let path = absolute(&self_units_dir)?;
    dependencies.push(LibraryDependency {
      reference: SELF_LIBRARY.to_string(),
      key,
      specification: toml::Value::Table(toml::Table::from_iter([("path".to_string(), path.into())])),
    });
  }

  Ok(dependencies)
}

/// Relative `path`s in `[libraries]` are relative to the instrument directory; the generated crate
/// lives elsewhere, so they are made absolute.
fn with_absolute_path(specification: &toml::Value, instrument_dir: &Path) -> Result<toml::Value> {
  let mut specification = specification.clone();
  if let Some(table) = specification.as_table_mut()
    && let Some(path) = table.get("path").and_then(|path| path.as_str())
  {
    let path = absolute(&instrument_dir.join(path))?;
    table.insert("path".to_string(), path.into());
  }
  Ok(specification)
}

fn absolute(path: &Path) -> Result<String> {
  let path = path
    .canonicalize()
    .wrap_err_with(|| format!("Library directory not found: {}", path.display()))?;
  Ok(path.to_string_lossy().into_owned())
}

fn package_name(crate_dir: &Path) -> Result<String> {
  let cargo_toml_path = crate_dir.join("Cargo.toml");
  let cargo_toml: toml::Table = fs::read_to_string(&cargo_toml_path)
    .wrap_err_with(|| format!("Cannot read {}", cargo_toml_path.display()))?
    .parse()
    .wrap_err_with(|| format!("Cannot parse {}", cargo_toml_path.display()))?;
  cargo_toml
    .get("package")
    .and_then(|package| package.get("name"))
    .and_then(|name| name.as_str())
    .map(str::to_string)
    .ok_or_else(|| eyre!("{} has no package name", cargo_toml_path.display()))
}

fn write_generated_crate(crate_dir: &Path, metadata: &InstrumentMetadata, dependencies: &[LibraryDependency]) -> Result<()> {
  let package = toml::Table::from_iter([
    ("name".to_string(), metadata.name.clone().into()),
    ("version".to_string(), metadata.version.clone().into()),
    ("edition".to_string(), "2024".into()),
    ("publish".to_string(), false.into()),
  ]);
  let dependency_table = dependencies
    .iter()
    .map(|dependency| (dependency.key.clone(), dependency.specification.clone()))
    .collect::<toml::Table>();
  let cargo_toml = toml::Table::from_iter([
    ("package".to_string(), package.into()),
    // Its own workspace, even when the instrument sits inside another one.
    ("workspace".to_string(), toml::Table::new().into()),
    ("dependencies".to_string(), dependency_table.into()),
  ]);

  let source_dir = crate_dir.join("src");
  fs::create_dir_all(&source_dir).wrap_err_with(|| format!("Cannot create {}", source_dir.display()))?;
  fs::write(
    crate_dir.join("Cargo.toml"),
    format!("# Generated by kahuna-compiler from instrument.toml. Do not edit.\n\n{}", toml::to_string(&cargo_toml)?),
  )?;

  // Cargo needs a target; the patch code generator will replace this.
  let library_root = source_dir.join("lib.rs");
  if !library_root.exists() {
    fs::write(&library_root, "// Generated by kahuna-compiler.\n")?;
  }
  Ok(())
}

/// Returns one location per dependency, in the same order.
fn locate_dependencies(crate_dir: &Path, dependencies: &[LibraryDependency]) -> Result<Vec<LocatedLibrary>> {
  let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
  let output = Command::new(cargo)
    .args(["metadata", "--format-version", "1", "--manifest-path"])
    .arg(crate_dir.join("Cargo.toml"))
    .output()
    .wrap_err("Cannot run cargo metadata")?;
  if !output.status.success() {
    bail!("cargo metadata failed:\n{}", String::from_utf8_lossy(&output.stderr).trim());
  }

  let metadata: Json = serde_json::from_slice(&output.stdout).wrap_err("Cannot parse cargo metadata output")?;
  let packages: HashMap<&str, &Json> = metadata["packages"]
    .as_array()
    .into_iter()
    .flatten()
    .filter_map(|package| Some((package["id"].as_str()?, package)))
    .collect();
  let root_id = metadata["resolve"]["root"].as_str().ok_or_else(|| eyre!("cargo metadata reports no root package"))?;
  let root_node = metadata["resolve"]["nodes"]
    .as_array()
    .into_iter()
    .flatten()
    .find(|node| node["id"] == root_id)
    .ok_or_else(|| eyre!("cargo metadata reports no dependencies for the generated crate"))?;

  dependencies
    .iter()
    .map(|dependency| {
      // The declared dependency names the package; the resolved one says which version and where.
      let declared = packages[root_id]["dependencies"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|declared| declared["rename"].as_str().or(declared["name"].as_str()) == Some(&dependency.key))
        .ok_or_else(|| eyre!("cargo metadata does not list the dependency '{}'", dependency.key))?;
      let resolved = root_node["deps"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|resolved| {
          resolved["pkg"]
            .as_str()
            .and_then(|id| packages.get(id))
            .is_some_and(|package| package["name"] == declared["name"])
        })
        .ok_or_else(|| eyre!("cargo metadata did not resolve the dependency '{}'", dependency.key))?;

      let package = packages[resolved["pkg"].as_str().unwrap_or_default()];
      let manifest_path = Path::new(package["manifest_path"].as_str().unwrap_or_default());
      Ok(LocatedLibrary {
        extern_name: resolved["name"].as_str().unwrap_or_default().to_string(),
        directory: manifest_path.parent().unwrap_or(manifest_path).to_path_buf(),
      })
    })
    .collect()
}

fn read_library_manifest(crate_dir: &Path) -> Result<LibraryManifest> {
  let path = crate_dir.join(LIBRARY_MANIFEST_FILE);
  let contents = fs::read_to_string(&path).map_err(|_| {
    eyre!(
      "{} not found; create it with `kahuna-compiler manifest {}`",
      path.display(),
      crate_dir.display()
    )
  })?;
  let manifest: LibraryManifest = serde_json::from_str(&contents).wrap_err_with(|| format!("Cannot parse {}", path.display()))?;
  ensure!(
    manifest.format_version == FORMAT_VERSION,
    "{} has format version {}, but this compiler reads version {FORMAT_VERSION}",
    path.display(),
    manifest.format_version
  );
  Ok(manifest)
}

fn unit_spec(unit: &UnitEntry, extern_name: &str) -> Result<UnitSpec> {
  let (inputs, outputs): (Vec<&PortEntry>, Vec<&PortEntry>) =
    unit.ports.iter().partition(|port| port.direction == PortDirection::Input);

  Ok(UnitSpec {
    type_path: format!("{extern_name}::{}", unit.rust_path),
    call_style: match unit.kind {
      UnitKind::Module => CallStyle::Module,
      UnitKind::Converter => CallStyle::Converter,
    },
    inputs: inputs.into_iter().map(port_spec).collect::<Result<_>>()?,
    outputs: outputs.into_iter().map(port_spec).collect::<Result<_>>()?,
  })
}

fn port_spec(port: &PortEntry) -> Result<PortSpec> {
  let kind = match (port.stream_type, port.value_type.as_str()) {
    (StreamType::Event, _) => ValueKind::Events,
    (StreamType::Signal, _) => ValueKind::Signal,
    (StreamType::Parameter, "bool") => ValueKind::Bool,
    (StreamType::Parameter, "f32" | "Unipolar" | "Bipolar") => ValueKind::Float,
    (StreamType::Parameter, other) => bail!("parameter port '{}' has type '{other}', which patches don't support yet", port.name),
  };
  // Only parameters with a default can stay unconnected; their constructor argument is an Option.
  let arity = match (port.direction, port.stream_type, port.required) {
    (PortDirection::Input, StreamType::Parameter, Some(false)) => Arity::Optional,
    _ => Arity::Required,
  };
  Ok(PortSpec { name: port.name.clone(), kind, arity })
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::compilation_steps::patch_code_generator::catalog::UnitCatalog;

  fn fixture_instrument() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/instruments/test-synth")
  }

  fn metadata(instrument_dir: &Path) -> InstrumentMetadata {
    InstrumentMetadata::from_str(&fs::read_to_string(instrument_dir.join("instrument.toml")).unwrap()).unwrap()
  }

  fn output_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kahuna-library-resolver-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
  }

  #[test]
  fn finds_own_and_listed_units() {
    let instrument_dir = fixture_instrument();

    let resolved = resolve_libraries(&instrument_dir, &metadata(&instrument_dir), &output_dir("found")).unwrap();
    let catalog = &resolved.catalog;

    let sine = catalog.find("self/sine-osc").expect("own unit");
    assert_eq!(sine.type_path, "test_synth_units::oscillators::SineOsc");

    let envelope = catalog.find("envelopes/adsr-envelope").expect("listed unit");
    assert_eq!(envelope.type_path, "envelopes::envelopes::adsr_envelope::AdsrEnvelope");
    assert_eq!(envelope.inputs[0].arity, Arity::Required);
    assert_eq!(envelope.inputs[1].arity, Arity::Optional);
    assert_eq!(envelope.outputs[0].kind, ValueKind::Signal);

    let gate = catalog.find("kahuna-std/midi-event-to-gate-parameter").expect("converter");
    assert_eq!(gate.type_path, "kahuna_std::mapping::midi_event_to_gate_parameter::MidiEventToGateParameter");
    assert!(matches!(gate.call_style, CallStyle::Converter));

    assert!(catalog.find("envelopes/gate-follower").is_none());
    assert_eq!(resolved.unsupported_units.len(), 1);
    assert!(resolved.unsupported_units[0].starts_with("envelopes/gate-follower: "), "{:?}", resolved.unsupported_units);
  }

  #[test]
  fn writes_generated_crate_with_absolute_paths_in_its_own_workspace() {
    let instrument_dir = fixture_instrument();
    let output_dir = output_dir("generated");

    resolve_libraries(&instrument_dir, &metadata(&instrument_dir), &output_dir).unwrap();

    let cargo_toml: toml::Table = fs::read_to_string(output_dir.join("test-synth/Cargo.toml")).unwrap().parse().unwrap();
    assert!(cargo_toml.contains_key("workspace"));
    let units_path = cargo_toml["dependencies"]["test-synth-units"]["path"].as_str().unwrap();
    assert_eq!(Path::new(units_path), instrument_dir.join("units").canonicalize().unwrap());
  }

  #[test]
  fn reports_libraries_without_manifest() {
    let instrument_dir = fixture_instrument();
    let mut metadata = metadata(&instrument_dir);
    let crate_without_manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../kahuna-core");
    metadata.libraries.insert(
      "kahuna-core".to_string(),
      toml::Value::Table(toml::Table::from_iter([(
        "path".to_string(),
        crate_without_manifest.to_string_lossy().into_owned().into(),
      )])),
    );

    let error = resolve_libraries(&instrument_dir, &metadata, &output_dir("missing")).unwrap_err();

    assert!(format!("{error}").contains("kahuna-library.json not found"), "{error}");
  }
}
