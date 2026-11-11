//! The compiler's steps in order, from an instrument directory to generated source.

use std::fs;
use std::path::Path;

use eyre::{Result, WrapErr};

use crate::compilation_steps::instrument_file_parser::instrument_metadata::InstrumentMetadata;
use crate::compilation_steps::library_resolver::library_resolver::resolve_libraries;
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

/// Reads `instrument.toml` and `patch.json` from the instrument directory, builds the unit catalog,
/// validates the patch against it, and returns the generated `patch.rs`.
///
/// The only file it writes is the generated crate's `Cargo.toml` in `output_dir/{name}/`, which
/// `cargo metadata` needs to locate the libraries. A patch that does not parse leaves `output_dir`
/// untouched.
pub fn generate_instrument_patch(instrument_dir: &Path, output_dir: &Path) -> Result<GeneratedPatch> {
  let metadata = InstrumentMetadata::from_file(&instrument_dir.join(INSTRUMENT_FILE))?;
  let patch_path = instrument_dir.join(PATCH_FILE);
  let graph = read_patch_graph(&patch_path)?;

  let resolved = resolve_libraries(instrument_dir, &metadata, output_dir)?;
  let source = generate_patch_source(&graph, &resolved.catalog)
    .wrap_err_with(|| format!("Cannot compile {}", patch_path.display()))?;

  Ok(GeneratedPatch {
    instrument_name: metadata.name,
    source,
    unsupported_units: resolved.unsupported_units,
  })
}

fn read_patch_graph(path: &Path) -> Result<PatchGraph> {
  let contents = fs::read_to_string(path).wrap_err_with(|| format!("Cannot read {}", path.display()))?;
  contents.parse().wrap_err_with(|| format!("Cannot parse {}", path.display()))
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;

  use super::*;

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
  fn reports_a_missing_patch_file() {
    let instrument_dir = output_dir("no-patch-instrument");
    fs::create_dir_all(&instrument_dir).unwrap();
    fs::copy(golden_synth().join(INSTRUMENT_FILE), instrument_dir.join(INSTRUMENT_FILE)).unwrap();

    let error = generate_instrument_patch(&instrument_dir, &output_dir("no-patch")).unwrap_err();

    assert!(format!("{error:#}").contains("patch.json"), "{error:#}");
  }
}
