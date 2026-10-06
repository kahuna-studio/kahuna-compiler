use crate::inputs::File;
use crate::compilation_steps::instrument_file_parser::instrument_metadata::InstrumentMetadata;
use eyre::Result;

/// Parses instrument.toml. Returns error if parsing fails.
pub fn parse_instrument_metadata(contents: &str, _file: File) -> Result<InstrumentMetadata> {
    InstrumentMetadata::from_str(contents)
}
