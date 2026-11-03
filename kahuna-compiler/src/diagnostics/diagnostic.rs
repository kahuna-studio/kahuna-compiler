use eyre::Report;
use salsa::Accumulator;
use crate::inputs::File;

#[salsa::accumulator]
pub struct Diagnostic(pub String);

impl Diagnostic {
    pub fn push_error(db: &dyn salsa::Database, file: File, error: Report) {
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
