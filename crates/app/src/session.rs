use std::io;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;

use domovoi_agent::{Agent, AgentAnswer, AgentContext, AgentError, AgentLimits, ToolStep};
use domovoi_downloader::CatalogEntry;
use domovoi_dups::{
    CloneJournal, ClonePlan, CloneReplaceError, CloneReplaced, CloudFolders, DuplicateGroup, DuplicateOptions,
    DuplicateProgress, Duplicates, HashingPool, SearchError, find_duplicates,
};
use domovoi_index::{
    CacheError, CacheHeader, Enrichment, Index, IndexRoot, ModelEvent, NodeId, Reclaimable, open_cache, write_cache,
};
use domovoi_inference::{CancelFlag, InferenceBackend};
use domovoi_rules::{
    CaseRule, ConfigOverrides, Denylist, Recommendations, RestoreError, RuleSet, TrashJournal, TrashOutcome, Tray,
    running_bundle_ids,
};
use domovoi_scanner::{
    CancelToken, ScanError, ScanOptions, ScanProgress, Since, VolumeEvent, VolumeEventStream, current_event_id, scan,
};

use crate::cache_opening::CacheOpening;
use crate::event_effect::EventEffect;
use crate::group_paths::GroupPaths;
use crate::session_error::SessionError;
use crate::session_paths::SessionPaths;

#[derive(Debug)]
pub struct Session {
    root: PathBuf,
    paths: SessionPaths,
    header: CacheHeader,
    rules: RuleSet,
    denylist: Denylist,
    enrichment: Enrichment,
    cloud_folders: CloudFolders,
    index: Option<Index>,
    recommendations: Option<Recommendations>,
    duplicates: Option<Duplicates>,
    tray: Tray,
    trash_journal: TrashJournal,
    clone_journal: CloneJournal,
    last_event_id: u64,
    workers: NonZeroUsize,
}

impl Session {
    pub fn open(root: &Path, paths: &SessionPaths, app_version: &str) -> io::Result<Self> {
        paths.create_directories()?;
        let mut trash_journal = TrashJournal::open(&paths.trash_journal)?;
        let mut clone_journal = CloneJournal::open(&paths.clone_journal)?;
        clone_journal.sweep(&mut trash_journal)?;
        let home =
            std::env::home_dir().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "home directory is unknown"))?;
        Ok(Self {
            root: root.to_path_buf(),
            paths: paths.clone(),
            header: CacheHeader::current(root, app_version)?,
            rules: RuleSet::for_volume(root, &ConfigOverrides::default())?,
            denylist: Denylist::new(CaseRule::Insensitive)?,
            enrichment: Enrichment::default(),
            cloud_folders: CloudFolders::of_home(&home),
            index: None,
            recommendations: None,
            duplicates: None,
            tray: Tray::default(),
            trash_journal,
            clone_journal,
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

    pub fn rules(&self) -> &RuleSet {
        &self.rules
    }

    pub fn recommendations(&self) -> Option<&Recommendations> {
        self.recommendations.as_ref()
    }

    pub fn duplicates(&self) -> Option<&Duplicates> {
        self.duplicates.as_ref()
    }

    pub fn tray(&self) -> &Tray {
        &self.tray
    }

    pub fn trash_journal(&self) -> &TrashJournal {
        &self.trash_journal
    }

    pub fn last_event_id(&self) -> u64 {
        self.last_event_id
    }

    pub fn open_cached(&mut self) -> Result<CacheOpening, CacheError> {
        match open_cache(&self.paths.cache, &self.header) {
            Ok(cached) => {
                self.last_event_id = cached.last_event_id;
                self.install(cached.index).map_err(CacheError::Io)?;
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
        self.last_event_id = started_at;
        self.install(Index::build(&report, root, &self.enrichment))
            .map_err(SessionError::Io)?;
        self.persist().map_err(SessionError::Io)
    }

    pub fn watch(&self) -> io::Result<(VolumeEventStream, Receiver<VolumeEvent>)> {
        let (sender, events) = mpsc::channel();
        let stream = VolumeEventStream::watch(&self.root, Since::Event(self.last_event_id), sender)?;
        Ok((stream, events))
    }

    pub fn apply_events(&mut self, events: &[VolumeEvent]) -> Result<EventEffect, SessionError> {
        if self.index.is_none() {
            return Ok(EventEffect::Ignored);
        }
        let mut changed = Vec::new();
        let mut last_event_id = self.last_event_id;
        for event in events {
            match event {
                VolumeEvent::Changed { path, id, .. } => {
                    changed.push(path.clone());
                    last_event_id = last_event_id.max(*id);
                }
                VolumeEvent::HistoryDone { id } => last_event_id = last_event_id.max(*id),
                VolumeEvent::RootChanged | VolumeEvent::Unmount { .. } => return Ok(EventEffect::RescanRequired),
            }
        }
        self.rescan_paths(&changed)?;
        self.last_event_id = last_event_id;
        Ok(if changed.is_empty() {
            EventEffect::Ignored
        } else {
            EventEffect::Applied
        })
    }

    pub fn persist(&self) -> io::Result<()> {
        match &self.index {
            Some(index) => write_cache(&self.paths.cache, &self.header, self.last_event_id, index),
            None => Ok(()),
        }
    }

    pub fn refresh_recommendations(&mut self) {
        self.recommendations = self
            .index
            .as_ref()
            .map(|index| Recommendations::build(index, &self.rules, &self.denylist, &running_bundle_ids(), Vec::new()));
    }

    pub fn find_duplicates(
        &mut self,
        progress: Option<SyncSender<DuplicateProgress>>,
        cancel: CancelToken,
    ) -> Result<(), SearchError> {
        let index = self
            .index
            .as_ref()
            .expect("duplicates are searched in a scanned volume");
        let pool = HashingPool::new(Some(self.workers)).map_err(SearchError::Io)?;
        let options = DuplicateOptions {
            progress,
            cancel,
            ..DuplicateOptions::new(self.cloud_folders.clone())
        };
        self.duplicates = Some(find_duplicates(index, &pool, &options)?);
        Ok(())
    }

    pub fn ask(
        &self,
        backend: &mut dyn InferenceBackend,
        model: &CatalogEntry,
        limits: AgentLimits,
        question: &str,
        cancel: &CancelFlag,
        observer: &mut dyn FnMut(&ToolStep),
    ) -> Result<AgentAnswer, AgentError> {
        let index = self.index.as_ref().expect("questions are asked about a scanned volume");
        let context = AgentContext {
            index,
            rules: &self.rules,
            denylist: &self.denylist,
            totals: self
                .recommendations
                .as_ref()
                .map(|recommendations| recommendations.totals)
                .unwrap_or_default(),
        };
        Agent::new(backend, model.template, limits)
            .calling_a_tool_first()
            .ask(&context, question, cancel, observer)
    }

    pub fn apply_model_event(&mut self, event: &ModelEvent) {
        if let Some(index) = self.index.as_mut() {
            index.apply_model_event(event, &mut self.enrichment);
            self.refresh_recommendations();
        } else {
            self.enrichment.record_model_event(event);
        }
    }

    pub fn add_to_tray(&mut self, node: NodeId, rule_id: Option<&str>) {
        let index = self
            .index
            .as_ref()
            .expect("the tray collects nodes of a scanned volume");
        self.tray.add(index, node, rule_id);
    }

    pub fn remove_from_tray(&mut self, node: NodeId) {
        self.tray.remove(node);
    }

    pub fn tray_reclaimable(&self) -> io::Result<Reclaimable> {
        match &self.index {
            Some(index) => self.tray.reclaimable(index),
            None => Ok(Reclaimable::default()),
        }
    }

    pub fn move_tray_to_trash(&mut self) -> Result<Vec<TrashOutcome>, SessionError> {
        let index = self
            .index
            .as_ref()
            .expect("the tray collects nodes of a scanned volume");
        let outcomes = self
            .tray
            .move_to_trash(index, &self.denylist, &mut self.trash_journal)
            .map_err(SessionError::Io)?;
        let moved: Vec<PathBuf> = outcomes
            .iter()
            .filter(|outcome| outcome.result.is_ok())
            .map(|outcome| outcome.path.clone())
            .collect();
        self.rescan_paths(&moved)?;
        Ok(outcomes)
    }

    pub fn restore(&mut self, position: usize) -> Result<PathBuf, RestoreError> {
        let restored = self.trash_journal.restore(position)?;
        self.rescan_paths(std::slice::from_ref(&restored))
            .map_err(|error| RestoreError::Io(io::Error::other(error.to_string())))?;
        Ok(restored)
    }

    pub fn clone_plan(&self, keep: NodeId, copy: NodeId) -> Result<ClonePlan, CloneReplaceError> {
        let index = self.index.as_ref().expect("clones replace copies of a scanned volume");
        ClonePlan::prepare(index, keep, copy, &self.denylist, &self.cloud_folders)
    }

    pub fn replace_with_clone(&mut self, keep: NodeId, copy: NodeId) -> Result<CloneReplaced, CloneReplaceError> {
        let index = self.index.as_ref().expect("clones replace copies of a scanned volume");
        let copy_path = index.path(copy);
        let replaced = self
            .clone_plan(keep, copy)?
            .execute(&mut self.trash_journal, &mut self.clone_journal)?;
        self.rescan_paths(&[copy_path])
            .map_err(|error| CloneReplaceError::Io(io::Error::other(error.to_string())))?;
        Ok(replaced)
    }

    fn install(&mut self, index: Index) -> io::Result<()> {
        let tray_paths = self.tray_paths();
        let duplicate_paths = self.duplicate_paths();
        self.index = Some(index);
        self.remap(&tray_paths, duplicate_paths)?;
        self.refresh_recommendations();
        Ok(())
    }

    fn rescan_paths(&mut self, paths: &[PathBuf]) -> Result<(), SessionError> {
        if paths.is_empty() {
            return Ok(());
        }
        let tray_paths = self.tray_paths();
        let duplicate_paths = self.duplicate_paths();
        let index = self.index.as_mut().expect("only a scanned volume is rescanned");
        let mut outermost: Vec<&PathBuf> = paths.iter().collect();
        outermost.sort();
        outermost.dedup_by(|inner, outer| inner.starts_with(outer.as_path()));
        for path in outermost {
            index.rescan(path, &ScanOptions::default(), &self.enrichment)?;
        }
        self.remap(&tray_paths, duplicate_paths).map_err(SessionError::Io)?;
        self.refresh_recommendations();
        Ok(())
    }

    fn tray_paths(&self) -> Vec<(PathBuf, Option<String>)> {
        self.tray
            .items()
            .iter()
            .map(|item| (item.path.clone(), item.rule_id.clone()))
            .collect()
    }

    fn duplicate_paths(&self) -> Option<Vec<GroupPaths>> {
        let index = self.index.as_ref()?;
        let duplicates = self.duplicates.as_ref()?;
        Some(
            duplicates
                .groups
                .iter()
                .map(|group| GroupPaths {
                    size: group.size,
                    fully_hashed: group.fully_hashed,
                    members: std::iter::once(group.keep)
                        .chain(group.copies.iter().map(|copy| copy.node))
                        .map(|member| index.path(member))
                        .collect(),
                })
                .collect(),
        )
    }

    fn remap(
        &mut self,
        tray_paths: &[(PathBuf, Option<String>)],
        duplicate_paths: Option<Vec<GroupPaths>>,
    ) -> io::Result<()> {
        let index = self.index.as_ref().expect("remapping follows a new index");
        let mut tray = Tray::default();
        for (path, rule_id) in tray_paths {
            if let Some(node) = index.find(path) {
                tray.add(index, node, rule_id.as_deref());
            }
        }
        self.tray = tray;
        if let (Some(groups), Some(duplicates)) = (duplicate_paths, self.duplicates.as_mut()) {
            let mut remapped = Vec::new();
            for group in groups {
                let members: Vec<NodeId> = group.members.iter().filter_map(|member| index.find(member)).collect();
                if members.len() > 1 {
                    remapped.push(DuplicateGroup::of_members(
                        index,
                        group.size,
                        group.fully_hashed,
                        &members,
                    )?);
                }
            }
            duplicates.groups = remapped;
        }
        Ok(())
    }
}
