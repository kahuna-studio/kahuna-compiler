//! The `build` subcommand, run as the executable a user would run.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn golden_synth() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/instruments/golden-synth")
}

fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kahuna-cli-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn copy_golden_synth(name: &str) -> PathBuf {
    let dir = scratch_dir(name);
    for file in ["instrument.toml", "patch.json"] {
        fs::copy(golden_synth().join(file), dir.join(file)).unwrap();
    }
    dir
}

fn compiler(args: &[&Path]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kahuna-compiler")).args(args).output().unwrap()
}

/// Runs the compiler with `cargo` as the cargo it should use.
fn compiler_with_cargo(cargo: &Path, args: &[&Path]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kahuna-compiler")).args(args).env("CARGO", cargo).output().unwrap()
}

/// A stand-in for cargo: building the golden instrument for real takes minutes, and these tests are
/// about what the compiler does around cargo. The real build is covered by an ignored test.
/// The compiler also asks cargo for `metadata`, which the stand-in leaves to the real one.
fn fake_cargo(name: &str, script: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let real_cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let path = scratch_dir(&format!("{name}-cargo")).join("cargo");
    let script = format!("if [ \"$1\" = metadata ]; then exec '{real_cargo}' \"$@\"; fi\n{script}");
    fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

const ARTIFACT: &str =
    r#"echo '{"reason":"compiler-artifact","target":{"kind":["bin"],"name":"golden-synth"},"executable":"/built/golden-synth"}'"#;

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn build_generates_the_crate_in_kahuna_rust_of_the_instrument_directory() {
    let instrument_dir = copy_golden_synth("default-output");
    let cargo = fake_cargo("default-output", ARTIFACT);

    let output = compiler_with_cargo(&cargo, &[Path::new("build"), &instrument_dir]);

    assert!(output.status.success(), "{}", text(&output.stderr));
    for file in ["Cargo.toml", "src/lib.rs", "src/patch.rs", "src/main.rs"] {
        assert!(instrument_dir.join(".kahuna/rust").join(file).is_file(), "{file}");
    }
}

#[test]
fn build_writes_into_the_output_dir_when_given() {
    let instrument_dir = copy_golden_synth("custom-output-instrument");
    let output_dir = scratch_dir("custom-output").join("out");
    let cargo = fake_cargo("custom-output", ARTIFACT);

    let output = compiler_with_cargo(&cargo, &[Path::new("build"), &instrument_dir, Path::new("--output-dir"), &output_dir]);

    assert!(output.status.success(), "{}", text(&output.stderr));
    assert!(output_dir.join("rust/src/patch.rs").is_file());
    assert!(!instrument_dir.join(".kahuna").exists());
}

#[test]
fn build_reports_the_executable_cargo_built_and_shows_cargos_progress() {
    let instrument_dir = copy_golden_synth("executable");
    let cargo = fake_cargo("executable", &format!("echo '   Compiling golden-synth v0.1.0' >&2\n{ARTIFACT}"));

    let output = compiler_with_cargo(&cargo, &[Path::new("build"), &instrument_dir]);

    assert!(output.status.success(), "{}", text(&output.stderr));
    assert!(text(&output.stdout).contains("✓ Built 'golden-synth': /built/golden-synth"), "{}", text(&output.stdout));
    assert!(text(&output.stderr).contains("Compiling golden-synth"), "{}", text(&output.stderr));
}

#[test]
fn build_shows_the_compiler_errors_of_a_failed_cargo_build_and_names_the_generated_patch() {
    let instrument_dir = copy_golden_synth("cargo-fails");
    let cargo = fake_cargo("cargo-fails", "echo 'error[E0425]: cannot find value `x` in this scope' >&2\nexit 101");

    let output = compiler_with_cargo(&cargo, &[Path::new("build"), &instrument_dir]);

    assert_eq!(output.status.code(), Some(1));
    let stderr = text(&output.stderr);
    assert!(stderr.contains("error[E0425]: cannot find value"), "{stderr}");
    assert!(stderr.contains(".kahuna/rust/src/patch.rs"), "{stderr}");
    assert!(stderr.contains("patch.json"), "{stderr}");
}

#[test]
fn build_of_a_directory_without_instrument_toml_fails_and_says_so() {
    let empty = scratch_dir("no-instrument");

    let output = compiler(&[Path::new("build"), &empty]);

    assert_eq!(output.status.code(), Some(1));
    assert!(text(&output.stderr).contains("instrument.toml"), "{}", text(&output.stderr));
}

#[test]
fn build_of_a_missing_directory_fails_and_names_it() {
    let missing = scratch_dir("missing-parent").join("nowhere");

    let output = compiler(&[Path::new("build"), &missing]);

    assert_eq!(output.status.code(), Some(1));
    assert!(text(&output.stderr).contains("nowhere"), "{}", text(&output.stderr));
}

#[test]
fn build_of_an_instrument_without_patch_json_fails_and_names_the_file() {
    let instrument_dir = copy_golden_synth("no-patch");
    fs::remove_file(instrument_dir.join("patch.json")).unwrap();

    let output = compiler(&[Path::new("build"), &instrument_dir]);

    assert_eq!(output.status.code(), Some(1));
    assert!(text(&output.stderr).contains("patch.json"), "{}", text(&output.stderr));
}

#[test]
fn build_of_a_patch_with_an_unknown_unit_names_the_unit_and_generates_no_patch() {
    let instrument_dir = copy_golden_synth("unknown-unit");
    let patch = fs::read_to_string(instrument_dir.join("patch.json")).unwrap();
    fs::write(instrument_dir.join("patch.json"), patch.replace("kahuna-std/sine", "kahuna-std/does-not-exist")).unwrap();

    let output = compiler(&[Path::new("build"), &instrument_dir]);

    assert_eq!(output.status.code(), Some(1));
    assert!(text(&output.stderr).contains("kahuna-std/does-not-exist"), "{}", text(&output.stderr));
    assert!(!instrument_dir.join(".kahuna/rust/src/patch.rs").exists());
}

#[test]
fn help_describes_the_subcommands_and_the_generated_layout() {
    let output = compiler(&[Path::new("--help")]);

    let help = text(&output.stdout);
    assert!(output.status.success());
    for expected in ["build", "watch", "manifest", ".kahuna/rust"] {
        assert!(help.contains(expected), "missing '{expected}' in:\n{help}");
    }
}

#[test]
fn build_help_says_that_cargo_builds_the_executable() {
    let output = compiler(&[Path::new("build"), Path::new("--help")]);

    let help = text(&output.stdout);
    assert!(output.status.success());
    for expected in ["cargo build --release", "CARGO", "--output-dir"] {
        assert!(help.contains(expected), "missing '{expected}' in:\n{help}");
    }
}
