use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eyre::{Report, Result, eyre};
use kahuna_compiler::instrument_metadata::InstrumentMetadata;
use kahuna_compiler::patch_graph::PatchGraph;
use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};
use salsa::{Accumulator, Setter, Storage};

const INSTRUMENT_FILE: &str = "instrument.toml";
const PATCH_FILE: &str = "patch.json";
const DEBOUNCE: Duration = Duration::from_millis(300);

// ------------------- main -------------------

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

fn base_dir_from_args() -> Result<PathBuf> {
    let instrument_path: PathBuf = std::env::args_os()
      .nth(1)
      .ok_or_else(|| eyre!("Usage: ./kahuna-compiler /path/to/{INSTRUMENT_FILE}"))?
      .into();
    let instrument_path = instrument_path.canonicalize()?;

    if instrument_path.file_name() != Some(std::ffi::OsStr::new(INSTRUMENT_FILE)) {
        return Err(eyre!(
            "Expected a file named '{INSTRUMENT_FILE}', got: {}",
            instrument_path.display()
        ));
    }

    instrument_path
      .parent()
      .map(Path::to_path_buf)
      .ok_or_else(|| eyre!("Instrument file has no parent directory"))
}

fn run(
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

// ------------------- file change handling -------------------

/// Blocks until the file watcher reports something, then applies all changes to the salsa inputs.
fn wait_for_changes(
    db: &mut CompilerDatabase,
    workspace: Workspace,
    base_dir: &Path,
    rx: &Receiver<DebounceEventResult>,
) -> Result<()> {
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

fn reload_all(db: &mut CompilerDatabase, workspace: Workspace, base_dir: &Path) {
    for relative in [INSTRUMENT_FILE, PATCH_FILE] {
        if let Err(error) = reload_file(db, workspace, base_dir, Path::new(relative)) {
            eprintln!("Cannot reload {relative}: {error:?}");
        }
    }
}

/// Re-reads one known input from disk. A missing file becomes `None`, so deletion
/// and later re-creation both invalidate the queries that depend on it.
fn reload_file(
    db: &mut CompilerDatabase,
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

fn file_for(db: &CompilerDatabase, workspace: Workspace, relative: &Path) -> Option<File> {
    if relative == Path::new(INSTRUMENT_FILE) {
        Some(*workspace.instrument(db))
    } else if relative == Path::new(PATCH_FILE) {
        Some(*workspace.patch(db))
    } else {
        None
    }
}

fn relative_to_base(base_dir: &Path, event_path: &Path) -> Option<PathBuf> {
    resolve_path(event_path)
      .strip_prefix(base_dir)
      .ok()
      .map(Path::to_path_buf)
}

/// Canonicalizes a path even if the file itself is gone (deleted, or replaced via rename).
fn resolve_path(path: &Path) -> PathBuf {
    path.canonicalize()
      .ok()
      .or_else(|| {
          let parent = path.parent()?.canonicalize().ok()?;
          Some(parent.join(path.file_name()?))
      })
      .unwrap_or_else(|| path.to_path_buf())
}

fn read_optional(path: &Path) -> Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(contents) => Ok(Some(contents)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn load_workspace(db: &CompilerDatabase, base_dir: &Path) -> Result<Workspace> {
    let instrument = load_file(db, base_dir, INSTRUMENT_FILE)?;
    let patch = load_file(db, base_dir, PATCH_FILE)?;
    Ok(Workspace::new(db, instrument, patch))
}

fn load_file(db: &CompilerDatabase, base_dir: &Path, relative: &str) -> Result<File> {
    let contents = read_optional(&base_dir.join(relative))?;
    Ok(File::new(db, PathBuf::from(relative), contents))
}

// ------------------- compiler queries -------------------

/// Entry point of the compilation. Returns the number of patch nodes for now (placeholder).
#[salsa::tracked]
fn compile_package(db: &dyn salsa::Database, workspace: Workspace) -> usize {
    let instrument_file = *workspace.instrument(db);
    let patch_file = *workspace.patch(db);

    Log::info(
        db,
        format!("Compiling package: {}", instrument_file.path(db).display()),
    );

    let _metadata = parse_instrument_metadata(db, instrument_file);

    match parse_patch_graph(db, patch_file) {
        Some(graph) => graph.nodes.len(),
        None => 0,
    }
}

/// Parses instrument.toml. `None` means a diagnostic was emitted.
#[salsa::tracked]
fn parse_instrument_metadata(db: &dyn salsa::Database, file: File) -> Option<InstrumentMetadata> {
    let Some(contents) = file.contents(db).as_deref() else {
        Diagnostic::push_error(db, file, eyre!("File not found"));
        return None;
    };

    match InstrumentMetadata::from_str(contents) {
        Ok(metadata) => {
            Log::info(
                db,
                format!(
                    "Loaded instrument '{}' by {} (version {})",
                    metadata.name, metadata.vendor, metadata.version
                ),
            );
            Some(metadata)
        }
        Err(error) => {
            Diagnostic::push_error(db, file, error);
            None
        }
    }
}

/// Parses patch.json. A missing file is not an error: the instrument may have no patch yet.
#[salsa::tracked]
fn parse_patch_graph(db: &dyn salsa::Database, file: File) -> Option<PatchGraph> {
    let Some(contents) = file.contents(db).as_deref() else {
        Log::info(db, format!("No {PATCH_FILE} found; skipping patch parsing"));
        return None;
    };

    match contents.parse::<PatchGraph>() {
        Ok(graph) => {
            Log::info(
                db,
                format!(
                    "Loaded patch graph with {} nodes and {} edges",
                    graph.nodes.len(),
                    graph.edges.len()
                ),
            );
            Some(graph)
        }
        Err(error) => {
            Diagnostic::push_error(db, file, error);
            None
        }
    }
}

// ------------------- salsa inputs -------------------

/// A file below the instrument directory. `contents == None` means the file does not exist.
#[salsa::input]
struct File {
    path: PathBuf,
    #[returns(ref)]
    contents: Option<String>,
}

/// All compiler inputs, created once up front so queries never touch the file system.
#[salsa::input]
struct Workspace {
    instrument: File,
    patch: File,
}

// ------------------- database -------------------

#[salsa::db]
#[derive(Clone)]
struct CompilerDatabase {
    storage: Storage<Self>,
    file_watcher: Arc<Mutex<Debouncer<RecommendedWatcher>>>,
}

#[salsa::db]
impl salsa::Database for CompilerDatabase {}

impl CompilerDatabase {
    fn new(tx: mpsc::Sender<DebounceEventResult>) -> Result<Self> {
        Ok(Self {
            storage: Storage::new(None),
            file_watcher: Arc::new(Mutex::new(new_debouncer(DEBOUNCE, tx)?)),
        })
    }

    fn watch(&self, dir: &Path, mode: RecursiveMode) -> Result<()> {
        let mut debouncer = self
          .file_watcher
          .lock()
          .map_err(|_| eyre!("File watcher mutex poisoned"))?;
        debouncer.watcher().watch(dir, mode)?;
        Ok(())
    }
}

// ------------------- diagnostics -------------------

#[salsa::accumulator]
struct Diagnostic(String);

impl Diagnostic {
    fn push_error(db: &dyn salsa::Database, file: File, error: Report) {
        Diagnostic(format!(
            "Error in file {}: {:?}\n",
            file.path(db)
              .file_name()
              .unwrap_or_else(|| "<unknown>".as_ref())
              .to_string_lossy(),
            error,
        ))
          .accumulate(db);
    }
}

#[salsa::accumulator]
struct Log(String);

impl Log {
    fn info(db: &dyn salsa::Database, message: String) {
        Log(message).accumulate(db);
    }
}
