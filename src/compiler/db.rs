use salsa::Storage;
use std::cell::RefCell;
use std::path::PathBuf;

thread_local! {
    static LOG: RefCell<Vec<&'static str>> = RefCell::new(Vec::new());
}

fn push_log(name: &'static str) {
    LOG.with(|l| l.borrow_mut().push(name));
}

pub fn take_log() -> Vec<&'static str> {
  LOG.replace(Vec::new())
}

//
// Database
//

#[salsa::db]
#[derive(Default)]
pub struct KahunaCompilerDatabase {
    storage: Storage<Self>,
}

#[salsa::db]
impl salsa::Database for KahunaCompilerDatabase {}

//
// File model
//

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FileKind {
    Source,
    Patch,
}

#[salsa::input]
pub struct File {
    pub path: PathBuf,
    pub contents: String,
    pub kind: FileKind,
}

#[salsa::input]
pub struct FileSet {
    pub files: Vec<File>,
}

//
// Fake compiler data types
//

#[derive(Clone, Debug, PartialEq)]
pub struct Ast {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypedAst {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Graph {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RustFile {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AppCode {
    pub id: String,
}

//
// Compiler phases (Salsa tracked)
//

#[salsa::tracked]
fn parse(db: &dyn salsa::Database, file: File) -> Ast {
    push_log("parse");
    Ast {
        id: format!("ast:{:?}", file.path(db)),
    }
}

#[salsa::tracked]
fn typecheck(db: &dyn salsa::Database, file: File) -> TypedAst {
    push_log("typecheck");
    let parsed = parse(db, file);

    TypedAst {
        id: format!("typed:{:?}", file.path(db)),
    }
}

#[salsa::tracked]
fn build_graph(db: &dyn salsa::Database, file: File) -> Graph {
    push_log("build_graph");
    let typed = typecheck(db, file);

    Graph {
        id: format!("graph:{:?}", file.path(db)),
    }
}

#[salsa::tracked]
fn transpile(db: &dyn salsa::Database, file: File) -> RustFile {
    push_log("transpile");
    let graph = build_graph(db, file);

    RustFile {
        id: format!("rust:{:?}", file.path(db)),
    }
}

#[salsa::tracked]
fn generate_app(db: &dyn salsa::Database, files: FileSet) -> AppCode {
    push_log("generate_app");
    for f in files.files(db) {
        let rust = transpile(db, f);
    }
    AppCode { id: "app".into() }
}

//
// Tests proving wiring + memoization
//

#[cfg(test)]
mod tests {
    use super::*;

    fn make_file(db: &mut KahunaCompilerDatabase, name: &str, kind: FileKind) -> File {
        File::new(db, PathBuf::from(name), "dummy contents".into(), kind)
    }

    #[test]
    fn full_pipeline_runs_once_then_memoizes() {
        let mut db = KahunaCompilerDatabase::default();
        let file = make_file(&mut db, "main.kahuna", FileKind::Source);

        transpile(&db, file);
        let first = take_log();
        assert_eq!(
            first,
            vec!["transpile", "build_graph", "typecheck", "parse"]
        );

        transpile(&db, file);
        let first = take_log();
        assert_eq!(
            first.is_empty(),
            true
        );

        transpile(&db, file);
        let second = take_log();
        assert!(second.is_empty());
    }
}
