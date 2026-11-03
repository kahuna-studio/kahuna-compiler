use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use eyre::Result;
use salsa::Setter;
use super::workspace::{File, Workspace, get_instrument_file, get_patch_file};

pub fn load_workspace(db: &dyn salsa::Database, base_dir: &Path) -> Result<Workspace> {
    let instrument = load_file(db, base_dir, get_instrument_file())?;
    let patch = load_file(db, base_dir, get_patch_file())?;
    Ok(Workspace::new(db, instrument, patch))
}

pub fn load_file(db: &dyn salsa::Database, base_dir: &Path, relative: &str) -> Result<File> {
    let contents = read_optional(&base_dir.join(relative))?;
    Ok(File::new(db, PathBuf::from(relative), contents))
}

/// Re-reads one known input from disk. A missing file becomes `None`, so deletion
/// and later re-creation both invalidate the queries that depend on it.
pub fn reload_file(
    db: &mut dyn salsa::Database,
    workspace: Workspace,
    base_dir: &Path,
    relative: &Path,
) -> Result<()> {
    let Some(file) = file_for(db, workspace, relative) else {
        return Ok(()); // not an input of the compiler
    };

    let contents = read_optional(&base_dir.join(relative))?;
    if *file.contents(db) != contents {
        file.set_contents(db).to(contents);
    }
    Ok(())
}

pub fn reload_all(db: &mut dyn salsa::Database, workspace: Workspace, base_dir: &Path) {
    for relative in [get_instrument_file(), get_patch_file()] {
        if let Err(error) = reload_file(db, workspace, base_dir, Path::new(relative)) {
            eprintln!("Cannot reload {relative}: {error:?}");
        }
    }
}

pub fn file_for(db: &dyn salsa::Database, workspace: Workspace, relative: &Path) -> Option<File> {
    if relative == Path::new(get_instrument_file()) {
        Some(*workspace.instrument(db))
    } else if relative == Path::new(get_patch_file()) {
        Some(*workspace.patch(db))
    } else {
        None
    }
}

pub fn relative_to_base(base_dir: &Path, event_path: &Path) -> Option<PathBuf> {
    resolve_path(event_path)
      .strip_prefix(base_dir)
      .ok()
      .map(Path::to_path_buf)
}

/// Canonicalizes a path even if the file itself is gone (deleted, or replaced via rename).
pub fn resolve_path(path: &Path) -> PathBuf {
    path.canonicalize()
      .ok()
      .or_else(|| {
          let parent = path.parent()?.canonicalize().ok()?;
          Some(parent.join(path.file_name()?))
      })
      .unwrap_or_else(|| path.to_path_buf())
}

pub fn read_optional(path: &Path) -> Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}
