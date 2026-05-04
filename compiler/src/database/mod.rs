use crate::ast::basics::File as AstFile;
use crate::compilation_steps::ast_construction::ast_mapper::map_to_ast;
use crate::compilation_steps::source_file_parser::source_file_parser::{parse_source_file};
use crate::compilation_steps::source_file_preprocessor::source_file_preprocessor::add_indentation_tags;
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


#[salsa::db]
#[derive(Default)]
pub struct KahunaCompilerDatabase {
  storage: Storage<Self>,
}

#[salsa::db]
impl salsa::Database for KahunaCompilerDatabase {}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FileKind {
  Source,
  Patch,
  PackageManifest,
}

#[salsa::input]
pub struct SourceFile {
  pub path: PathBuf,
  pub contents: String,
  pub kind: FileKind,
}

#[salsa::input]
pub struct FileSet {
  pub files: Vec<SourceFile>,
}

#[salsa::tracked]
pub fn get_source_file_ast(db: &dyn salsa::Database, file: SourceFile) -> AstFile {
  push_log("file_ast");
  let contents = file.contents(db);
  let preprocessed = add_indentation_tags(&contents);
  let mut pairs = parse_source_file(&preprocessed).expect("parse failed");
  let ast = map_to_ast(&mut pairs).expect("ast construction failed");

  ast
}


#[cfg(test)]
mod tests {
  use super::*;

  fn make_file(db: &mut KahunaCompilerDatabase, name: &str, kind: FileKind) -> SourceFile {
    SourceFile::new(db, PathBuf::from(name), "a = () -> 42".into(), kind)
  }

  #[test]
  fn full_pipeline_runs_once_then_memoizes() {
    let mut db = KahunaCompilerDatabase::default();
    let file = make_file(&mut db, "main.module.kh", FileKind::Source);

    get_source_file_ast(&db, file);
    let first = take_log();
    assert_eq!(first, vec!["file_ast"]);

    get_source_file_ast(&db, file);
    let second = take_log();
    assert!(second.is_empty());

    get_source_file_ast(&db, file);
    let third = take_log();
    assert!(third.is_empty());
  }
}
