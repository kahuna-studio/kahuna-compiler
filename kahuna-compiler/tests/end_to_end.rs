//! From the golden instrument to a running executable, with the compiler as a user runs it.
//!
//! The full build is slow and ignored by default: `cargo test -- --ignored`. On Linux it needs the ALSA
//! headers (`libasound2-dev`).

use kahuna_compiler::pipeline::generate_instrument_crate;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);

fn golden_synth() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/instruments/golden-synth")
}

/// A copy of the instrument directory, so that `.kahuna/` is generated outside of the repository.
fn copy_golden_synth(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kahuna-e2e-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    for file in ["instrument.toml", "patch.json"] {
        fs::copy(golden_synth().join(file), dir.join(file)).unwrap();
    }
    dir
}

#[test]
fn the_generated_patch_of_the_golden_instrument_is_the_expected_one() {
    let instrument_dir = copy_golden_synth("expected-patch");

    let generated = generate_instrument_crate(&instrument_dir, &instrument_dir.join(".kahuna")).unwrap();

    let actual = fs::read_to_string(generated.crate_dir.join("src/patch.rs")).unwrap();
    let expected = fs::read_to_string(golden_synth().join("expected-patch.rs")).unwrap().replace("\r\n", "\n");
    assert!(
        actual == expected,
        "The generated patch.rs differs from tests/fixtures/instruments/golden-synth/expected-patch.rs. \
         If the change is intended, copy {} over the expected file.",
        generated.crate_dir.join("src/patch.rs").display()
    );
}

#[test]
fn a_failed_build_leaves_no_executable() {
    let instrument_dir = copy_golden_synth("failed-build");
    let patch = fs::read_to_string(instrument_dir.join("patch.json")).unwrap();
    fs::write(instrument_dir.join("patch.json"), patch.replace("kahuna-std/sine", "kahuna-std/does-not-exist")).unwrap();

    let output = build(&instrument_dir);

    assert_eq!(output.status.code(), Some(1));
    assert!(!instrument_dir.join(".kahuna/bin").exists());
}

#[test]
#[ignore = "runs cargo build --release on the generated crate, twice"]
fn the_golden_instrument_builds_incrementally_and_the_executable_starts_without_panicking() {
    let instrument_dir = copy_golden_synth("full-build");

    let first = build(&instrument_dir);
    assert!(first.status.success(), "{}", text(&first.stderr));
    let executable = instrument_dir.join(".kahuna/bin").join(format!("golden-synth{}", std::env::consts::EXE_SUFFIX));
    assert!(executable.is_file(), "{}", executable.display());
    assert_eq!(fs::read_to_string(instrument_dir.join(".gitignore")).unwrap().trim(), ".kahuna/");
    let patch = fs::read_to_string(instrument_dir.join(".kahuna/rust/src/patch.rs")).unwrap();
    assert_eq!(patch, fs::read_to_string(golden_synth().join("expected-patch.rs")).unwrap().replace("\r\n", "\n"));

    let second = build(&instrument_dir);
    assert!(second.status.success(), "{}", text(&second.stderr));
    assert!(instrument_dir.join(".kahuna/rust/target").is_dir(), "the build cache was removed");
    assert!(!text(&second.stderr).contains("Compiling cpal"), "the second build compiled cpal again:\n{}", text(&second.stderr));

    assert_starts_and_stops_cleanly(&executable);
}

/// Runs `build` as a user would, with cargo's build cache in `.kahuna/rust/target` even where the
/// environment of the test redirects cargo's target directory.
fn build(instrument_dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_kahuna-compiler"))
        .arg("build")
        .arg(instrument_dir)
        .env_remove("CARGO_TARGET_DIR")
        .output()
        .unwrap()
}

/// Starts the executable with an open stdin. Where the machine has audio, it plays until a line
/// arrives on stdin and then exits with 0. Where it has none, it exits with 1 and a line starting
/// with `Error: `. A missing MIDI device is no error. A panic is never right.
fn assert_starts_and_stops_cleanly(executable: &Path) {
    let mut child = Command::new(executable).stdin(Stdio::piped()).stderr(Stdio::piped()).stdout(Stdio::null()).spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let stderr = std::thread::spawn({
        let mut pipe = child.stderr.take().unwrap();
        move || {
            let mut output = String::new();
            pipe.read_to_string(&mut output).ok();
            output
        }
    });

    let early_exit = wait_for_exit(&mut child, Duration::from_secs(3));
    if early_exit.is_none() {
        stdin.write_all(b"\n").unwrap();
    }
    let status = early_exit.unwrap_or_else(|| {
        wait_for_exit(&mut child, STARTUP_TIMEOUT).unwrap_or_else(|| {
            child.kill().ok();
            panic!("the executable did not stop after Enter");
        })
    });
    drop(stdin);
    let stderr = stderr.join().unwrap();

    assert!(!stderr.contains("panicked"), "{stderr}");
    match status.code() {
        Some(0) => assert!(stderr.contains("[Kahuna] Running."), "{stderr}"),
        Some(1) => assert!(stderr.lines().any(|line| line.starts_with("Error: ")), "{stderr}"),
        other => panic!("unexpected exit status {other:?}:\n{stderr}"),
    }
}

/// The exit status, or none when the process still runs after `timeout`. The process is left running.
fn wait_for_exit(child: &mut std::process::Child, timeout: Duration) -> Option<std::process::ExitStatus> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().unwrap() {
            return Some(status);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
