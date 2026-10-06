use std::path::Path;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use eyre::Result;
use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};
use salsa::Storage;

const DEBOUNCE: Duration = Duration::from_millis(300);

#[salsa::db]
#[derive(Clone)]
pub struct CompilerDatabase {
    pub storage: Storage<Self>,
    file_watcher: Arc<Mutex<Debouncer<RecommendedWatcher>>>,
}

#[salsa::db]
impl salsa::Database for CompilerDatabase {}

impl CompilerDatabase {
    pub fn new(tx: mpsc::Sender<DebounceEventResult>) -> Result<Self> {
        Ok(Self {
            storage: Storage::new(None),
            file_watcher: Arc::new(Mutex::new(new_debouncer(DEBOUNCE, tx)?)),
        })
    }

    pub fn watch(&self, dir: &Path, mode: RecursiveMode) -> Result<()> {
        let mut debouncer = self
          .file_watcher
          .lock()
          .map_err(|_| eyre::eyre!("File watcher mutex poisoned"))?;
        debouncer.watcher().watch(dir, mode)?;
        Ok(())
    }
}
