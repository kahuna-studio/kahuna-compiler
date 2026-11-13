//! Builds the generated crate with cargo and finds the executable it produced.
//!
//! Cargo runs with `--message-format=json-render-diagnostics`: compiler errors and warnings are
//! rendered by cargo itself and stream to this process's stderr while the build runs, and what
//! arrives on stdout is one JSON message per line, among them the `compiler-artifact` messages that
//! name the files cargo has built. The executable is taken from those, so it is found wherever
//! cargo puts it (a `CARGO_TARGET_DIR` or `build.target-dir` setting included).

use std::ffi::{OsStr, OsString};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use eyre::{ensure, eyre, Result, WrapErr};

/// The cargo that started this process (`cargo run` sets `CARGO`), otherwise the one on the `PATH`.
pub fn cargo_program() -> OsString {
  std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into())
}

/// Runs `cargo build --release` for the crate in `crate_dir` and returns the path of the executable
/// that is named like the package. Progress and compiler messages go to stderr as they happen.
pub fn build_release(cargo: &OsStr, crate_dir: &Path, package_name: &str) -> Result<PathBuf> {
  let mut cargo_process = Command::new(cargo)
    .args(["build", "--release", "--message-format=json-render-diagnostics", "--manifest-path"])
    .arg(crate_dir.join("Cargo.toml"))
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::inherit())
    .spawn()
    .wrap_err_with(|| format!("Cannot run {cargo:?}; is cargo installed and on the PATH?"))?;

  let stdout = cargo_process.stdout.take().expect("stdout is piped");
  let mut executable = None;
  for line in BufReader::new(stdout).lines() {
    let line = line.wrap_err("Cannot read the output of cargo")?;
    if let Some(path) = executable_of_message(&line, package_name) {
      executable = Some(path);
    }
  }

  let status = cargo_process.wait().wrap_err("Cannot wait for cargo")?;
  ensure!(status.success(), "cargo build failed: {status}");
  executable.ok_or_else(|| eyre!("cargo build succeeded but reported no executable named '{package_name}'"))
}

/// The executable of a `compiler-artifact` message for the binary target `package_name`.
/// Every other line, JSON or not, is none of our business.
fn executable_of_message(line: &str, package_name: &str) -> Option<PathBuf> {
  let message: serde_json::Value = serde_json::from_str(line).ok()?;
  if message["reason"] != "compiler-artifact" {
    return None;
  }
  let target = &message["target"];
  let is_binary = target["kind"].as_array()?.iter().any(|kind| kind == "bin");
  if !is_binary || target["name"] != package_name {
    return None;
  }
  message["executable"].as_str().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
  use super::*;

  const BIN_ARTIFACT: &str = r#"{"reason":"compiler-artifact","package_id":"my-synth 0.1.0 (path+file:///x)","target":{"kind":["bin"],"crate_types":["bin"],"name":"my-synth"},"executable":"/x/target/release/my-synth","fresh":false}"#;

  fn artifact(kind: &str, name: &str, executable: &str) -> String {
    format!(r#"{{"reason":"compiler-artifact","target":{{"kind":["{kind}"],"name":"{name}"}},"executable":{executable}}}"#)
  }

  #[test]
  fn takes_the_executable_of_the_binary_artifact() {
    assert_eq!(executable_of_message(BIN_ARTIFACT, "my-synth"), Some(PathBuf::from("/x/target/release/my-synth")));
  }

  #[test]
  fn ignores_the_library_artifact_of_the_same_package() {
    let library = artifact("lib", "my-synth", "null");

    assert_eq!(executable_of_message(&library, "my-synth"), None);
  }

  #[test]
  fn ignores_binaries_of_other_packages() {
    let other = artifact("bin", "other", r#""/x/other""#);

    assert_eq!(executable_of_message(&other, "my-synth"), None);
  }

  #[test]
  fn ignores_other_messages_and_lines_that_are_not_json() {
    for line in [r#"{"reason":"build-finished","success":true}"#, r#"{"reason":"build-script-executed"}"#, "Compiling", ""] {
      assert_eq!(executable_of_message(line, "my-synth"), None, "{line}");
    }
  }

  #[cfg(unix)]
  mod with_a_fake_cargo {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn fake_cargo(name: &str, script: &str) -> PathBuf {
      let dir = std::env::temp_dir().join(format!("kahuna-cargo-runner-{name}-{}", std::process::id()));
      let _ = fs::remove_dir_all(&dir);
      fs::create_dir_all(&dir).unwrap();
      let path = dir.join("cargo");
      fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
      fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
      path
    }

    #[test]
    fn returns_the_executable_cargo_reports() {
      let cargo = fake_cargo("ok", &format!("echo '{BIN_ARTIFACT}'"));

      let executable = build_release(cargo.as_os_str(), Path::new("/crate"), "my-synth").unwrap();

      assert_eq!(executable, Path::new("/x/target/release/my-synth"));
    }

    #[test]
    fn builds_in_release_mode_with_the_manifest_of_the_crate() {
      let arguments_file = std::env::temp_dir().join(format!("kahuna-cargo-arguments-{}", std::process::id()));
      let cargo = fake_cargo("arguments", &format!("echo \"$@\" > '{}'\necho '{BIN_ARTIFACT}'", arguments_file.display()));

      build_release(cargo.as_os_str(), Path::new("/crate"), "my-synth").unwrap();

      let arguments = fs::read_to_string(&arguments_file).unwrap();
      assert_eq!(
        arguments.trim(),
        "build --release --message-format=json-render-diagnostics --manifest-path /crate/Cargo.toml"
      );
    }

    #[test]
    fn reports_a_failed_build_with_cargos_exit_status() {
      let cargo = fake_cargo("failing", "echo 'error[E0425]: cannot find value' >&2\nexit 101");

      let error = build_release(cargo.as_os_str(), Path::new("/crate"), "my-synth").unwrap_err();

      assert!(format!("{error}").contains("cargo build failed") && format!("{error}").contains("101"), "{error}");
    }

    #[test]
    fn reports_a_build_that_produced_no_executable() {
      let cargo = fake_cargo("no-artifact", "echo '{\"reason\":\"build-finished\",\"success\":true}'");

      let error = build_release(cargo.as_os_str(), Path::new("/crate"), "my-synth").unwrap_err();

      assert!(format!("{error}").contains("no executable named 'my-synth'"), "{error}");
    }

    #[test]
    fn reports_a_cargo_that_cannot_be_run() {
      let error = build_release(OsStr::new("/nonexistent/cargo"), Path::new("/crate"), "my-synth").unwrap_err();

      assert!(format!("{error:#}").contains("Cannot run"), "{error:#}");
    }
  }
}
