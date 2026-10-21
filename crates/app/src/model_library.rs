use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Instant;

use domovoi_downloader::{
    Catalog, CatalogEntry, DownloadOutcome, Downloader, InstallError, InstalledModel, ModelSource, ModelStore,
    OrphanReason, PartialDownload, StoreReport, physical_memory,
};
use domovoi_index::ModelEvent;

use crate::assistant_settings::AssistantSettings;
use crate::install_error_text::install_error_text;
use crate::model_card_model::ModelCardModel;
use crate::model_card_state::ModelCardState;
use crate::model_download::ModelDownload;
use crate::model_screen_model::ModelScreenModel;
use crate::orphan_row::OrphanRow;
use crate::strings::{ADOPTION_RUNNING, FILE_NOT_IN_CATALOG, ORPHAN_DIGEST_MISMATCH, ORPHAN_NOT_IN_CATALOG};
use crate::tilde::tilde;
use crate::transfer_speed::TransferSpeed;

type Adoption = (String, JoinHandle<Result<ModelEvent, InstallError>>);

#[derive(Debug)]
pub struct ModelLibrary {
    catalog: Catalog,
    downloader: Downloader,
    report: Option<StoreReport>,
    scan: Option<JoinHandle<io::Result<StoreReport>>>,
    downloads: Vec<ModelDownload>,
    queued: Vec<String>,
    adoption: Option<Adoption>,
    failures: Vec<(String, &'static str)>,
}

impl ModelLibrary {
    pub fn new(store: ModelStore, catalog: Catalog, source: ModelSource) -> Self {
        let scan = {
            let store = store.clone();
            let catalog = catalog.clone();
            thread::spawn(move || store.scan(&catalog))
        };
        Self {
            catalog,
            downloader: Downloader::new(source, store),
            report: None,
            scan: Some(scan),
            downloads: Vec::new(),
            queued: Vec::new(),
            adoption: None,
            failures: Vec::new(),
        }
    }

    pub fn set_source(&mut self, source: ModelSource) {
        self.downloader = Downloader::new(source, self.downloader.store().clone());
    }

    pub fn is_busy(&self) -> bool {
        self.scan.is_some() || !self.downloads.is_empty() || self.adoption.is_some()
    }

    pub fn installed(&self, entry_id: &str) -> Option<(&CatalogEntry, &Path)> {
        let installed = self
            .report
            .as_ref()?
            .installed
            .iter()
            .find(|model| model.entry_id == entry_id)?;
        Some((self.catalog.entry(entry_id)?, installed.path.as_path()))
    }

    pub fn first_installed(&self) -> Option<&str> {
        self.report
            .as_ref()?
            .installed
            .first()
            .map(|model| model.entry_id.as_str())
    }

    pub fn shown_entry(&self, position: usize) -> Option<&CatalogEntry> {
        self.shown().get(position).copied()
    }

    pub fn screen_model(&self, settings: &AssistantSettings) -> ModelScreenModel {
        let orphans = self.report.as_ref().map_or_else(Vec::new, |report| {
            report
                .orphans
                .iter()
                .map(|orphan| OrphanRow {
                    path: tilde(&orphan.path),
                    bytes: fs::metadata(&orphan.path).map_or(0, |metadata| metadata.len()),
                    reason: match orphan.reason {
                        OrphanReason::NotInCatalog => ORPHAN_NOT_IN_CATALOG,
                        OrphanReason::DigestMismatch => ORPHAN_DIGEST_MISMATCH,
                    }
                    .to_owned(),
                })
                .collect()
        });
        ModelScreenModel {
            enabled: settings.enabled,
            memory_bytes: physical_memory(),
            cards: self
                .shown()
                .into_iter()
                .map(|entry| ModelCardModel::of_entry(entry, self.card_state(entry, settings)))
                .collect(),
            unload: settings.unload,
            source: settings.source_host(),
            orphans,
            #[cfg(feature = "claude-code")]
            claude_code: None,
        }
    }

    pub fn start(&mut self, position: usize) {
        let Some(entry) = self.shown_entry(position).cloned() else {
            return;
        };
        if self.report.is_none() {
            if !self.queued.contains(&entry.id) {
                self.queued.push(entry.id);
            }
            return;
        }
        self.start_entry(&entry);
    }

    fn start_entry(&mut self, entry: &CatalogEntry) {
        let Some(report) = &self.report else {
            return;
        };
        let busy = self.downloads.iter().any(|download| download.entry_id == entry.id)
            || report.installed.iter().any(|model| model.entry_id == entry.id);
        if busy {
            return;
        }
        let received = report
            .partials
            .iter()
            .find(|partial| partial.entry_id == entry.id)
            .map_or(0, |partial| partial.received);
        self.failures.retain(|(entry_id, _)| *entry_id != entry.id);
        let (sender, updates) = mpsc::channel();
        let handle = self.downloader.start(entry, move |progress| {
            let _ = sender.send(progress);
        });
        self.downloads.push(ModelDownload {
            entry_id: entry.id.clone(),
            handle,
            updates,
            received,
            speed: TransferSpeed::default(),
        });
    }

    pub fn pause(&mut self, position: usize) {
        self.unqueue(position);
        if let Some(download) = self.download_at(position) {
            download.handle.pause();
        }
    }

    pub fn pause_all(&self) {
        for download in &self.downloads {
            download.handle.pause();
        }
    }

    pub fn cancel(&mut self, position: usize) -> Result<(), InstallError> {
        self.unqueue(position);
        if let Some(download) = self.download_at(position) {
            download.handle.cancel();
            return Ok(());
        }
        let Some(entry_id) = self.shown_entry(position).map(|entry| entry.id.clone()) else {
            return Ok(());
        };
        let Some(report) = self.report.as_mut() else {
            return Ok(());
        };
        if let Some(partial) = report.partials.iter().find(|partial| partial.entry_id == entry_id) {
            self.downloader.store().remove(&partial.path)?;
            report.partials.retain(|partial| partial.entry_id != entry_id);
        }
        self.failures.retain(|(failed, _)| *failed != entry_id);
        Ok(())
    }

    pub fn remove(&mut self, position: usize) -> Result<Option<ModelEvent>, InstallError> {
        let Some(entry_id) = self.shown_entry(position).map(|entry| entry.id.clone()) else {
            return Ok(None);
        };
        let Some(report) = self.report.as_mut() else {
            return Ok(None);
        };
        let Some(installed) = report.installed.iter().find(|model| model.entry_id == entry_id) else {
            return Ok(None);
        };
        let event = self.downloader.store().remove(&installed.path)?;
        report.installed.retain(|model| model.entry_id != entry_id);
        Ok(Some(event))
    }

    pub fn trash_orphan(&mut self, position: usize) -> Result<(), InstallError> {
        let Some(report) = self.report.as_mut() else {
            return Ok(());
        };
        if let Some(orphan) = report.orphans.get(position) {
            self.downloader.store().remove(&orphan.path)?;
            report.orphans.remove(position);
        }
        Ok(())
    }

    pub fn adopt(&mut self, source: PathBuf) -> Result<(), &'static str> {
        if self.adoption.is_some() {
            return Err(ADOPTION_RUNNING);
        }
        let size = fs::metadata(&source).map_err(|_| FILE_NOT_IN_CATALOG)?.len();
        let entry = self
            .catalog
            .entries
            .iter()
            .find(|entry| entry.size == size)
            .cloned()
            .ok_or(FILE_NOT_IN_CATALOG)?;
        let store = self.downloader.store().clone();
        self.failures.retain(|(failed, _)| *failed != entry.id);
        self.adoption = Some((entry.id.clone(), thread::spawn(move || store.adopt(&source, &entry))));
        Ok(())
    }

    pub fn finish_scan(&mut self) -> Result<Vec<ModelEvent>, String> {
        let Some(scan) = self.scan.take() else {
            return Ok(Vec::new());
        };
        match scan.join().expect("the model folder scan does not panic") {
            Ok(report) => {
                let events = report
                    .installed
                    .iter()
                    .map(|model| ModelEvent::Downloaded {
                        path: model.path.clone(),
                    })
                    .collect();
                self.report = Some(report);
                Ok(events)
            }
            Err(error) => {
                self.report = Some(StoreReport::default());
                Err(error.to_string())
            }
        }
    }

    pub fn poll(&mut self, now: Instant) -> (Vec<ModelEvent>, Vec<String>) {
        let mut events = Vec::new();
        let mut problems = Vec::new();
        if self.scan.as_ref().is_some_and(JoinHandle::is_finished) {
            match self.finish_scan() {
                Ok(installed) => events.extend(installed),
                Err(problem) => problems.push(problem),
            }
            for entry_id in std::mem::take(&mut self.queued) {
                if let Some(entry) = self.catalog.entry(&entry_id).cloned() {
                    self.start_entry(&entry);
                }
            }
        }
        for download in &mut self.downloads {
            for progress in download.updates.try_iter() {
                download.received = progress.received;
                download.speed.record(now, progress.received);
            }
        }
        let (finished, running): (Vec<_>, Vec<_>) = std::mem::take(&mut self.downloads)
            .into_iter()
            .partition(|download| download.handle.is_finished());
        self.downloads = running;
        for download in finished {
            let entry_id = download.entry_id.clone();
            match download.handle.wait() {
                DownloadOutcome::Installed(event) => {
                    self.record_installed(&entry_id, &event);
                    events.push(event);
                }
                DownloadOutcome::Paused | DownloadOutcome::Cancelled => self.refresh_partial(&entry_id),
                DownloadOutcome::Failed(error) => {
                    self.failures.push((entry_id.clone(), install_error_text(&error)));
                    self.refresh_partial(&entry_id);
                }
            }
        }
        if self.adoption.as_ref().is_some_and(|(_, worker)| worker.is_finished()) {
            let (entry_id, worker) = self.adoption.take().expect("a finished adoption is present");
            match worker.join().expect("the model check does not panic") {
                Ok(event) => {
                    self.record_installed(&entry_id, &event);
                    events.push(event);
                }
                Err(error) => problems.push(install_error_text(&error).to_owned()),
            }
        }
        (events, problems)
    }

    fn shown(&self) -> Vec<&CatalogEntry> {
        self.catalog
            .entries
            .iter()
            .filter(|entry| {
                entry.primary || self.card_state(entry, &AssistantSettings::default()) != ModelCardState::Available
            })
            .collect()
    }

    fn unqueue(&mut self, position: usize) {
        if let Some(entry_id) = self.shown_entry(position).map(|entry| entry.id.clone()) {
            self.queued.retain(|queued| *queued != entry_id);
        }
    }

    fn download_at(&self, position: usize) -> Option<&ModelDownload> {
        let entry = self.shown_entry(position)?;
        self.downloads.iter().find(|download| download.entry_id == entry.id)
    }

    fn card_state(&self, entry: &CatalogEntry, settings: &AssistantSettings) -> ModelCardState {
        if let Some(download) = self.downloads.iter().find(|download| download.entry_id == entry.id) {
            return ModelCardState::Downloading {
                received: download.received,
                bytes_per_second: download.speed.bytes_per_second(),
            };
        }
        if self
            .adoption
            .as_ref()
            .is_some_and(|(entry_id, _)| *entry_id == entry.id)
        {
            return ModelCardState::Checking;
        }
        if let Some((_, reason)) = self.failures.iter().find(|(entry_id, _)| *entry_id == entry.id) {
            return ModelCardState::Failed {
                reason: (*reason).to_owned(),
            };
        }
        let Some(report) = &self.report else {
            return if self.downloader.store().model_path(entry).exists() {
                ModelCardState::Checking
            } else {
                ModelCardState::Available
            };
        };
        if report.installed.iter().any(|model| model.entry_id == entry.id) {
            if settings.active_entry.as_deref() == Some(entry.id.as_str()) {
                ModelCardState::Active
            } else {
                ModelCardState::Installed
            }
        } else if let Some(partial) = report.partials.iter().find(|partial| partial.entry_id == entry.id) {
            ModelCardState::Paused {
                received: partial.received,
            }
        } else {
            ModelCardState::Available
        }
    }

    fn record_installed(&mut self, entry_id: &str, event: &ModelEvent) {
        let (Some(report), ModelEvent::Downloaded { path }) = (self.report.as_mut(), event) else {
            return;
        };
        report.partials.retain(|partial| partial.entry_id != entry_id);
        report.installed.retain(|model| model.entry_id != entry_id);
        report.installed.push(InstalledModel {
            entry_id: entry_id.to_owned(),
            path: path.clone(),
        });
    }

    fn refresh_partial(&mut self, entry_id: &str) {
        let Some(entry) = self.catalog.entry(entry_id) else {
            return;
        };
        let path = self.downloader.store().partial_path(entry);
        let Some(report) = self.report.as_mut() else {
            return;
        };
        report.partials.retain(|partial| partial.entry_id != entry_id);
        if let Ok(metadata) = fs::metadata(&path) {
            report.partials.push(PartialDownload {
                entry_id: entry_id.to_owned(),
                path,
                received: metadata.len(),
            });
        }
    }
}
