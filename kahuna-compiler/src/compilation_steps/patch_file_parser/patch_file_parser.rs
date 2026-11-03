use crate::compilation_steps::patch_file_parser::patch_graph::PatchGraph;
use crate::inputs::File;
use eyre::Result;

/// Parses patch.json. Returns error if parsing fails.
pub fn parse_patch_graph(contents: &str, _file: File) -> Result<PatchGraph> {
    contents.parse::<PatchGraph>()
}
