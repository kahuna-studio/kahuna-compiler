//! The compiler's steps in order, from an instrument directory to generated source.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use eyre::{Result, WrapErr};

use crate::compilation_steps::cargo_runner::cargo_runner::{build_release, cargo_program};
use crate::compilation_steps::crate_writer::crate_writer::{clean_project, crate_dir, write_crate};
use crate::compilation_steps::instrument_file_parser::instrument_metadata::InstrumentMetadata;
use crate::compilation_steps::library_resolver::library_resolver::{resolve_libraries, ResolvedLibraries};
use crate::compilation_steps::patch_code_generator::patch_code_generator::generate_patch_source;
use crate::compilation_steps::patch_file_parser::patch_graph::PatchGraph;

/// The file in the instrument directory that holds the patch.
pub const PATCH_FILE: &str = "patch.json";

/// Where generated files go by default: a disposable directory inside the instrument directory.
pub const GENERATED_DIR: &str = ".kahuna";

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
/// Inputs that cannot be read or parsed leave `output_dir` untouched. Otherwise the generated
/// project in `output_dir/rust/` is emptied, and only its `Cargo.toml` and `src/lib.rs` are written,
/// which `cargo metadata` needs to locate the libraries; the next build removes them.
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
/// Every build starts from a clean project: what an earlier build left in `rust/` is removed,
/// except cargo's `target/` and `Cargo.lock`.
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

#[derive(Debug)]
pub struct BuildReport {
  /// The name from instrument.toml.
  pub instrument_name: String,
  /// `{output_dir}/rust`: the generated crate.
  pub crate_dir: PathBuf,
  /// The standalone executable cargo built from the crate.
  pub executable: PathBuf,
  /// Units patches can't use yet, with the reason. Warnings, not errors.
  pub unsupported_units: Vec<String>,
}

/// Generates the crate with `generate_instrument_crate` and builds it with `cargo build --release`.
/// Cargo's progress and compiler messages go to stderr while it runs. When the build fails, the
/// error says where the generated code is, because that is what the compiler messages refer to.
pub fn build_instrument(instrument_dir: &Path, output_dir: &Path) -> Result<BuildReport> {
  build_instrument_with_cargo(&cargo_program(), instrument_dir, output_dir)
}

fn build_instrument_with_cargo(cargo: &OsStr, instrument_dir: &Path, output_dir: &Path) -> Result<BuildReport> {
  let generated = generate_instrument_crate(instrument_dir, output_dir)?;
  let executable = build_release(cargo, &generated.crate_dir, &generated.instrument_name).wrap_err_with(|| {
    format!(
      "Cannot build '{}': cargo failed on the generated crate. Errors refer to {}, generated from {}",
      generated.instrument_name,
      generated.crate_dir.join("src/patch.rs").display(),
      instrument_dir.join(PATCH_FILE).display()
    )
  })?;
  Ok(BuildReport {
    instrument_name: generated.instrument_name,
    crate_dir: generated.crate_dir,
    executable,
    unsupported_units: generated.unsupported_units,
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

  // inputs are read and parsed: from here on the output directory is ours to change
  clean_project(&crate_dir(output_dir))?;
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
  fn building_again_starts_clean_and_keeps_only_the_build_cache_and_the_lock_file() {
    let output_dir = output_dir("rebuild");
    let crate_dir = generate_instrument_crate(&golden_synth(), &output_dir).unwrap().crate_dir;
    fs::create_dir_all(crate_dir.join("target")).unwrap();
    fs::write(crate_dir.join("target/cached"), "build output").unwrap();
    let lock_before = fs::read(crate_dir.join("Cargo.lock")).expect("cargo metadata writes the lock file");
    fs::write(crate_dir.join("src/stale.rs"), "// stale").unwrap();
    let patch_before = fs::read(crate_dir.join("src/patch.rs")).unwrap();

    generate_instrument_crate(&golden_synth(), &output_dir).unwrap();

    assert_eq!(fs::read(crate_dir.join("src/patch.rs")).unwrap(), patch_before, "the output is deterministic");
    assert!(!crate_dir.join("src/stale.rs").exists());
    assert_eq!(fs::read_to_string(crate_dir.join("target/cached")).unwrap(), "build output");
    assert_eq!(fs::read(crate_dir.join("Cargo.lock")).unwrap(), lock_before);
  }

  #[test]
  fn a_failed_build_leaves_no_patch_rs_of_an_earlier_build() {
    let output_dir = output_dir("failed-rebuild");
    let crate_dir = generate_instrument_crate(&golden_synth(), &output_dir).unwrap().crate_dir;
    let broken = golden_synth_with_patch("failed-rebuild", |patch| patch.replace("kahuna-std/sine", "kahuna-std/does-not-exist"));

    let error = generate_instrument_crate(&broken, &output_dir).unwrap_err();

    assert!(format!("{error:#}").contains("kahuna-std/does-not-exist"));
    assert!(!crate_dir.join("src/patch.rs").exists());
    assert!(!crate_dir.join("src/main.rs").exists());
  }

  #[test]
  fn an_unparsable_patch_leaves_an_earlier_build_untouched() {
    let output_dir = output_dir("unparsable-rebuild");
    let crate_dir = generate_instrument_crate(&golden_synth(), &output_dir).unwrap().crate_dir;
    let broken = golden_synth_with_patch("unparsable-rebuild", |_| "not json".to_string());

    assert!(generate_instrument_crate(&broken, &output_dir).is_err());

    assert!(crate_dir.join("src/patch.rs").is_file());
  }

  #[test]
  fn refuses_to_build_into_a_rust_directory_that_was_not_generated() {
    let output_dir = output_dir("foreign");
    fs::create_dir_all(output_dir.join("rust")).unwrap();
    fs::write(output_dir.join("rust/Cargo.toml"), "[package]\nname = \"mine\"\n").unwrap();

    let error = generate_instrument_crate(&golden_synth(), &output_dir).unwrap_err();

    assert!(format!("{error:#}").contains("Refusing to clean"), "{error:#}");
    assert!(output_dir.join("rust/Cargo.toml").is_file());
  }

  #[cfg(unix)]
  mod build_with_cargo {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn fake_cargo(name: &str, script: &str) -> PathBuf {
      let path = output_dir(&format!("{name}-cargo")).join("cargo");
      fs::create_dir_all(path.parent().unwrap()).unwrap();
      fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
      fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
      path
    }

    const ARTIFACT: &str = r#"{"reason":"compiler-artifact","target":{"kind":["bin"],"name":"golden-synth"},"executable":"/built/golden-synth"}"#;

    #[test]
    fn reports_the_executable_next_to_the_generated_crate() {
      let cargo = fake_cargo("report", &format!("echo '{ARTIFACT}'"));
      let output_dir = output_dir("report");

      let report = build_instrument_with_cargo(cargo.as_os_str(), &golden_synth(), &output_dir).unwrap();

      assert_eq!(report.instrument_name, "golden-synth");
      assert_eq!(report.crate_dir, output_dir.join("rust"));
      assert_eq!(report.executable, Path::new("/built/golden-synth"));
      assert!(report.crate_dir.join("src/patch.rs").is_file());
    }

    #[test]
    fn a_failed_cargo_build_names_the_generated_patch_and_the_patch_it_came_from() {
      let cargo = fake_cargo("failure", "exit 101");
      let output_dir = output_dir("failure");

      let error = build_instrument_with_cargo(cargo.as_os_str(), &golden_synth(), &output_dir).unwrap_err();

      let message = format!("{error:#}");
      assert!(message.contains(&output_dir.join("rust/src/patch.rs").display().to_string()), "{message}");
      assert!(message.contains(&golden_synth().join("patch.json").display().to_string()), "{message}");
      assert!(message.contains("101"), "{message}");
    }

    #[test]
    fn a_patch_that_does_not_compile_never_starts_cargo() {
      let marker = output_dir("never-started-marker");
      let cargo = fake_cargo("never-started", &format!("touch '{}'", marker.display()));
      let broken = golden_synth_with_patch("never-started", |patch| patch.replace("kahuna-std/sine", "kahuna-std/does-not-exist"));

      assert!(build_instrument_with_cargo(cargo.as_os_str(), &broken, &output_dir("never-started")).is_err());

      assert!(!marker.exists(), "cargo must not run for a patch that does not compile");
    }
  }

  /// Slow: compiles the generated crate and the toolchain's dependencies (cpal, midir) and, on
  /// Linux, needs the ALSA headers (`libasound2-dev`). Run with `cargo test -- --ignored`.
  #[test]
  #[ignore = "runs cargo build --release on the generated crate"]
  fn building_the_golden_instrument_produces_an_executable() {
    let report = build_instrument(&golden_synth(), &output_dir("real-build")).unwrap();

    assert!(report.executable.is_file(), "{}", report.executable.display());
    assert!(report.executable.file_name().unwrap().to_string_lossy().starts_with("golden-synth"));
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
