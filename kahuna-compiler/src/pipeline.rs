//! The compiler's steps in order, from an instrument directory to generated source.

use std::fs;
use std::path::{Path, PathBuf};

use eyre::{Result, WrapErr};

use crate::compilation_steps::crate_writer::crate_writer::{crate_dir, write_crate};
use crate::compilation_steps::instrument_file_parser::instrument_metadata::InstrumentMetadata;
use crate::compilation_steps::library_resolver::library_resolver::{resolve_libraries, ResolvedLibraries};
use crate::compilation_steps::patch_code_generator::patch_code_generator::generate_patch_source;
use crate::compilation_steps::patch_file_parser::patch_graph::PatchGraph;

/// The file in the instrument directory that holds the patch.
pub const PATCH_FILE: &str = "patch.json";

const INSTRUMENT_FILE: &str = "instrument.toml";

#[derive(Debug)]
pub struct GeneratedPatch {
  /// The name from instrument.toml.
  pub instrument_name: String,
  /// The contents of `patch.rs`.
  pub source: String,
  /// Units patches can't use yet, with the reason. Warnings, not errors.
  pub unsupported_units: Vec<String>,
}

#[derive(Debug)]
pub struct GeneratedCrate {
  /// The name from instrument.toml.
  pub instrument_name: String,
  /// `{output_dir}/rust`: a crate that `cargo build` can build.
  pub crate_dir: PathBuf,
  /// Units patches can't use yet, with the reason. Warnings, not errors.
  pub unsupported_units: Vec<String>,
}

/// Reads `instrument.toml` and `patch.json` from the instrument directory, builds the unit catalog,
/// validates the patch against it, and returns the generated `patch.rs`.
///
/// The only files it writes are the generated project's `Cargo.toml` and `src/lib.rs` in
/// `output_dir/rust/`, which `cargo metadata` needs to locate the libraries. A patch that does
/// not parse leaves `output_dir` untouched.
pub fn generate_instrument_patch(instrument_dir: &Path, output_dir: &Path) -> Result<GeneratedPatch> {
  let compiled = compile_patch(instrument_dir, output_dir)?;
  Ok(GeneratedPatch {
    instrument_name: compiled.metadata.name,
    source: compiled.source,
    unsupported_units: compiled.resolved.unsupported_units,
  })
}

/// Everything `generate_instrument_patch` does, and then writes the complete crate to
/// `{output_dir}/rust/`: `Cargo.toml`, `src/lib.rs`, `src/patch.rs` and `src/main.rs`.
/// An existing crate is overwritten in place; its `target/` stays.
pub fn generate_instrument_crate(instrument_dir: &Path, output_dir: &Path) -> Result<GeneratedCrate> {
  let compiled = compile_patch(instrument_dir, output_dir)?;
  let crate_dir = crate_dir(output_dir);
  write_crate(&crate_dir, &compiled.metadata, &compiled.resolved.dependencies, &compiled.source)?;
  Ok(GeneratedCrate {
    instrument_name: compiled.metadata.name,
    crate_dir,
    unsupported_units: compiled.resolved.unsupported_units,
  })
}

struct CompiledPatch {
  metadata: InstrumentMetadata,
  source: String,
  resolved: ResolvedLibraries,
}

fn compile_patch(instrument_dir: &Path, output_dir: &Path) -> Result<CompiledPatch> {
  let metadata = InstrumentMetadata::from_file(&instrument_dir.join(INSTRUMENT_FILE))?;
  let patch_path = instrument_dir.join(PATCH_FILE);
  let graph = read_patch_graph(&patch_path)?;

  let resolved = resolve_libraries(&metadata, output_dir)?;
  let source = generate_patch_source(&graph, &resolved.catalog)
    .wrap_err_with(|| format!("Cannot compile {}", patch_path.display()))?;
  Ok(CompiledPatch { metadata, source, resolved })
}

fn read_patch_graph(path: &Path) -> Result<PatchGraph> {
  let contents = fs::read_to_string(path).wrap_err_with(|| format!("Cannot read {}", path.display()))?;
  contents.parse().wrap_err_with(|| format!("Cannot parse {}", path.display()))
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::compilation_steps::crate_writer::crate_writer::GENERATED_NOTICE;

  fn golden_synth() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/instruments/golden-synth")
  }

  fn output_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kahuna-pipeline-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
  }

  /// A copy of the golden instrument whose patch has been changed by `edit`.
  fn golden_synth_with_patch(name: &str, edit: impl Fn(&str) -> String) -> PathBuf {
    let dir = output_dir(&format!("{name}-instrument"));
    fs::create_dir_all(&dir).unwrap();
    fs::copy(golden_synth().join(INSTRUMENT_FILE), dir.join(INSTRUMENT_FILE)).unwrap();
    let patch = fs::read_to_string(golden_synth().join(PATCH_FILE)).unwrap();
    fs::write(dir.join(PATCH_FILE), edit(&patch)).unwrap();
    dir
  }

  /// prettyplease wraps long lines and then adds trailing commas, so compare without both.
  fn contains_code(source: &str, snippet: &str) -> bool {
    let strip = |text: &str| text.chars().filter(|c| !c.is_whitespace()).collect::<String>().replace(",)", ")");
    strip(source).contains(&strip(snippet))
  }

  #[test]
  fn generates_the_golden_patch() {
    let generated = generate_instrument_patch(&golden_synth(), &output_dir("golden")).unwrap();

    assert_eq!(generated.instrument_name, "golden-synth");
    let source = &generated.source;
    assert!(contains_code(source, "for voice in 0..self.poly_0_voices"), "{source}");
    assert!(contains_code(source, "self.state.merge_0__out[..frames].fill(0.0)"), "{source}");
    assert!(contains_code(source, "self.audio_out_0.process"), "{source}");
    assert!(contains_code(source, "kahuna_std::value_operators::arithmetic::multiply("), "{source}");
    assert!(
      contains_code(source, "<kahuna_std::modules::adsr_envelope::AdsrEnvelope as kahuna_core::KahunaModule>::Inputs::new("),
      "{source}"
    );
  }

  #[test]
  fn reports_unknown_units_with_node_id() {
    let instrument_dir = golden_synth_with_patch("unknown-unit", |patch| {
      patch.replace("kahuna-std/sine", "kahuna-std/does-not-exist")
    });

    let error = generate_instrument_patch(&instrument_dir, &output_dir("unknown-unit")).unwrap_err();

    let message = format!("{error:#}");
    assert!(message.contains("sine_0") && message.contains("kahuna-std/does-not-exist"), "{message}");
    assert!(message.contains("patch.json"), "{message}");
  }

  #[test]
  fn names_the_node_of_an_unparsable_patch_and_writes_nothing() {
    let instrument_dir = golden_synth_with_patch("bad-kind", |patch| patch.replace("\"kind\": \"merge\"", "\"kind\": \"blend\""));
    let output_dir = output_dir("bad-kind");

    let error = generate_instrument_patch(&instrument_dir, &output_dir).unwrap_err();

    let message = format!("{error:#}");
    assert!(message.contains("node 'merge_0'") && message.contains("blend"), "{message}");
    assert!(!output_dir.exists(), "output dir must stay untouched");
  }

  #[test]
  fn writes_the_complete_crate_for_the_golden_instrument() {
    let output_dir = output_dir("crate");

    let generated = generate_instrument_crate(&golden_synth(), &output_dir).unwrap();

    assert_eq!(generated.instrument_name, "golden-synth");
    assert_eq!(generated.crate_dir, output_dir.join("rust"));
    for file in ["Cargo.toml", "src/lib.rs", "src/patch.rs", "src/main.rs"] {
      let contents = fs::read_to_string(generated.crate_dir.join(file)).unwrap();
      assert!(contents.starts_with(&format!("{} {}", if file.ends_with(".toml") { "#" } else { "//" }, GENERATED_NOTICE)), "{file}");
    }
    let patch = fs::read_to_string(generated.crate_dir.join("src/patch.rs")).unwrap();
    assert!(contains_code(&patch, "self.audio_out_0.process"), "{patch}");
    let main = fs::read_to_string(generated.crate_dir.join("src/main.rs")).unwrap();
    assert!(main.contains("use golden_synth::patch::Patch;"), "{main}");
  }

  #[test]
  fn building_the_crate_again_leaves_the_files_and_the_build_cache_alone() {
    let output_dir = output_dir("rebuild");
    let crate_dir = generate_instrument_crate(&golden_synth(), &output_dir).unwrap().crate_dir;
    fs::create_dir_all(crate_dir.join("target")).unwrap();
    fs::write(crate_dir.join("target/cached"), "build output").unwrap();
    let patch_before = fs::read(crate_dir.join("src/patch.rs")).unwrap();

    generate_instrument_crate(&golden_synth(), &output_dir).unwrap();

    assert_eq!(fs::read(crate_dir.join("src/patch.rs")).unwrap(), patch_before, "the output is deterministic");
    assert_eq!(fs::read_to_string(crate_dir.join("target/cached")).unwrap(), "build output");
  }

  /// Slow: needs the toolchain's dependencies (cpal, midir) and, on Linux, the ALSA headers
  /// (`libasound2-dev`). Run with `cargo test -- --ignored`.
  #[test]
  #[ignore = "runs cargo check on the generated crate"]
  fn the_generated_crate_of_the_golden_instrument_passes_cargo_check() {
    let generated = generate_instrument_crate(&golden_synth(), &output_dir("cargo-check")).unwrap();
    let manifest = generated.crate_dir.join("Cargo.toml");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());

    let check = std::process::Command::new(&cargo)
      .args(["check", "--all-targets", "--manifest-path"])
      .arg(&manifest)
      .output()
      .unwrap();
    assert!(
      check.status.success(),
      "cargo check failed for {}:\n{}",
      manifest.display(),
      String::from_utf8_lossy(&check.stderr)
    );

    // units in kahuna-std implement the traits of kahuna-core: there must be exactly one copy of it
    let metadata = std::process::Command::new(&cargo)
      .args(["metadata", "--format-version", "1", "--manifest-path"])
      .arg(&manifest)
      .output()
      .unwrap();
    let metadata: serde_json::Value = serde_json::from_slice(&metadata.stdout).unwrap();
    let copies = metadata["packages"].as_array().unwrap().iter().filter(|package| package["name"] == "kahuna-core").count();
    assert_eq!(copies, 1, "kahuna-core must resolve to one path");
  }

  #[test]
  fn reports_a_missing_patch_file() {
    let instrument_dir = output_dir("no-patch-instrument");
    fs::create_dir_all(&instrument_dir).unwrap();
    fs::copy(golden_synth().join(INSTRUMENT_FILE), instrument_dir.join(INSTRUMENT_FILE)).unwrap();

    let error = generate_instrument_patch(&instrument_dir, &output_dir("no-patch")).unwrap_err();

    assert!(format!("{error:#}").contains("patch.json"), "{error:#}");
  }
}
