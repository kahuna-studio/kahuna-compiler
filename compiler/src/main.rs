use std::sync::mpsc;
use eyre::Result;
use kahuna_compiler::cli::base_dir_from_args;
use kahuna_compiler::database::CompilerDatabase;
use kahuna_compiler::inputs::load_workspace;
use kahuna_compiler::runner::run;
use notify_debouncer_mini::notify::RecursiveMode;

fn main() -> Result<()> {
    let base_dir = base_dir_from_args()?;

    let (tx, rx) = mpsc::channel();
    let mut db = CompilerDatabase::new(tx)?;
    let workspace = load_workspace(&db, &base_dir)?;

    // Only files directly inside base_dir are inputs for now.
    // Switch to Recursive once modules/UI sources become inputs.
    db.watch(&base_dir, RecursiveMode::NonRecursive)?;

    run(&mut db, workspace, &base_dir, &rx)
}
