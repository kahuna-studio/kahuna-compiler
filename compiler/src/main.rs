use std::path::PathBuf;

use clap::Parser;
use eyre::{eyre, Result, WrapErr};
use kahuna_compiler::database::CompilerDatabase;
use kahuna_compiler::inputs::{load_workspace, INSTRUMENT_FILE};
use kahuna_compiler::runner::run;
use notify_debouncer_mini::notify::RecursiveMode;
use std::sync::mpsc;

/// Compiles a Kahuna instrument and recompiles on file changes.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Directory containing instrument.toml
    instrument_dir: PathBuf,
}

fn main() -> Result<()> {
    let base_dir = resolve_base_dir(Cli::parse().instrument_dir)?;

    let (tx, rx) = mpsc::channel();
    let mut db = CompilerDatabase::new(tx)?;
    let workspace = load_workspace(&db, &base_dir)?;

    // Only files directly inside base_dir are inputs for now.
    // Switch to Recursive once modules/UI sources become inputs.
    db.watch(&base_dir, RecursiveMode::NonRecursive)?;

    run(&mut db, workspace, &base_dir, &rx)
}

fn resolve_base_dir(dir: PathBuf) -> Result<PathBuf> {
    let base_dir = dir
      .canonicalize()
      .wrap_err_with(|| format!("Cannot access directory: {}", dir.display()))?;

    if !base_dir.is_dir() {
        return Err(eyre!("Not a directory: {}", base_dir.display()));
    }

    let instrument_file = base_dir.join(INSTRUMENT_FILE);
    if !instrument_file.is_file() {
        return Err(eyre!(
            "Expected '{INSTRUMENT_FILE}' in {}, but it was not found",
            base_dir.display()
        ));
    }

    Ok(base_dir)
}
