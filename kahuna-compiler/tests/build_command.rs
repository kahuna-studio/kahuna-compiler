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

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn build_generates_the_crate_in_kahuna_rust_of_the_instrument_directory() {
    let instrument_dir = copy_golden_synth("default-output");

    let output = compiler(&[Path::new("build"), &instrument_dir]);

    assert!(output.status.success(), "{}", text(&output.stderr));
    let stdout = text(&output.stdout);
    assert!(stdout.contains("✓ Generated crate for 'golden-synth'"), "{stdout}");
    for file in ["Cargo.toml", "src/lib.rs", "src/patch.rs", "src/main.rs"] {
        assert!(instrument_dir.join(".kahuna/rust").join(file).is_file(), "{file}");
    }
}

#[test]
fn build_writes_into_the_output_dir_when_given() {
    let instrument_dir = copy_golden_synth("custom-output-instrument");
    let output_dir = scratch_dir("custom-output").join("out");

    let output = compiler(&[Path::new("build"), &instrument_dir, Path::new("--output-dir"), &output_dir]);

    assert!(output.status.success(), "{}", text(&output.stderr));
    assert!(output_dir.join("rust/src/patch.rs").is_file());
    assert!(!instrument_dir.join(".kahuna").exists());
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
