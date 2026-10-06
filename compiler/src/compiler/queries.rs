use crate::inputs::{File, Workspace};
use crate::diagnostics::{Diagnostic, Log};
use crate::instrument_metadata::InstrumentMetadata;
use crate::patch_graph::PatchGraph;

const PATCH_FILE: &str = "patch.json";

/// Entry point of the compilation. Returns the number of patch nodes for now (placeholder).
#[salsa::tracked]
pub fn compile_package(db: &dyn salsa::Database, workspace: Workspace) -> usize {
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
pub fn parse_instrument_metadata(db: &dyn salsa::Database, file: File) -> Option<InstrumentMetadata> {
    let Some(contents) = file.contents(db).as_deref() else {
        Diagnostic::push_error(db, file, eyre::eyre!("File not found"));
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
pub fn parse_patch_graph(db: &dyn salsa::Database, file: File) -> Option<PatchGraph> {
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
