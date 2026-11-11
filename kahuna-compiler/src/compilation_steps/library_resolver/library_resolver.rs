//! Finds the unit libraries an instrument uses and builds a unit catalog from their manifests.
//!
//! The compiler writes the generated `{instrument}` crate's Cargo.toml with every library as a
//! dependency, then asks `cargo metadata` where Cargo put each of them. Nothing is compiled.
//!
//! For now an instrument has two sources of units: `kahuna-std`, which every instrument gets without
//! listing it, and the instrument's own library in `units/`, if it has one. `[libraries]` in
//! instrument.toml is rejected until other libraries are supported.

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

/// The standard library: a sibling of this crate in the toolchain repository, and a dependency of
/// every generated crate. It is both the crate name and what patches write before the slash.
const STD_LIBRARY: &str = "kahuna-std";

/// A unit library as the generated crate depends on it.
#[derive(Debug, Clone, PartialEq)]
struct LibraryDependency {
  /// What patches write before the slash: `kahuna-std`, or `self` for the instrument's own library.
  reference: String,
  /// The dependency key in the generated Cargo.toml.
  key: String,
  /// The Cargo dependency specification, with absolute paths.
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
  ensure!(
    metadata.libraries.is_empty(),
    "[libraries] in instrument.toml is not supported yet (found: {}); remove it. \
     Every instrument can use the units of {STD_LIBRARY} as `{STD_LIBRARY}/{{unit id}}`",
    metadata.libraries.keys().cloned().collect::<Vec<_>>().join(", ")
  );

  let mut dependencies = vec![path_dependency(STD_LIBRARY, STD_LIBRARY, &std_library_dir())?];

  let own_units_dir = instrument_dir.join(OWN_LIBRARY_DIR);
  if own_units_dir.join("Cargo.toml").is_file() {
    // Read fresh every time, so renaming the crate never breaks a patch.
    let key = package_name(&own_units_dir)?;
    ensure!(
      key != STD_LIBRARY,
      "The instrument's own library in {OWN_LIBRARY_DIR}/ is named '{STD_LIBRARY}', which is the standard library; rename its package"
    );
    dependencies.push(path_dependency(SELF_LIBRARY, &key, &own_units_dir)?);
  }

  Ok(dependencies)
}

/// Until the compiler is installed separately from the toolchain repository, `kahuna-std` is found
/// next to the compiler's own crate.
fn std_library_dir() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(STD_LIBRARY)
}

/// The generated crate lives elsewhere, so the path to the library is made absolute.
fn path_dependency(reference: &str, key: &str, directory: &Path) -> Result<LibraryDependency> {
  let path = absolute(directory)?;
  Ok(LibraryDependency {
    reference: reference.to_string(),
    key: key.to_string(),
    specification: toml::Value::Table(toml::Table::from_iter([("path".to_string(), path.into())])),
  })
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
  let call_style = match unit.kind {
    UnitKind::Module => CallStyle::Module,
    UnitKind::Converter => CallStyle::Converter,
    UnitKind::ValueOperator => {
      ensure!(
        unit.type_parameters.is_empty(),
        "operators with type parameters ({}) can't be used in patches yet",
        unit.type_parameters.join(", ")
      );
      CallStyle::ValueOperator { fold: unit.fold }
    }
  };
  let port_spec = |port: &PortEntry| match unit.kind {
    UnitKind::Module | UnitKind::Converter => struct_unit_port_spec(port),
    UnitKind::ValueOperator => operator_port_spec(port, unit.fold),
  };

  Ok(UnitSpec {
    type_path: format!("{extern_name}::{}", unit.rust_path),
    call_style,
    inputs: inputs.into_iter().map(port_spec).collect::<Result<_>>()?,
    outputs: outputs.into_iter().map(port_spec).collect::<Result<_>>()?,
  })
}

/// Modules and converters.
fn struct_unit_port_spec(port: &PortEntry) -> Result<PortSpec> {
  let stream_type = port
    .stream_type
    .ok_or_else(|| eyre!("port '{}' has no stream type", port.name))?;
  let kind = match (stream_type, port.value_type.as_str()) {
    (StreamType::Event, _) => ValueKind::Events,
    (StreamType::Signal, _) => ValueKind::Signal,
    (StreamType::Parameter, "bool") => ValueKind::Bool,
    (StreamType::Parameter, "f32" | "Unipolar" | "Bipolar") => ValueKind::Float,
    (StreamType::Parameter, other) => bail!("parameter port '{}' has type '{other}', which patches don't support yet", port.name),
  };
  // Only parameters with a default can stay unconnected; their constructor argument is an Option.
  let arity = match (port.direction, stream_type, port.required) {
    (PortDirection::Input, StreamType::Parameter, Some(false)) => Arity::Optional,
    _ => Arity::Required,
  };
  Ok(PortSpec {
    name: port.name.clone(),
    kind,
    arity,
    min_connections: 0,
    default: None,
  })
}

/// So far the generator handles value operators whose ports are all `f32`, with fixed inputs or
/// one folded input. Their ports carry an audio signal or a parameter, whichever is connected.
fn operator_port_spec(port: &PortEntry, fold: bool) -> Result<PortSpec> {
  ensure!(
    port.value_type == "f32",
    "port '{}' has type '{}'; patches support value operators with f32 ports only yet",
    port.name,
    port.value_type
  );
  let arity = match (port.direction, port.min_connections, port.required) {
    (PortDirection::Input, Some(_), _) => {
      ensure!(fold, "port '{}' takes a list; patches support folded variadic inputs only yet", port.name);
      Arity::Variadic
    }
    (PortDirection::Input, None, Some(false)) => Arity::Optional,
    _ => Arity::Required,
  };
  Ok(PortSpec {
    name: port.name.clone(),
    kind: ValueKind::Number,
    arity,
    min_connections: port.min_connections.unwrap_or(0),
    default: port.default.as_ref().and_then(|default| default.as_f64()).map(|default| default as f32),
  })
}

/// Shared by the tests of other compilation steps.
#[cfg(test)]
pub mod test_support {
  use std::sync::OnceLock;

  use super::*;

  /// The catalog of an instrument that has no units of its own: `kahuna-std` only.
  /// Resolved once per test run, since it runs `cargo metadata`.
  pub fn std_catalog() -> &'static StaticCatalog {
    static CATALOG: OnceLock<StaticCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
      let directory = std::env::temp_dir().join(format!("kahuna-std-catalog-{}", std::process::id()));
      let metadata = InstrumentMetadata::from_str("name = \"std-catalog\"\nvendor = \"Kahuna Studio\"\nversion = \"0.1.0\"\n").unwrap();
      let resolved = resolve_libraries(&directory, &metadata, &directory.join("generated")).unwrap();
      resolved.catalog
    })
  }
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

  fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kahuna-library-resolver-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
  }

  /// An instrument directory holding only `instrument.toml` and the given files.
  fn instrument_with_files(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = scratch_dir(&format!("{name}-instrument"));
    let instrument_toml = "name = \"bare-synth\"\nvendor = \"Kahuna Studio\"\nversion = \"0.1.0\"\n";
    for (path, contents) in [("instrument.toml", instrument_toml)].into_iter().chain(files.iter().copied()) {
      fs::create_dir_all(dir.join(path).parent().unwrap()).unwrap();
      fs::write(dir.join(path), contents).unwrap();
    }
    dir
  }

  #[test]
  fn instrument_without_libraries_gets_the_std_units() {
    let instrument_dir = instrument_with_files("bare", &[]);

    let resolved = resolve_libraries(&instrument_dir, &metadata(&instrument_dir), &scratch_dir("bare")).unwrap();
    let catalog = &resolved.catalog;

    for unit in [
      "kahuna-std/sine-osc",
      "kahuna-std/adsr-envelope",
      "kahuna-std/multiply",
      "kahuna-std/midi-event-to-frequency-parameter",
      "kahuna-std/midi-event-to-gate-parameter",
    ] {
      assert!(catalog.find(unit).is_some(), "{unit} is missing from the catalog");
    }
    assert!(catalog.find("self/sine-osc").is_none(), "no units/ directory, so no own units");
  }

  #[test]
  fn finds_std_and_own_units() {
    let instrument_dir = fixture_instrument();

    let resolved = resolve_libraries(&instrument_dir, &metadata(&instrument_dir), &scratch_dir("found")).unwrap();
    let catalog = &resolved.catalog;

    let sine = catalog.find("self/sine-osc").expect("own unit");
    assert_eq!(sine.type_path, "test_synth_units::oscillators::SineOsc");

    let envelope = catalog.find("kahuna-std/adsr-envelope").expect("std module");
    assert_eq!(envelope.type_path, "kahuna_std::modules::adsr_envelope::AdsrEnvelope");
    assert_eq!(envelope.inputs[0].arity, Arity::Required);
    assert_eq!(envelope.inputs[1].arity, Arity::Optional);
    assert_eq!(envelope.outputs[0].kind, ValueKind::Signal);

    let gate = catalog.find("kahuna-std/midi-event-to-gate-parameter").expect("converter");
    assert_eq!(gate.type_path, "kahuna_std::converters::midi_event_to_gate_parameter::MidiEventToGateParameter");
    assert!(matches!(gate.call_style, CallStyle::Converter));

    let multiply = catalog.find("kahuna-std/multiply").expect("value operator");
    assert!(matches!(multiply.call_style, CallStyle::ValueOperator { fold: true }));
    assert_eq!(multiply.inputs[0].kind, ValueKind::Number);
    assert_eq!(multiply.inputs[0].arity, Arity::Variadic);
    assert_eq!(multiply.inputs[0].min_connections, 2);
    let scale_offset = catalog.find("kahuna-std/scale-offset").expect("value operator");
    assert_eq!(scale_offset.inputs[1].default, Some(1.0));
  }

  #[test]
  fn reports_units_patches_cannot_use_yet_as_warnings() {
    let instrument_dir = instrument_with_files("unsupported", &[]);

    let resolved = resolve_libraries(&instrument_dir, &metadata(&instrument_dir), &scratch_dir("unsupported")).unwrap();

    let unsupported = resolved
      .unsupported_units
      .iter()
      .map(|report| report.split(':').next().unwrap())
      .collect::<Vec<_>>();
    assert_eq!(
      unsupported,
      [
        "kahuna-std/greater",
        "kahuna-std/greater-or-equal",
        "kahuna-std/less",
        "kahuna-std/less-or-equal",
        "kahuna-std/select",
      ]
    );
    assert!(resolved.catalog.find("kahuna-std/select").is_none());
  }

  #[test]
  fn writes_generated_crate_with_absolute_paths_in_its_own_workspace() {
    let instrument_dir = fixture_instrument();
    let output_dir = scratch_dir("generated");

    resolve_libraries(&instrument_dir, &metadata(&instrument_dir), &output_dir).unwrap();

    let cargo_toml: toml::Table = fs::read_to_string(output_dir.join("test-synth/Cargo.toml")).unwrap().parse().unwrap();
    assert!(cargo_toml.contains_key("workspace"));
    let std_path = cargo_toml["dependencies"][STD_LIBRARY]["path"].as_str().unwrap();
    assert_eq!(Path::new(std_path), std_library_dir().canonicalize().unwrap());
    let units_path = cargo_toml["dependencies"]["test-synth-units"]["path"].as_str().unwrap();
    assert_eq!(Path::new(units_path), instrument_dir.join("units").canonicalize().unwrap());
  }

  #[test]
  fn rejects_libraries_before_writing_anything() {
    let instrument_dir = fixture_instrument();
    let output_dir = scratch_dir("rejected");
    let mut metadata = metadata(&instrument_dir);
    metadata.libraries.insert(
      "envelopes".to_string(),
      toml::Value::Table(toml::Table::from_iter([("path".to_string(), "../envelopes".into())])),
    );

    let error = resolve_libraries(&instrument_dir, &metadata, &output_dir).unwrap_err();

    let message = format!("{error}");
    assert!(message.contains("[libraries]") && message.contains("not supported yet"), "{message}");
    assert!(message.contains("envelopes"), "{message}");
    assert!(!output_dir.exists(), "nothing is generated for an invalid instrument");
  }

  #[test]
  fn rejects_own_library_named_like_the_std_library() {
    let instrument_dir = instrument_with_files(
      "std-name",
      &[
        ("units/Cargo.toml", "[package]\nname = \"kahuna-std\"\nversion = \"0.1.0\"\nedition = \"2024\"\n"),
        ("units/src/lib.rs", ""),
      ],
    );

    let error = resolve_libraries(&instrument_dir, &metadata(&instrument_dir), &scratch_dir("std-name")).unwrap_err();

    assert!(format!("{error}").contains("standard library"), "{error}");
  }

  #[test]
  fn reports_own_library_without_manifest() {
    let instrument_dir = instrument_with_files(
      "no-manifest",
      &[
        ("units/Cargo.toml", "[package]\nname = \"bare-units\"\nversion = \"0.1.0\"\nedition = \"2024\"\n"),
        ("units/src/lib.rs", ""),
      ],
    );

    let error = resolve_libraries(&instrument_dir, &metadata(&instrument_dir), &scratch_dir("no-manifest")).unwrap_err();

    assert!(format!("{error}").contains("kahuna-library.json not found"), "{error}");
  }
}
