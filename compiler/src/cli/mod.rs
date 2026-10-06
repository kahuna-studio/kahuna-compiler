use std::path::{Path, PathBuf};
use eyre::{Result, eyre};

const INSTRUMENT_FILE: &str = "instrument.toml";

pub fn base_dir_from_args() -> Result<PathBuf> {
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
