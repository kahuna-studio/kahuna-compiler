#![allow(unreachable_patterns)]

use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use dashmap::DashMap;
use dashmap::mapref::entry::Entry;
use eyre::{Report, Result, eyre};
use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};
use salsa::{Accumulator, Setter, Storage};

use kahuna_compiler::instrument_metadata::InstrumentMetadata;

// ------------------- main -------------------

fn main() -> Result<()> {
    let instrument_path: PathBuf = std::env::args_os()
        .nth(1)
        .ok_or_else(|| eyre!("Usage: ./kahuna-compiler /path/to/instrument.toml"))?
        .into();

    let instrument_path = instrument_path.canonicalize()?;

    if instrument_path.file_name() != Some(std::ffi::OsStr::new("instrument.toml")) {
        return Err(eyre!(
            "Expected a file named 'instrument.toml', got: {}",
            instrument_path.display()
        ));
    }

    let base_dir = instrument_path.parent().unwrap().canonicalize()?;

    let (tx, rx) = mpsc::channel();
    let mut db = KahunaCompilerDatabase::new(tx, base_dir.clone());

    // Watch upfront
    {
        let mut watcher = db.file_watcher.lock().unwrap();
        watcher
            .watcher()
            .watch(&instrument_path, RecursiveMode::NonRecursive)?;
        watcher
            .watcher()
            .watch(&base_dir, RecursiveMode::Recursive)?;
    }

    // Ensure the instrument file exists as salsa input
    let initial = db.input(PathBuf::from("instrument.toml"))?;

    loop {
        let sum = compile_package(&db, initial);
        let diagnostics = compile_package::accumulated::<Diagnostic>(&db, initial);
        let logs = compile_package::accumulated::<Log>(&db, initial);

        if diagnostics.is_empty() {
            println!("✓ Compilation successful (Sum: {sum})");
        } else {
            for diagnostic in diagnostics {
                println!("{}", diagnostic.0);
            }
        }

        for log_message in logs {
            print!("{}\n", log_message.0);
        }

        // Wait for fs events
        for event in rx.recv()?.unwrap() {
            let path = match event.path.canonicalize() {
                Ok(p) => p,
                Err(_) => continue,
            };

            let contents = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let relative_path = path.strip_prefix(&base_dir)?.to_path_buf();

            let file = match db.files.get(&relative_path) {
                Some(file) => *file,
                None => {
                    let file = File::new(&db, relative_path.clone(), contents.clone());
                    db.files.insert(relative_path, file);
                    continue;
                }
            };
            file.set_contents(&mut db).to(contents);
        }
    }
}

// ------------------- salsa inputs -------------------

#[salsa::input]
struct File {
    path: PathBuf,
    #[returns(ref)]
    contents: String,
}

#[salsa::db]
trait Db: salsa::Database {
    fn input(&self, path: PathBuf) -> Result<File>;
}

#[salsa::db]
#[derive(Clone)]
struct KahunaCompilerDatabase {
    storage: Storage<Self>,
    base_dir: PathBuf,
    logs: Arc<Mutex<Vec<String>>>,
    files: DashMap<PathBuf, File>,
    file_watcher: Arc<Mutex<Debouncer<RecommendedWatcher>>>,
}

impl KahunaCompilerDatabase {
    fn new(tx: mpsc::Sender<DebounceEventResult>, base_dir: PathBuf) -> Self {
        let logs: Arc<Mutex<Vec<String>>> = Default::default();

        Self {
            storage: Storage::new(Some(Box::new({
                let logs = logs.clone();
                move |event| {
                    if let salsa::EventKind::WillExecute { .. } = event.kind {
                        logs.lock().unwrap().push(format!("{event:?}"));
                    }
                }
            }))),
            base_dir,
            logs,
            files: DashMap::new(),
            file_watcher: Arc::new(Mutex::new(
                new_debouncer(Duration::from_secs(1), tx).unwrap(),
            )),
        }
    }
}

#[salsa::db]
impl salsa::Database for KahunaCompilerDatabase {}

#[salsa::db]
impl Db for KahunaCompilerDatabase {
    fn input(&self, path: PathBuf) -> Result<File> {
        let path = path.canonicalize()?;
        Ok(match self.files.entry(path.clone()) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => {
                let contents = std::fs::read_to_string(&path)?;
                *e.insert(File::new(self, path, contents))
            }
        })
    }
}

// ------------------- diagnostics -------------------

#[salsa::accumulator]
struct Diagnostic(String);

impl Diagnostic {
    fn push_error(db: &dyn Db, file: File, error: Report) {
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
    fn info(db: &dyn Db, message: String) {
        Log(message).accumulate(db);
    }
}

// ------------------- compiler -------------------

#[salsa::tracked]
struct ParsedFile<'db> {
    value: u32,
    #[returns(ref)]
    links: Vec<ParsedFile<'db>>,
}

/// Parse instrument metadata from the instrument.toml file
#[salsa::tracked]
fn parse_instrument_metadata(db: &dyn Db, manifest_file: File) -> InstrumentMetadata {
    Log::info(
        db,
        format!(
            "Parsing instrument metadata: {}",
            manifest_file.path(db).display()
        ),
    );
    
    let contents = manifest_file.contents(db).clone();
    match InstrumentMetadata::from_str(&contents) {
        Ok(metadata) => {
            Log::info(
                db,
                format!(
                    "Loaded instrument '{}' by {} (version {})",
                    metadata.name, metadata.vendor, metadata.version
                ),
            );
            metadata
        }
        Err(e) => {
            Diagnostic::push_error(db, manifest_file, e);
            // Return a dummy value on error
            InstrumentMetadata {
                name: "error".to_string(),
                vendor: "error".to_string(),
                version: "0.0.0".to_string(),
                unit_crates: Default::default(),
            }
        }
    }
}

#[salsa::tracked]
fn compile_package(db: &dyn Db, manifest_file: File) -> u32 {
    Log::info(db, format!("Compiling package: {}", manifest_file.path(db).display()));
    
    // Parse instrument metadata
    let _metadata = parse_instrument_metadata(db, manifest_file);
    
    // For now, just return 1 to indicate success
    // Later steps will use this metadata for code generation
    1
}
