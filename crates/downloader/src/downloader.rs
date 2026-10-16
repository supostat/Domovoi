use std::sync::Arc;
use std::thread;

use crate::catalog_entry::CatalogEntry;
use crate::download_control::DownloadControl;
use crate::download_handle::DownloadHandle;
use crate::download_job::DownloadJob;
use crate::download_progress::DownloadProgress;
use crate::model_source::ModelSource;
use crate::model_store::ModelStore;

const WORKER_NAME: &str = "domovoi-download";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Downloader {
    source: ModelSource,
    store: ModelStore,
}

impl Downloader {
    pub fn new(source: ModelSource, store: ModelStore) -> Self {
        Self { source, store }
    }

    pub fn store(&self) -> &ModelStore {
        &self.store
    }

    pub fn start(
        &self,
        entry: &CatalogEntry,
        on_progress: impl FnMut(DownloadProgress) + Send + 'static,
    ) -> DownloadHandle {
        let control = Arc::new(DownloadControl::default());
        let job = DownloadJob {
            entry: entry.clone(),
            source: self.source.clone(),
            store: self.store.clone(),
            control: Arc::clone(&control),
            on_progress: Box::new(on_progress),
        };
        let worker = thread::Builder::new()
            .name(WORKER_NAME.to_owned())
            .spawn(move || job.run())
            .expect("the download worker starts");
        DownloadHandle { control, worker }
    }
}
