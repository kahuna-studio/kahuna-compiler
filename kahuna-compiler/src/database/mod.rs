pub mod compiler_db;

pub use compiler_db::CompilerDatabase;

use crate::ast::basics::File as AstFile;
use crate::compilation_steps::ast_construction::ast_mapper::map_to_ast;
use crate::compilation_steps::source_file_parser::source_file_parser::{parse_source_file};
use crate::compilation_steps::source_file_preprocessor::source_file_preprocessor::add_indentation_tags;
use crate::compilation_steps::{instrument_file_parser, patch_file_parser};
use salsa::Storage;
use std::cell::RefCell;
use std::path::PathBuf;
use crate::compilation_steps::patch_code_parser::graph::PatchGraph as CodeParserPatchGraph;
use crate::compilation_steps::patch_file_parser::patch_graph::PatchGraph;
use crate::compilation_steps::patch_code_parser::parse_patch_code;
use crate::inputs::{File, Workspace};
use crate::diagnostics::{Diagnostic, Log};
use crate::compilation_steps::instrument_file_parser::instrument_metadata::InstrumentMetadata;

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

#[salsa::tracked]
pub fn get_patch_graph(db: &dyn salsa::Database, file: SourceFile) -> CodeParserPatchGraph {
  push_log("patch_file_graph");
  let contents = file.contents(db);
  let patch_graph = parse_patch_code(&contents).expect("parse failed");

  patch_graph
}

/// Entry point of the compilation. Returns the number of patch nodes for now (placeholder).
#[salsa::tracked]
pub fn compile_package(db: &dyn salsa::Database, workspace: Workspace) -> usize {
    let instrument_file = *workspace.instrument(db);
    let patch_file = *workspace.patch(db);

    Log::info(
        db,
        format!("Compiling package: {}", instrument_file.path(db).display()),
    );

    let _metadata = parse_instrument_metadata_tracked(db, instrument_file);

    match parse_patch_graph_tracked(db, patch_file) {
        Some(graph) => graph.nodes.len(),
        None => 0,
    }
}

/// Salsa-tracked wrapper for instrument.toml parsing. `None` means a diagnostic was emitted.
#[salsa::tracked]
pub fn parse_instrument_metadata_tracked(db: &dyn salsa::Database, file: File) -> Option<InstrumentMetadata> {
    let Some(contents) = file.contents(db).as_deref() else {
        Diagnostic::push_error(db, file, eyre::eyre!("File not found"));
        return None;
    };

    match instrument_file_parser::parse_instrument_metadata(contents, file) {
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

/// Salsa-tracked wrapper for patch.json parsing. A missing file is not an error: the instrument may have no patch yet.
#[salsa::tracked]
pub fn parse_patch_graph_tracked(db: &dyn salsa::Database, file: File) -> Option<PatchGraph> {
    let Some(contents) = file.contents(db).as_deref() else {
        Log::info(db, "No patch.json found; skipping patch parsing".to_string());
        return None;
    };

    match patch_file_parser::parse_patch_graph(contents, file) {
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
