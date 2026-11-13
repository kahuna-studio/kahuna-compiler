//! The files the compiler places around the generated project: the finished executable in
//! `{output_dir}/bin/` and, for an instrument that has none, a `.gitignore` for `.kahuna/`.

use std::fs;
use std::path::{Path, PathBuf};

use eyre::{eyre, Result, WrapErr};

/// The directory for finished executables inside the output directory (`.kahuna/bin`). Like `rust/`
/// it has a fixed name, so an instrument called `rust` cannot collide with anything.
pub const BIN_DIR: &str = "bin";

const GITIGNORE_FILE: &str = ".gitignore";

/// Where the executable of the instrument `name` ends up: `{output_dir}/bin/{name}`, with the
/// platform's executable suffix (`.exe` on Windows).
pub fn executable_path(output_dir: &Path, name: &str) -> PathBuf {
  output_dir.join(BIN_DIR).join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

/// Copies the executable that cargo built to `{output_dir}/bin/{name}` and returns that path. An
/// executable of an earlier build is replaced. The copy is written under a temporary name and then
/// renamed, so an interrupted copy never leaves half an executable under the final name.
pub fn install_executable(executable: &Path, output_dir: &Path, name: &str) -> Result<PathBuf> {
  let destination = executable_path(output_dir, name);
  let bin_dir = output_dir.join(BIN_DIR);
  fs::create_dir_all(&bin_dir).wrap_err_with(|| format!("Cannot create {}", bin_dir.display()))?;

  let temporary = bin_dir.join(format!(".{name}.tmp"));
  // `fs::copy` keeps the permission bits, so the copy stays executable on Unix.
  fs::copy(executable, &temporary)
    .wrap_err_with(|| format!("Cannot copy {} to {}", executable.display(), temporary.display()))?;
  if let Err(error) = fs::rename(&temporary, &destination) {
    let _ = fs::remove_file(&temporary);
    return Err(eyre!(error)).wrap_err_with(|| {
      format!("Cannot write {}; if the instrument is running, stop it and build again", destination.display())
    });
  }
  Ok(destination)
}

/// Creates `{instrument_dir}/.gitignore` with `.kahuna/` when the instrument has no `.gitignore`
/// yet. An existing file is never edited: it is the author's.
pub fn ensure_gitignore(instrument_dir: &Path) -> Result<()> {
  let path = instrument_dir.join(GITIGNORE_FILE);
  if path.exists() {
    return Ok(());
  }
  fs::write(&path, format!("{}/\n", crate::pipeline::GENERATED_DIR)).wrap_err_with(|| format!("Cannot write {}", path.display()))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kahuna-output-files-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
  }

  fn dummy_executable(dir: &Path, contents: &str) -> PathBuf {
    let path = dir.join("built");
    fs::write(&path, contents).unwrap();
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    path
  }

  #[test]
  fn the_executable_is_named_after_the_instrument_in_bin() {
    let output_dir = Path::new("/out");

    let path = executable_path(output_dir, "my-synth");

    assert_eq!(path, Path::new("/out/bin").join(format!("my-synth{}", std::env::consts::EXE_SUFFIX)));
  }

  #[test]
  fn installing_copies_the_executable_and_creates_bin() {
    let dir = scratch_dir("install");
    let built = dummy_executable(&dir, "binary");

    let installed = install_executable(&built, &dir.join(".kahuna"), "my-synth").unwrap();

    assert_eq!(installed, executable_path(&dir.join(".kahuna"), "my-synth"));
    assert_eq!(fs::read_to_string(&installed).unwrap(), "binary");
  }

  #[cfg(unix)]
  #[test]
  fn the_copy_stays_executable() {
    use std::os::unix::fs::PermissionsExt;
    let dir = scratch_dir("permissions");
    let built = dummy_executable(&dir, "binary");

    let installed = install_executable(&built, &dir, "my-synth").unwrap();

    assert_eq!(fs::metadata(&installed).unwrap().permissions().mode() & 0o111, 0o111);
  }

  #[test]
  fn installing_again_replaces_the_executable_of_the_earlier_build() {
    let dir = scratch_dir("replace");
    install_executable(&dummy_executable(&dir, "first"), &dir, "my-synth").unwrap();

    let installed = install_executable(&dummy_executable(&dir, "second"), &dir, "my-synth").unwrap();

    assert_eq!(fs::read_to_string(installed).unwrap(), "second");
  }

  #[test]
  fn no_temporary_file_remains_in_bin() {
    let dir = scratch_dir("temporary");
    install_executable(&dummy_executable(&dir, "binary"), &dir, "my-synth").unwrap();

    let names: Vec<_> = fs::read_dir(dir.join(BIN_DIR)).unwrap().map(|e| e.unwrap().file_name()).collect();

    assert_eq!(names, [std::ffi::OsString::from(format!("my-synth{}", std::env::consts::EXE_SUFFIX))]);
  }

  #[test]
  fn a_missing_executable_is_reported_with_its_path_and_leaves_nothing_behind() {
    let dir = scratch_dir("missing");

    let error = install_executable(&dir.join("nowhere"), &dir, "my-synth").unwrap_err();

    assert!(format!("{error:#}").contains("nowhere"), "{error:#}");
    assert_eq!(fs::read_dir(dir.join(BIN_DIR)).unwrap().count(), 0);
  }

  #[test]
  fn a_gitignore_is_created_when_the_instrument_has_none() {
    let dir = scratch_dir("gitignore-created");

    ensure_gitignore(&dir).unwrap();

    assert_eq!(fs::read_to_string(dir.join(".gitignore")).unwrap(), ".kahuna/\n");
  }

  #[test]
  fn an_existing_gitignore_is_never_edited() {
    let dir = scratch_dir("gitignore-existing");
    fs::write(dir.join(".gitignore"), "target/\n").unwrap();

    ensure_gitignore(&dir).unwrap();

    assert_eq!(fs::read_to_string(dir.join(".gitignore")).unwrap(), "target/\n");
  }
}
