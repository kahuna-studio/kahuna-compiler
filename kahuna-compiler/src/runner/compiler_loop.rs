use std::path::Path;
use std::sync::mpsc::Receiver;
use eyre::Result;
use notify_debouncer_mini::DebounceEventResult;
use crate::database::CompilerDatabase;
use crate::inputs::{Workspace, reload_file, reload_all};
use crate::database::compile_package;
use crate::diagnostics::{Diagnostic, Log};

pub fn run(
    db: &mut CompilerDatabase,
    workspace: Workspace,
    base_dir: &Path,
    rx: &Receiver<DebounceEventResult>,
) -> Result<()> {
    loop {
        report(db, workspace);
        wait_for_changes(db, workspace, base_dir, rx)?;
    }
}

fn report(db: &CompilerDatabase, workspace: Workspace) {
    let node_count = compile_package(db, workspace);
    let diagnostics = compile_package::accumulated::<Diagnostic>(db, workspace);
    let logs = compile_package::accumulated::<Log>(db, workspace);

    if diagnostics.is_empty() {
        println!("✓ Compilation successful ({node_count} patch nodes)");
    } else {
        for diagnostic in diagnostics {
            println!("{}", diagnostic.0);
        }
    }

    for log_message in logs {
        println!("{}", log_message.0);
    }
}

/// Blocks until the file watcher reports something, then applies all changes to the salsa inputs.
fn wait_for_changes(
    db: &mut CompilerDatabase,
    workspace: Workspace,
    base_dir: &Path,
    rx: &Receiver<DebounceEventResult>,
) -> Result<()> {
    use crate::inputs::relative_to_base;
    
    let events = match rx.recv()? {
        Ok(events) => events,
        Err(watch_error) => {
            // Recompile anyway: a lost event must not leave us with stale inputs.
            eprintln!("File watcher error: {watch_error}");
            reload_all(db, workspace, base_dir);
            return Ok(());
        }
    };

    for event in events {
        let Some(relative) = relative_to_base(base_dir, &event.path) else {
            continue;
        };
        if let Err(error) = reload_file(db, workspace, base_dir, &relative) {
            eprintln!("Cannot reload {}: {error:?}", relative.display());
        }
    }
    Ok(())
}
