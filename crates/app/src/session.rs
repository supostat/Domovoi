use std::collections::BTreeSet;
use std::io;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;

use domovoi_index::{CacheError, CacheHeader, Enrichment, Index, IndexRoot, open_cache, write_cache};
use domovoi_rules::{CaseRule, ConfigOverrides, Denylist, Recommendations, RuleSet};
use domovoi_scanner::{
    CancelToken, ScanError, ScanOptions, ScanProgress, Since, VolumeEvent, VolumeEventStream, current_event_id, scan,
};

use crate::cache_opening::CacheOpening;
use crate::event_effect::EventEffect;
use crate::session_error::SessionError;

#[derive(Debug)]
pub struct Session {
    root: PathBuf,
    cache: PathBuf,
    header: CacheHeader,
    rules: RuleSet,
    denylist: Denylist,
    enrichment: Enrichment,
    index: Option<Index>,
    recommendations: Option<Recommendations>,
    last_event_id: u64,
    workers: NonZeroUsize,
}

impl Session {
    pub fn open(root: &Path, cache: &Path, app_version: &str) -> io::Result<Self> {
        Ok(Self {
            root: root.to_path_buf(),
            cache: cache.to_path_buf(),
            header: CacheHeader::current(root, app_version)?,
            rules: RuleSet::for_volume(root, &ConfigOverrides::default())?,
            denylist: Denylist::new(CaseRule::Insensitive)?,
            enrichment: Enrichment::default(),
            index: None,
            recommendations: None,
            last_event_id: 0,
            workers: thread::available_parallelism()?,
        })
    }

    pub fn workers(&self) -> usize {
        self.workers.get()
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn index(&self) -> Option<&Index> {
        self.index.as_ref()
    }

    pub fn recommendations(&self) -> Option<&Recommendations> {
        self.recommendations.as_ref()
    }

    pub fn last_event_id(&self) -> u64 {
        self.last_event_id
    }

    pub fn open_cached(&mut self) -> Result<CacheOpening, CacheError> {
        match open_cache(&self.cache, &self.header) {
            Ok(cached) => {
                self.last_event_id = cached.last_event_id;
                self.index = Some(cached.index);
                Ok(CacheOpening::Opened)
            }
            Err(CacheError::Io(error)) if error.kind() == io::ErrorKind::NotFound => Ok(CacheOpening::Absent),
            Err(CacheError::Mismatch | CacheError::Corrupt(_)) => Ok(CacheOpening::Reset),
            Err(error) => Err(error),
        }
    }

    pub fn scan(
        &mut self,
        progress: Option<SyncSender<ScanProgress>>,
        cancel: CancelToken,
    ) -> Result<(), SessionError> {
        let started_at = current_event_id();
        let options = ScanOptions {
            workers: Some(self.workers),
            progress,
            cancel,
            ..ScanOptions::default()
        };
        let report = scan(&self.root, &options)?;
        let root = IndexRoot::for_path(&self.root).map_err(ScanError::Root)?;
        self.index = Some(Index::build(&report, root, &self.enrichment));
        self.last_event_id = started_at;
        self.persist().map_err(SessionError::Cache)
    }

    pub fn watch(&self) -> io::Result<(VolumeEventStream, Receiver<VolumeEvent>)> {
        let (sender, events) = mpsc::channel();
        let stream = VolumeEventStream::watch(&self.root, Since::Event(self.last_event_id), sender)?;
        Ok((stream, events))
    }

    pub fn apply_event(&mut self, event: &VolumeEvent) -> Result<EventEffect, ScanError> {
        let Some(index) = self.index.as_mut() else {
            return Ok(EventEffect::Ignored);
        };
        match event {
            VolumeEvent::Changed { path, id, .. } => {
                index.rescan(path, &ScanOptions::default(), &self.enrichment)?;
                self.last_event_id = *id;
                Ok(EventEffect::Applied)
            }
            VolumeEvent::HistoryDone { id } => {
                self.last_event_id = self.last_event_id.max(*id);
                Ok(EventEffect::Ignored)
            }
            VolumeEvent::RootChanged | VolumeEvent::Unmount { .. } => Ok(EventEffect::RescanRequired),
        }
    }

    pub fn persist(&self) -> io::Result<()> {
        match &self.index {
            Some(index) => write_cache(&self.cache, &self.header, self.last_event_id, index),
            None => Ok(()),
        }
    }

    pub fn refresh_recommendations(&mut self, running_bundle_ids: &BTreeSet<String>) -> io::Result<()> {
        self.recommendations = match &self.index {
            Some(index) => Some(Recommendations::build(
                index,
                &self.rules,
                &self.denylist,
                running_bundle_ids,
                Vec::new(),
            )?),
            None => None,
        };
        Ok(())
    }
}
