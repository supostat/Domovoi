use std::ffi::CStr;
use std::io;
use std::mem;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::thread;

use domovoi_agent::{Agent, AgentAnswer, AgentContext, AgentError, AgentLimits, ToolStep};
#[cfg(feature = "claude-code")]
use domovoi_agent::{ClaudeAnswer, ClaudeCode, ClaudeCodeError};
use domovoi_downloader::CatalogEntry;
use domovoi_dups::{
    CloneJournal, CloneJournalError, ClonePlan, CloneReplaceError, CloneReplaced, CloudFolders, DuplicateGroup,
    DuplicateOptions, DuplicateProgress, Duplicates, HashingPool, SearchError, find_duplicates,
};
use domovoi_index::{
    CacheError, CacheHeader, ChangedPath, DeltaApplied, DirectoryListing, Enrichment, Index, IndexRoot, ModelEvent,
    NodeId, Reclaimable, open_cache, query_screen_captures, write_cache,
};
use domovoi_inference::{CancelFlag, InferenceBackend};
use domovoi_log_sys::SystemLog;
use domovoi_rules::{
    AppRemoval, CaseRule, ConfigOverrides, DeleteRefusal, DeleteReport, DeleteSubject, Denylist,
    InstalledSimulatorRuntimes, Recommendations, RuleSet, Tray, running_bundle_ids, screen_capture_location,
};
use domovoi_scanner::{
    CancelToken, ScanError, ScanOptions, ScanProgress, Since, VolumeEvent, VolumeEventStream, current_event_id, scan,
};

use crate::cache_opening::CacheOpening;
use crate::clone_progress::CloneProgress;
use crate::clones_outcome::ClonesOutcome;
use crate::drop_verdict::DropVerdict;
use crate::event_effect::EventEffect;
use crate::event_request::EventRequest;
use crate::group_paths::GroupPaths;
use crate::member_stamp::MemberStamp;
use crate::pending_delta::PendingDelta;
use crate::question::Question;
use crate::remapping::Remapping;
use crate::session_error::SessionError;
use crate::session_paths::SessionPaths;
use crate::tray_model::TrayModel;
use crate::tray_summary::TraySummary;
use crate::tray_target::TrayTarget;

const LOG_SUBSYSTEM: &CStr = c"com.domovoi.app";
const LOG_CATEGORY: &CStr = c"session";

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
    clone_journal: CloneJournal,
    last_event_id: u64,
    workers: NonZeroUsize,
}

impl Session {
    pub fn open(root: &Path, paths: &SessionPaths, app_version: &str) -> io::Result<Self> {
        paths.create_directories()?;
        let mut clone_journal = open_clone_journal(&paths.clone_journal)?;
        clone_journal.sweep()?;
        Ok(Self {
            root: root.to_path_buf(),
            paths: paths.clone(),
            header: CacheHeader::current(root, app_version)?,
            rules: rules_for(root, &paths.home)?,
            denylist: Denylist::new(CaseRule::Insensitive)?,
            enrichment: Enrichment::default(),
            cloud_folders: CloudFolders::of_home(&paths.home),
            index: None,
            recommendations: None,
            duplicates: None,
            tray: Tray::default(),
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

    pub fn last_event_id(&self) -> u64 {
        self.last_event_id
    }

    pub fn open_cached(&mut self) -> Result<CacheOpening, CacheError> {
        match open_cache(&self.paths.cache, &self.header) {
            Ok(cached) => {
                self.last_event_id = cached.last_event_id;
                self.install(cached.index);
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
        self.refresh_screen_captures();
        self.rules = rules_for(&self.root, &self.paths.home).map_err(SessionError::Io)?;
        self.install(Index::build(&report, root, &self.enrichment));
        self.persist().map_err(SessionError::Io)
    }

    pub fn watch(&self) -> io::Result<(VolumeEventStream, Receiver<VolumeEvent>)> {
        let (sender, events) = mpsc::channel();
        let stream = VolumeEventStream::watch(&self.root, Since::Event(self.last_event_id), sender)?;
        Ok((stream, events))
    }

    pub fn delta_request(&self, events: &[VolumeEvent]) -> EventRequest {
        let index = self.index.as_ref().expect("events are watched on a scanned volume");
        let mut changed = Vec::new();
        let mut last_event_id = self.last_event_id;
        for event in events {
            match event {
                VolumeEvent::Changed {
                    path,
                    rescan_subtree,
                    id,
                } => {
                    changed.push(ChangedPath {
                        path: path.clone(),
                        subtree: *rescan_subtree,
                    });
                    last_event_id = last_event_id.max(*id);
                }
                VolumeEvent::HistoryDone { id } => last_event_id = last_event_id.max(*id),
                VolumeEvent::RootChanged | VolumeEvent::Unmount { .. } => return EventRequest::RescanRequired,
            }
        }
        EventRequest::Delta(PendingDelta {
            request: index.delta_request(&changed),
            last_event_id,
            touches_rule_sources: touches_rule_sources(index, &self.paths.home, &changed),
        })
    }

    pub fn apply_listings(
        &mut self,
        pending: &PendingDelta,
        listings: Vec<DirectoryListing>,
    ) -> Result<EventEffect, SessionError> {
        if pending.request.is_empty() {
            self.last_event_id = self.last_event_id.max(pending.last_event_id);
            return Ok(EventEffect::Ignored);
        }
        let duplicate_paths = self.duplicate_paths();
        self.refresh_screen_captures();
        let index = self.index.as_mut().expect("deltas apply to a scanned volume");
        let applied = index
            .apply_delta(&pending.request, listings, &self.enrichment)
            .map_err(SessionError::Stale)?;
        self.last_event_id = self.last_event_id.max(pending.last_event_id);
        match &applied {
            DeltaApplied::Spliced { .. } => self.remap(duplicate_paths, Remapping::Detached),
            DeltaApplied::Compacted { .. } => self.remap(duplicate_paths, Remapping::Everything),
        }
        if pending.touches_rule_sources {
            self.rules = rules_for(&self.root, &self.paths.home).map_err(SessionError::Io)?;
            self.refresh_recommendations();
            return Ok(EventEffect::Applied);
        }
        let index = self.index.as_ref().expect("deltas apply to a scanned volume");
        self.recommendations
            .as_mut()
            .expect("a scanned volume has recommendations")
            .apply_delta(index, &self.rules, &self.denylist, &running_bundle_ids(), &applied);
        Ok(EventEffect::Applied)
    }

    pub fn apply_events(&mut self, events: &[VolumeEvent]) -> Result<EventEffect, SessionError> {
        if self.index.is_none() {
            return Ok(EventEffect::Ignored);
        }
        match self.delta_request(events) {
            EventRequest::RescanRequired => Ok(EventEffect::RescanRequired),
            EventRequest::Delta(pending) => {
                let listings = pending.read(&ScanOptions::default())?;
                self.apply_listings(&pending, listings)
            }
        }
    }

    pub fn persist(&mut self) -> io::Result<()> {
        let duplicate_paths = self.duplicate_paths();
        let Some(index) = self.index.as_mut() else {
            return Ok(());
        };
        if index.compact_garbage() {
            self.remap(duplicate_paths, Remapping::Everything);
            self.refresh_recommendations();
        }
        let index = self.index.as_ref().expect("a persisted session holds an index");
        write_cache(&self.paths.cache, &self.header, self.last_event_id, index)
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
        question: &Question,
        cancel: &CancelFlag,
        observer: &mut dyn FnMut(&ToolStep),
    ) -> Result<AgentAnswer, AgentError> {
        Agent::new(backend, model.template, limits).calling_a_tool_first().ask(
            &self.agent_context(question),
            &question.text,
            cancel,
            observer,
        )
    }

    #[cfg(feature = "claude-code")]
    pub fn ask_claude(
        &self,
        claude: &ClaudeCode,
        question: &Question,
        claude_session: Option<&str>,
        cancel: &CancelFlag,
        observer: &mut dyn FnMut(&ToolStep),
    ) -> Result<ClaudeAnswer, ClaudeCodeError> {
        claude.ask(
            &self.agent_context(question),
            &question.text,
            claude_session,
            cancel,
            observer,
        )
    }

    fn agent_context<'a>(&'a self, question: &'a Question) -> AgentContext<'a> {
        AgentContext {
            index: self.index.as_ref().expect("questions are asked about a scanned volume"),
            rules: &self.rules,
            denylist: &self.denylist,
            totals: self
                .recommendations
                .as_ref()
                .map(|recommendations| recommendations.totals)
                .unwrap_or_default(),
            home: &self.paths.home,
            history: &question.history,
            focus: &question.focus,
        }
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

    pub fn add_duplicate_copy_to_tray(&mut self, copy: NodeId, kept: NodeId) {
        let index = self
            .index
            .as_ref()
            .expect("the tray collects nodes of a scanned volume");
        self.tray.add_duplicate_copy(index, copy, kept);
    }

    pub fn add_target_to_tray(&mut self, target: &TrayTarget) {
        let index = self
            .index
            .as_ref()
            .expect("the tray collects nodes of a scanned volume");
        match (&target.project, &target.rule_id) {
            (Some(project), Some(rule_id)) => self.tray.add_project_artifact(index, target.node, rule_id, project),
            _ => self.tray.add(index, target.node, target.rule_id.as_deref()),
        }
    }

    pub fn add_application_to_tray(&mut self, bundle: NodeId) -> bool {
        let index = self
            .index
            .as_ref()
            .expect("the tray collects nodes of a scanned volume");
        let Some(removal) = AppRemoval::of(index, bundle) else {
            return false;
        };
        self.tray.add_application(index, &removal);
        true
    }

    pub fn remove_from_tray(&mut self, node: NodeId) {
        self.tray.remove(node);
    }

    pub fn judge_drop(&self, paths: &[PathBuf]) -> DropVerdict {
        let index = self
            .index
            .as_ref()
            .expect("the tray collects nodes of a scanned volume");
        DropVerdict::judge(index, &self.denylist, &self.tray, paths)
    }

    pub fn drop_into_tray(&mut self, paths: &[PathBuf]) -> DropVerdict {
        let index = self
            .index
            .as_ref()
            .expect("the tray collects nodes of a scanned volume");
        let verdict = DropVerdict::judge(index, &self.denylist, &self.tray, paths);
        for node in &verdict.allowed {
            self.tray.add(index, *node, None);
        }
        verdict
    }

    pub fn tray_summary(&self) -> TraySummary {
        let index = self
            .index
            .as_ref()
            .expect("the tray collects nodes of a scanned volume");
        TraySummary {
            objects: self.tray.items().len(),
            bytes: TrayModel::estimated_sizes(index, &self.tray)
                .iter()
                .map(|size| size.bytes)
                .sum(),
        }
    }

    pub fn take_tray_for_delete(
        &mut self,
        sizes: &[Reclaimable],
    ) -> io::Result<(Vec<DeleteSubject>, Vec<DeleteRefusal>)> {
        self.denylist.refresh()?;
        let running = running_bundle_ids();
        let index = self
            .index
            .as_ref()
            .expect("the tray collects nodes of a scanned volume");
        let rules = self.rules.rules();
        Ok(self.tray.take_for_delete(index, &self.denylist, sizes, |rule_id| {
            rules
                .iter()
                .find(|rule| rule.id == rule_id)
                .and_then(|rule| rule.owner_bundle)
                .is_some_and(|owner| running.contains(owner))
        }))
    }

    pub fn settle_delete(&mut self, report: &DeleteReport) -> Result<(), SessionError> {
        let gone: Vec<PathBuf> = report
            .deleted
            .iter()
            .chain(&report.interrupted)
            .map(|subject| subject.path.clone())
            .collect();
        self.rescan_paths(&gone)
    }

    pub fn clone_assessment(&self, keep: NodeId, copy: NodeId) -> Result<(), CloneReplaceError> {
        let index = self.index.as_ref().expect("clones replace copies of a scanned volume");
        ClonePlan::assess(index, keep, copy, &self.denylist, &self.cloud_folders)
    }

    pub fn clone_bytes(&self, pairs: &[(NodeId, NodeId)]) -> u64 {
        let index = self.index.as_ref().expect("clones replace copies of a scanned volume");
        pairs.iter().map(|(_, copy)| index.node(*copy).total_size).sum()
    }

    pub fn replace_with_clones(
        &mut self,
        pairs: &[(NodeId, NodeId)],
        progress: Option<&SyncSender<CloneProgress>>,
        cancel: &CancelToken,
    ) -> Result<ClonesOutcome, SessionError> {
        let total = self.clone_bytes(pairs);
        let index = self.index.as_ref().expect("clones replace copies of a scanned volume");
        let mut outcome = ClonesOutcome::default();
        let mut replaced = Vec::new();
        let mut finished = 0;
        for (keep, copy) in pairs {
            if cancel.is_cancelled() {
                outcome.stopped = true;
                break;
            }
            let path = index.path(*copy);
            let compared = |offset: u64| report_clone_progress(progress, finished + offset, total);
            let result = ClonePlan::prepare(index, *keep, *copy, &self.denylist, &self.cloud_folders)
                .and_then(|plan| plan.execute(&mut self.clone_journal, cancel, &compared));
            finished += index.node(*copy).total_size;
            match result {
                Ok(CloneReplaced { freed_bytes }) => {
                    outcome.freed += freed_bytes;
                    replaced.push(path);
                }
                Err(CloneReplaceError::Stopped) => {
                    outcome.stopped = true;
                    break;
                }
                Err(error) => outcome.failures.push((path, error)),
            }
            report_clone_progress(progress, finished, total);
        }
        self.rescan_paths(&replaced)?;
        Ok(outcome)
    }

    fn refresh_screen_captures(&mut self) {
        self.enrichment.screen_captures = match query_screen_captures() {
            Ok(screen_captures) => screen_captures,
            Err(error) => {
                SystemLog::new(LOG_SUBSYSTEM, LOG_CATEGORY)
                    .error(&format!("Spotlight did not list screen captures: {error}"));
                Vec::new()
            }
        };
    }

    fn install(&mut self, index: Index) {
        let duplicate_paths = self.duplicate_paths();
        self.index = Some(index);
        self.remap(duplicate_paths, Remapping::Everything);
        self.refresh_recommendations();
    }

    fn rescan_paths(&mut self, paths: &[PathBuf]) -> Result<(), SessionError> {
        if paths.is_empty() {
            return Ok(());
        }
        let index = self.index.as_ref().expect("only a scanned volume is rescanned");
        let changed: Vec<ChangedPath> = paths
            .iter()
            .map(|path| ChangedPath {
                path: path.clone(),
                subtree: false,
            })
            .collect();
        let pending = PendingDelta {
            request: index.delta_request(&changed),
            last_event_id: self.last_event_id,
            touches_rule_sources: touches_rule_sources(index, &self.paths.home, &changed),
        };
        let listings = pending.read(&ScanOptions::default())?;
        self.apply_listings(&pending, listings).map(drop)
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
                    members: std::iter::once(group.keep)
                        .chain(group.copies.iter().map(|copy| copy.node))
                        .map(|member| (member, index.path(member), MemberStamp::of(index, member)))
                        .collect(),
                })
                .collect(),
        )
    }

    fn remap(&mut self, duplicate_paths: Option<Vec<GroupPaths>>, remapping: Remapping) {
        let index = self.index.as_ref().expect("remapping follows a new index");
        let resolve = |node: NodeId, path: &Path| match remapping {
            Remapping::Detached if index.is_live(node) => Some(node),
            Remapping::Detached | Remapping::Everything => index.find(path),
        };
        self.tray = self.tray.remapped(index, resolve);
        if let (Some(groups), Some(duplicates)) = (duplicate_paths, self.duplicates.as_mut()) {
            let mut remapped = Vec::new();
            for (group, paths) in mem::take(&mut duplicates.groups).into_iter().zip(groups) {
                let members: Vec<NodeId> = paths
                    .members
                    .iter()
                    .filter_map(|(node, path, stamp)| {
                        resolve(*node, path).filter(|member| MemberStamp::of(index, *member) == *stamp)
                    })
                    .collect();
                let unchanged = members.len() == paths.members.len()
                    && members
                        .iter()
                        .zip(&paths.members)
                        .all(|(member, (node, _, _))| member == node);
                if unchanged && remapping == Remapping::Detached {
                    remapped.push(group);
                } else if members.len() > 1
                    && let Some(regrouped) =
                        DuplicateGroup::of_members(index, paths.size, &members, &mut duplicates.skipped)
                {
                    remapped.push(regrouped);
                }
            }
            duplicates.groups = remapped;
        }
    }
}

fn rules_for(root: &Path, home: &Path) -> io::Result<RuleSet> {
    RuleSet::for_volume(
        root,
        &ConfigOverrides::read(home)?.with_screenshot_location(screen_capture_location(home)),
    )
}

fn touches_rule_sources(index: &Index, home: &Path, changed: &[ChangedPath]) -> bool {
    let sources: Vec<NodeId> = ConfigOverrides::source_folders(home)
        .into_iter()
        .chain(InstalledSimulatorRuntimes::source_folders())
        .filter_map(|folder| index.find(&folder))
        .collect();
    changed.iter().any(|change| {
        let changed_node = index.find(&change.path);
        let parent_node = change.path.parent().and_then(|parent| index.find(parent));
        let at_source_or_its_child = [changed_node, parent_node]
            .into_iter()
            .flatten()
            .any(|node| sources.contains(&node));
        let source_inside_rescanned_subtree = change.subtree
            && changed_node.is_some_and(|rescanned| sources.iter().any(|source| encloses(index, rescanned, *source)));
        at_source_or_its_child || source_inside_rescanned_subtree
    })
}

fn encloses(index: &Index, ancestor: NodeId, node: NodeId) -> bool {
    let mut current = Some(node);
    while let Some(member) = current {
        if member == ancestor {
            return true;
        }
        current = index.parent(member);
    }
    false
}

fn open_clone_journal(path: &Path) -> io::Result<CloneJournal> {
    match CloneJournal::open(path) {
        Ok(clone_journal) => Ok(clone_journal),
        Err(CloneJournalError::Corrupt(reason)) => {
            SystemLog::new(LOG_SUBSYSTEM, LOG_CATEGORY)
                .error(&format!("The clone journal is corrupt and is set aside: {reason}"));
            CloneJournal::set_aside(path)
        }
        Err(CloneJournalError::Io(error)) => Err(error),
    }
}

fn report_clone_progress(progress: Option<&SyncSender<CloneProgress>>, done: u64, total: u64) {
    if let Some(sender) = progress {
        match sender.try_send(CloneProgress { done, total }) {
            Ok(()) | Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {}
        }
    }
}
