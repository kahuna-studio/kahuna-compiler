use std::path::PathBuf;

use clap::{Parser, Subcommand};
use eyre::{eyre, Result, WrapErr};
use std::process::ExitCode;
use kahuna_compiler::api::write_library_manifest;
use kahuna_compiler::database::CompilerDatabase;
use kahuna_compiler::inputs::{load_workspace, INSTRUMENT_FILE};
use kahuna_compiler::pipeline::{build_instrument, GENERATED_DIR};
use kahuna_compiler::runner::run;
use notify_debouncer_mini::notify::RecursiveMode;
use std::sync::mpsc;

/// Compiles Kahuna instruments.
///
/// An instrument is a directory with `instrument.toml` and `patch.json`. Compiling it generates a
/// Rust project in `.kahuna/rust/` inside that directory and builds it into an executable;
/// `.kahuna/` is disposable and not meant to be committed.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Builds an instrument into an executable, via a Rust project in <instrument_dir>/.kahuna/rust
    ///
    /// Reads instrument.toml and patch.json, checks the patch against the units of kahuna-std,
    /// writes the project (Cargo.toml, src/lib.rs, src/patch.rs, src/main.rs) and runs
    /// `cargo build --release` on it; cargo's output is shown while it runs. Whatever an earlier
    /// build left in the project is removed first, except cargo's target/ and Cargo.lock. Uses the
    /// cargo in the CARGO environment variable, or else the one on the PATH.
    Build {
        /// Directory containing instrument.toml and patch.json
        instrument_dir: PathBuf,

        /// Directory for the generated files instead of <instrument_dir>/.kahuna
        #[arg(long)]
        output_dir: Option<PathBuf>,
    },
    /// Checks an instrument and checks it again whenever its files change
    Watch {
        /// Directory containing instrument.toml
        instrument_dir: PathBuf,
    },
    /// Writes kahuna-library.json for a unit library by reading its source, without compiling it
    Manifest {
        /// Directory containing the library's Cargo.toml
        library_dir: PathBuf,
    },
}

fn main() -> ExitCode {
    match run_command(Cli::parse().command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run_command(command: Command) -> Result<()> {
    match command {
        Command::Build { instrument_dir, output_dir } => build(instrument_dir, output_dir),
        Command::Watch { instrument_dir } => compile_and_watch(instrument_dir),
        Command::Manifest { library_dir } => {
            let path = write_library_manifest(&library_dir)?;
            println!("✓ Wrote {}", path.display());
            Ok(())
        }
    }
}

fn build(instrument_dir: PathBuf, output_dir: Option<PathBuf>) -> Result<()> {
    let instrument_dir = resolve_base_dir(instrument_dir)?;
    let output_dir = output_dir.unwrap_or_else(|| instrument_dir.join(GENERATED_DIR));

    let report = build_instrument(&instrument_dir, &output_dir)?;

    for warning in &report.unsupported_units {
        eprintln!("warning: {warning}");
    }
    println!("✓ Built '{}': {}", report.instrument_name, report.executable.display());
    Ok(())
}

fn compile_and_watch(instrument_dir: PathBuf) -> Result<()> {
    let base_dir = resolve_base_dir(instrument_dir)?;

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
