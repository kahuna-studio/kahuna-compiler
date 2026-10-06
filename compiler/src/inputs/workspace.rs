use std::path::PathBuf;

const INSTRUMENT_FILE: &str = "instrument.toml";
const PATCH_FILE: &str = "patch.json";

/// A file below the instrument directory. `contents == None` means the file does not exist.
#[salsa::input]
pub struct File {
    pub path: PathBuf,
    #[returns(ref)]
    pub contents: Option<String>,
}

/// All compiler inputs, created once up front so queries never touch the file system.
#[salsa::input]
pub struct Workspace {
    pub instrument: File,
    pub patch: File,
}

pub const fn get_instrument_file() -> &'static str {
    INSTRUMENT_FILE
}

pub const fn get_patch_file() -> &'static str {
    PATCH_FILE
}
