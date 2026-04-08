#![allow(unreachable_patterns)]

pub mod compiler;
pub use compiler::lexer::lexer::lex as internal_parse;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossbeam_channel::{Sender, unbounded};
use dashmap::DashMap;
use dashmap::mapref::entry::Entry;
use eyre::{Report, Result, eyre};
use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};
use salsa::{Accumulator, Setter, Storage};

// ------------------- main -------------------

fn main() -> Result<()> {
    let package_path: PathBuf = std::env::args_os()
        .nth(1)
        .ok_or_else(|| eyre!("Usage: ./kahuna-compiler /path/to/package.toml"))?
        .into();

    let package_path = package_path.canonicalize()?;

    if package_path.file_name() != Some(std::ffi::OsStr::new("package.toml")) {
        return Err(eyre!(
            "Expected a file named 'package.toml', got: {}",
            package_path.display()
        ));
    }

    let src_dir = package_path.parent().unwrap().join("src").canonicalize()?;
    let base_dir = package_path.parent().unwrap().canonicalize()?;

    let (tx, rx) = unbounded();
    let mut db = KahunaCompilerDatabase::new(tx, base_dir);

    // Watch upfront
    {
        let mut watcher = db.file_watcher.lock().unwrap();
        watcher
            .watcher()
            .watch(&package_path, RecursiveMode::NonRecursive)?;
        watcher
            .watcher()
            .watch(&src_dir, RecursiveMode::Recursive)?;
    }

    // Ensure the package file exists as salsa input
    let initial = db.input(PathBuf::from("package.toml"))?;

    loop {
        let sum = compile_package(&db, initial);
        let diagnostics = compile_package::accumulated::<Diagnostic>(&db, initial);
        let logs = compile_package::accumulated::<Log>(&db, initial);

        if diagnostics.is_empty() {
            println!("Sum is: {sum}");
        } else {
            for diagnostic in diagnostics {
                println!("{}", diagnostic.0);
            }
        }

        for log_message in logs {
            print!("{}\n", log_message.0);
        }

        /*
        for log in db.logs.lock().unwrap().drain(..) {
            eprintln!("{log}");
        }
        */

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

            let relative_path = path.strip_prefix(&db.base_dir)?.to_path_buf();

            let file = match db.files.get(&relative_path) {
                Some(file) => *file,
                None => {
                    let file = File::new(&db, relative_path, contents.clone());
                    db.files.insert(path, file);
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
    fn new(tx: Sender<DebounceEventResult>, base_dir: PathBuf) -> Self {
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

#[salsa::tracked]
fn compile_package(db: &dyn Db, manifest_file: File) -> u32 {
    Log::info(db, format!("Compiling package: {}", manifest_file.path(db).display()));
    let parsed = parse(db, manifest_file);
    sum(db, parsed)
}

#[salsa::tracked]
fn parse(db: &dyn Db, input: File) -> ParsedFile<'_> {
    Log::info(db, format!("Parsing file: {}", input.path(db).display()));
    let mut lines = input.contents(db).lines();

    let value = match lines.next().map(|line| (line.parse::<u32>(), line)) {
        Some((Ok(num), _)) => num,
        Some((Err(e), line)) => {
            Diagnostic::push_error(
                db,
                input,
                Report::new(e).wrap_err(format!(
                    "First line ({line}) could not be parsed as an integer"
                )),
            );
            0
        }
        None => {
            Diagnostic::push_error(db, input, eyre!("File must contain an integer"));
            0
        }
    };

    let links = lines
        .filter_map(|path| {
            let relative_path = path.parse::<PathBuf>().ok()?;
            let link_path = input.path(db).parent().unwrap().join(relative_path);
            db.input(link_path).ok().map(|file| parse(db, file))
        })
        .collect();

    ParsedFile::new(db, value, links)
}

#[salsa::tracked]
fn sum<'db>(db: &'db dyn Db, input: ParsedFile<'db>) -> u32 {
    input.value(db)
        + input
            .links(db)
            .iter()
            .map(|&file| sum(db, file))
            .sum::<u32>()
}
