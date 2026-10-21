use std::fs::{self, File};
use std::iter;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::mpsc::RecvTimeoutError;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use block2::RcBlock;

use domovoi_agent::{AgentLimits, FocusContext};
use domovoi_downloader::{
    Catalog, CatalogEntry, DownloadOutcome, Downloader, ModelSource, ModelStore, Sha256Hasher, is_excluded_from_backup,
};
use domovoi_index::{ModelEvent, NodeId};
use domovoi_inference::{CancelFlag, InferenceBackend};
use domovoi_inference_proto::Timeouts;
use domovoi_render::NodeOrigin;
use domovoi_scanner::{CancelToken, VolumeEvent};
use objc2::MainThreadMarker;
use objc2_foundation::NSTimer;
use serde_json::{Value, json};

use crate::access_state::AccessState;
use crate::apply_job::ApplyJob;
use crate::assistant::Assistant;
use crate::assistant_settings::AssistantSettings;
use crate::cache_opening::CacheOpening;
use crate::care_models::CareModels;
use crate::chat_model::ChatModel;
use crate::disk_overview::DiskOverview;
use crate::disk_probe::DiskProbe;
use crate::duplicate_summary::DuplicateSummary;
use crate::duplicates_model::DuplicatesModel;
use crate::event_effect::EventEffect;
use crate::event_queue::{EventQueue, HEAVY_DELTA_NODES};
use crate::expansion::{Expansion, NODE_BUDGET};
use crate::inference_client::InferenceClient;
use crate::purge_job::PurgeJob;
use crate::question::Question;
use crate::recommendations_model::RecommendationsModel;
use crate::safety_badge::SafetyBadge;
use crate::scan_job::ScanJob;
use crate::scan_stage::ScanStage;
use crate::scan_status::ScanStatus;
use crate::scenario::Scenario;
use crate::scenario_step::ScenarioStep;
use crate::screen::Screen;
use crate::service_address::ServiceAddress;
use crate::session::Session;
use crate::session_paths::SessionPaths;
use crate::shell_data::ShellData;
use crate::shell_snapshot::write_snapshots;
use crate::snapshot_options::SnapshotOptions;
use crate::tray_measure_job::TrayMeasureJob;
use crate::tray_model::TrayModel;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const EVENT_POLL: Duration = Duration::from_millis(50);
const SCAN_POLL: Duration = Duration::from_millis(50);
const SESSION_BETWEEN_STEPS: &str = "the session is home between steps";

pub fn run_end_to_end(marker: MainThreadMarker, scenario: &Path, report: &Path) -> Result<(), String> {
    let scenario = Scenario::read(scenario)?;
    let mut run = EndToEnd::open(&scenario)?;
    let mut steps = Vec::new();
    let mut failure = None;
    for step in &scenario.steps {
        match run.perform(marker, step) {
            Ok(detail) => steps.push(detail),
            Err(error) => {
                steps.push(json!({ "step": step_name(step), "error": error }));
                failure = Some(error);
                break;
            }
        }
    }
    let totals = run.session().recommendations().map(|recommendations| {
        json!({
            "safe": recommendations.totals.safe,
            "recoverable": recommendations.totals.recoverable,
            "check_yourself": recommendations.totals.check_yourself,
        })
    });
    let document = json!({ "steps": steps, "totals": totals, "snapshots": run.snapshots });
    fs::write(
        report,
        serde_json::to_string_pretty(&document).expect("the report serialises"),
    )
    .map_err(|error| format!("{}: {error}", report.display()))?;
    failure.map_or(Ok(()), Err)
}

struct EndToEnd {
    root: PathBuf,
    paths: SessionPaths,
    session: Option<Session>,
    duplicates: Option<DuplicateSummary>,
    model: Option<(CatalogEntry, PathBuf)>,
    snapshots: Vec<PathBuf>,
    disk: Option<(PathBuf, DiskProbe)>,
}

impl EndToEnd {
    fn open(scenario: &Scenario) -> Result<Self, String> {
        let paths = SessionPaths::in_directory(&scenario.state);
        Ok(Self {
            root: scenario.root.clone(),
            session: Some(Session::open(&scenario.root, &paths, APP_VERSION).map_err(|error| error.to_string())?),
            paths,
            duplicates: None,
            model: None,
            snapshots: Vec::new(),
            disk: None,
        })
    }

    fn perform(&mut self, marker: MainThreadMarker, step: &ScenarioStep) -> Result<Value, String> {
        match step {
            ScenarioStep::Scan => {
                self.session_mut()
                    .scan(None, CancelToken::default())
                    .map_err(|error| error.to_string())?;
                let index = self.session().index().expect("a finished scan installs an index");
                let root = index.node(index.root());
                Ok(json!({ "step": "scan", "files": root.files, "bytes": root.allocated, "nodes": index.len() }))
            }
            ScenarioStep::ReopenCache => {
                let mut reopened =
                    Session::open(&self.root, &self.paths, APP_VERSION).map_err(|error| error.to_string())?;
                match reopened.open_cached().map_err(|error| error.to_string())? {
                    CacheOpening::Opened => {}
                    opening => return Err(format!("the scan cache was not reused: {opening:?}")),
                }
                self.session = Some(reopened);
                Ok(json!({ "step": "reopen_cache", "last_event_id": self.session().last_event_id() }))
            }
            ScenarioStep::Rescan { path } => self.rescan(path),
            ScenarioStep::CreateFile { path, bytes } => {
                fs::write(path, vec![b'x'; *bytes]).map_err(|error| format!("{}: {error}", path.display()))?;
                Ok(json!({ "step": "create_file", "path": path, "bytes": bytes }))
            }
            ScenarioStep::AwaitChange {
                path,
                timeout_seconds,
                heavy_nodes,
            } => self.await_change(
                path,
                Duration::from_secs(*timeout_seconds),
                heavy_nodes.unwrap_or(HEAVY_DELTA_NODES),
            ),
            ScenarioStep::Recommendations => {
                let model = self.recommendations();
                let rules: Vec<String> = model.cards.iter().map(|card| card.provenance.label()).collect();
                let blocked: Vec<&str> = model.cards.iter().filter_map(|card| card.blocked.as_deref()).collect();
                Ok(json!({ "step": "recommendations", "cards": rules, "blocked": blocked, "total": model.total() }))
            }
            ScenarioStep::TrashRecommendations { level } => self.trash_recommendations(*level),
            ScenarioStep::RestoreLast => {
                let last = self
                    .session()
                    .trash_journal()
                    .entries()
                    .len()
                    .checked_sub(1)
                    .ok_or("the trash journal is empty")?;
                let restored = self.session_mut().restore(last).map_err(|error| format!("{error:?}"))?;
                Ok(json!({ "step": "restore_last", "restored": restored }))
            }
            ScenarioStep::FindDuplicates => {
                let started = Instant::now();
                self.session_mut()
                    .find_duplicates(None, CancelToken::default())
                    .map_err(|error| error.to_string())?;
                let model = self.duplicates_model(started.elapsed().as_secs())?;
                let groups: Vec<Value> = model
                    .groups()
                    .iter()
                    .map(|group| json!({ "name": group.name, "copies": group.copies.len(), "frees": group.frees() }))
                    .collect();
                Ok(json!({ "step": "find_duplicates", "groups": groups }))
            }
            ScenarioStep::ReplaceWithClone => self.replace_with_clone(),
            ScenarioStep::PurgeTrash => self.purge_trash(),
            ScenarioStep::DownloadModel {
                catalog,
                entry,
                mirror,
                models,
            } => self.download_model(catalog, entry, mirror, models),
            ScenarioStep::AskAssistant {
                question,
                max_tokens_per_step,
            } => self.ask_assistant(question, *max_tokens_per_step),
            ScenarioStep::ScrollDiskList { path } => self.scroll_disk_list(marker, path),
            ScenarioStep::UpdateDiskScreen => self.update_disk_screen(),
            ScenarioStep::Snapshots { directory, scale } => {
                let written = self.snapshots(marker, directory, *scale)?;
                self.snapshots.extend(written.iter().cloned());
                Ok(json!({ "step": "snapshots", "files": written }))
            }
        }
    }

    fn session(&self) -> &Session {
        self.session.as_ref().expect(SESSION_BETWEEN_STEPS)
    }

    fn purge_trash(&mut self) -> Result<Value, String> {
        let entries = self.session_mut().take_for_purge().map_err(|error| error.to_string())?;
        let trashed: Vec<PathBuf> = entries.iter().map(|entry| PathBuf::from(&entry.trashed)).collect();
        let job = PurgeJob::start(entries);
        while !job.is_finished() {
            thread::sleep(SCAN_POLL);
        }
        let report = job.finish();
        let failed: Vec<String> = report
            .failed
            .iter()
            .map(|(entry, error)| format!("{}: {error}", entry.trashed))
            .collect();
        let (purged, skipped) = (report.purged.len(), report.skipped.len());
        self.session_mut()
            .return_untouched(report.untouched)
            .map_err(|error| error.to_string())?;
        Ok(json!({
            "step": "purge_trash",
            "purged": purged,
            "skipped": skipped,
            "failed": failed,
            "journal_left": self.session().trash_journal().entries().len(),
            "paths_left": trashed.iter().filter(|path| path.symlink_metadata().is_ok()).count(),
        }))
    }

    fn rescan(&mut self, path: &Path) -> Result<Value, String> {
        let expected = DiskOverview::measure(&self.root)
            .map_err(|error| error.to_string())?
            .volumes
            .data_used
            .value;
        let timer = unsafe {
            NSTimer::timerWithTimeInterval_repeats_block(
                SCAN_POLL.as_secs_f64(),
                true,
                &RcBlock::new(|_: NonNull<NSTimer>| {}),
            )
        };
        let session = self.session.take().expect(SESSION_BETWEEN_STEPS);
        let mut job = ScanJob::start(session, false, expected, timer);
        while !job.is_finished() {
            job.absorb_progress();
            thread::sleep(SCAN_POLL);
        }
        job.absorb_progress();
        let files = job.status.files;
        let (session, outcome) = job.finish();
        self.session = Some(session);
        outcome.map_err(|error| error.to_string())?;
        let index = self.session().index().expect("a finished rescan installs an index");
        Ok(json!({
            "step": "rescan",
            "found": index.find(path).is_some(),
            "files": files,
            "last_event_id": self.session().last_event_id(),
        }))
    }

    fn session_mut(&mut self) -> &mut Session {
        self.session.as_mut().expect(SESSION_BETWEEN_STEPS)
    }

    fn recommendations(&self) -> RecommendationsModel {
        let index = self.session().index().expect("recommendations follow a scan");
        self.session()
            .recommendations()
            .map(|recommendations| {
                RecommendationsModel::build(index, self.session().rules(), recommendations, SystemTime::now())
            })
            .unwrap_or_default()
    }

    fn duplicates_model(&mut self, seconds: u64) -> Result<DuplicatesModel, String> {
        let session = self.session.as_ref().expect(SESSION_BETWEEN_STEPS);
        let index = session.index().expect("duplicates follow a scan");
        let duplicates = session.duplicates().expect("a finished search stores duplicates");
        let candidates = duplicates
            .groups
            .iter()
            .map(|group| group.copies.len() as u64 + 1)
            .sum::<u64>()
            + duplicates.skipped.len() as u64;
        self.duplicates = Some(DuplicateSummary { candidates, seconds });
        let mut model =
            DuplicatesModel::found(index, duplicates, candidates, seconds).map_err(|error| error.to_string())?;
        model.assess_clones(|keep, copy| session.clone_plan(keep, copy).map(drop));
        Ok(model)
    }

    fn trash_recommendations(&mut self, level: SafetyBadge) -> Result<Value, String> {
        let targets: Vec<_> = self
            .recommendations()
            .cards
            .into_iter()
            .filter(|card| card.level == level && card.selectable())
            .flat_map(|card| card.targets)
            .collect();
        for target in &targets {
            self.session_mut().add_to_tray(target.node, target.rule_id.as_deref());
        }
        let (measured, measured_files) = self.measured_tray()?;
        let moved = measured.total();
        let outcomes = self
            .session_mut()
            .move_tray_to_trash()
            .map_err(|error| error.to_string())?;
        let model = measured.with_outcome(&outcomes, self.session().trash_journal());
        let trashed: Vec<String> = self
            .session()
            .trash_journal()
            .entries()
            .iter()
            .map(|entry| entry.trashed.clone())
            .collect();
        Ok(json!({
            "step": "trash_recommendations",
            "moved": moved,
            "measured_files": measured_files,
            "estimated": model.outcome.as_ref().is_some_and(|outcome| outcome.estimated),
            "failures": model.outcome.map(|outcome| outcome.failures).unwrap_or_default(),
            "trashed": trashed,
        }))
    }

    fn measured_tray(&self) -> Result<(TrayModel, u64), String> {
        let items: Vec<PathBuf> = self
            .session()
            .tray()
            .items()
            .iter()
            .map(|item| item.physical.clone())
            .collect();
        let measured = TrayMeasureJob::start(items)
            .finish()
            .map_err(|error| error.to_string())?;
        let session = self.session();
        let index = session.index().expect("the tray belongs to a scanned volume");
        let model = TrayModel::measured(index, session.tray(), session.trash_journal(), &measured.sizes);
        Ok((model, measured.walked.files))
    }

    fn replace_with_clone(&mut self) -> Result<Value, String> {
        let model = self.duplicates_model(self.duplicates.map_or(0, |summary| summary.seconds))?;
        let (keep, copy) = model
            .groups()
            .iter()
            .flat_map(|group| group.copies.iter().map(move |copy| (group, copy)))
            .find(|(_, copy)| copy.can_check(true))
            .and_then(|(group, copy)| Some((group.keep.node?, copy.node?)))
            .ok_or("no copy can be replaced with a clone")?;
        let replaced = self
            .session_mut()
            .replace_with_clone(keep, copy)
            .map_err(|error| format!("{error:?}"))?;
        Ok(json!({ "step": "replace_with_clone", "trashed": replaced.trashed }))
    }

    fn download_model(&mut self, catalog: &Path, entry: &str, mirror: &str, models: &Path) -> Result<Value, String> {
        let catalog = Catalog::from_path(catalog).map_err(|error| error.to_string())?;
        let entry = catalog
            .entry(entry)
            .ok_or_else(|| format!("{entry} is not in the catalog"))?
            .clone();
        let store = ModelStore::open(models).map_err(|error| error.to_string())?;
        let handle = Downloader::new(ModelSource::mirror(mirror), store.clone()).start(&entry, |_| {});
        let event = match handle.wait() {
            DownloadOutcome::Installed(event) => event,
            other => return Err(format!("the download ended as {other:?}")),
        };
        self.session_mut().apply_model_event(&event);
        let ModelEvent::Downloaded { path } = event else {
            return Err("an installed model reports its path".to_owned());
        };
        let digest = Sha256Hasher::digest_of_file(&path).map_err(|error| error.to_string())?;
        let excluded = is_excluded_from_backup(store.directory()).map_err(|error| error.to_string())?;
        let report = json!({
            "step": "download_model",
            "path": path,
            "sha256_matches": digest == entry.sha256,
            "excluded_from_backup": excluded,
        });
        self.model = Some((entry, path));
        Ok(report)
    }

    fn ask_assistant(&mut self, question: &str, max_tokens_per_step: Option<u32>) -> Result<Value, String> {
        let (entry, weights) = self.model.clone().ok_or("a question needs a downloaded model")?;
        let timeouts = Timeouts::from_environment().map_err(|error| error.to_string())?;
        let mut client =
            InferenceClient::connect(ServiceAddress::bundled(), timeouts).map_err(|error| error.to_string())?;
        let file = File::open(&weights).map_err(|error| format!("{}: {error}", weights.display()))?;
        client
            .load(file.into(), entry.class, entry.context_length, &CancelFlag::default())
            .map_err(|error| error.to_string())?;
        let defaults = AgentLimits::for_context_length(entry.context_length);
        let limits = AgentLimits {
            max_tokens_per_step: max_tokens_per_step.unwrap_or(defaults.max_tokens_per_step),
            ..defaults
        };
        let mut chat = ChatModel::default();
        let asked = Question {
            text: question.to_owned(),
            history: chat.past_turns(),
            focus: FocusContext::default(),
        };
        chat.ask(question);
        let answer = self
            .session()
            .ask(
                &mut client,
                &entry,
                limits,
                &asked,
                &CancelFlag::default(),
                &mut |step| {
                    chat.record_step(step);
                },
            )
            .map_err(|error| error.to_string())?;
        chat.record_answer(&answer);
        let tools: Vec<&str> = answer.steps.iter().map(|step| step.tool.name()).collect();
        Ok(json!({
            "step": "ask_assistant",
            "service_pid": client.service_pid(),
            "tools": tools,
            "answer_characters": answer.text.chars().count(),
            "hypotheses": answer.hypotheses.len(),
            "actions": answer.actions.len(),
            "chat_entries": chat.entries.len(),
        }))
    }

    fn await_change(&mut self, path: &Path, timeout: Duration, heavy_nodes: usize) -> Result<Value, String> {
        let (_stream, events) = self.session().watch().map_err(|error| error.to_string())?;
        let mut queue = EventQueue::new(heavy_nodes);
        let mut applied_in_background = 0_u64;
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(index) = self.session().index()
                && let Some(node) = index.find(path)
            {
                return Ok(json!({
                    "step": "await_change",
                    "path": path,
                    "allocated": index.node(node).allocated,
                    "descended": queue.descended(),
                    "applied_in_background": applied_in_background,
                }));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(format!("{} did not reach the index in {timeout:?}", path.display()));
            }
            let arrived: Vec<VolumeEvent> = match events.recv_timeout(remaining.min(EVENT_POLL)) {
                Ok(event) => iter::once(event).chain(events.try_iter()).collect(),
                Err(RecvTimeoutError::Timeout) => Vec::new(),
                Err(RecvTimeoutError::Disconnected) => return Err("the volume event stream stopped".to_owned()),
            };
            match queue
                .advance(arrived, self.session_mut())
                .map_err(|error| error.to_string())?
            {
                EventEffect::Applied | EventEffect::Ignored => {}
                EventEffect::Heavy => {
                    let batch = queue.take_heavy().expect("a heavy batch waits in the queue");
                    let session = self.session.take().expect(SESSION_BETWEEN_STEPS);
                    let (session, applied) = ApplyJob::start(session, batch).finish();
                    self.session = Some(session);
                    queue.settle(applied).map_err(|error| error.to_string())?;
                    applied_in_background += 1;
                }
                EventEffect::RescanRequired => return Err("the volume asked for a full rescan".to_owned()),
            }
        }
    }

    fn snapshots(&mut self, marker: MainThreadMarker, directory: &Path, scale: f64) -> Result<Vec<PathBuf>, String> {
        let root = self.session().index().expect("snapshots follow a scan").root();
        let data = self.live_data(root)?;
        let mut written = Vec::new();
        for dark in [true, false] {
            let options = SnapshotOptions {
                screen: Screen::Access,
                dark,
                scale,
                high_contrast: false,
                directory: directory.to_path_buf(),
            };
            written.extend(write_snapshots(marker, &data, &Screen::primary(), &options)?);
        }
        Ok(written)
    }

    fn scroll_disk_list(&mut self, marker: MainThreadMarker, path: &Path) -> Result<Value, String> {
        let (data, focus) = self.live_disk(path)?;
        let probe = DiskProbe::scrolled_to_the_end(marker, &data, focus);
        let report = json!({ "step": "scroll_disk_list", "rows": probe.rows(), "offset": probe.offset() });
        self.disk = Some((path.to_path_buf(), probe));
        Ok(report)
    }

    fn update_disk_screen(&mut self) -> Result<Value, String> {
        let (path, mut probe) = self
            .disk
            .take()
            .ok_or("scroll_disk_list shows the disk screen before it updates")?;
        let (data, focus) = self.live_disk(&path)?;
        probe.update(&data, focus);
        let report = json!({
            "step": "update_disk_screen",
            "same_scroll": probe.keeps_its_scroll(),
            "same_table": probe.keeps_its_table(),
            "rows": probe.rows(),
            "offset": probe.offset(),
        });
        self.disk = Some((path, probe));
        Ok(report)
    }

    fn live_disk(&mut self, path: &Path) -> Result<(ShellData, usize), String> {
        let node = self
            .session()
            .index()
            .expect("the disk screen follows a scan")
            .find(path)
            .ok_or_else(|| format!("{} is not in the index", path.display()))?;
        let data = self.live_data(node)?;
        let focus = data
            .tree_position(NodeOrigin::Indexed(node))
            .ok_or_else(|| format!("{} is not in the diagram", path.display()))?;
        Ok((data, focus))
    }

    fn live_data(&mut self, focus: NodeId) -> Result<ShellData, String> {
        let overview = DiskOverview::measure(&self.root).map_err(|error| error.to_string())?;
        let recommendations = self.recommendations();
        let duplicates = match self.duplicates {
            Some(summary) => self.duplicates_model(summary.seconds)?,
            None => DuplicatesModel::idle(),
        };
        let index = self.session().index().expect("live data follows a scan");
        let (tray, _) = self.measured_tray()?;
        let expansion = Expansion::around(index, focus, NODE_BUDGET);
        let root = index.node(index.root());
        let scan = ScanStatus {
            bytes: root.allocated,
            files: u64::from(root.files),
            sweep: 1.0,
            threads: self.session().workers(),
            current: self.root.display().to_string(),
            full_first_scan: false,
            stage: ScanStage::Walking,
        };
        let care = CareModels {
            recommendations,
            duplicates,
            tray,
        };
        let models = ModelStore::open(&self.paths.models).map_err(|error| error.to_string())?;
        let mut assistant = Assistant::new(models, Catalog::bundled(), AssistantSettings::default());
        assistant.library.finish_scan()?;
        Ok(ShellData {
            rescan_allowed: true,
            ..ShellData::measured(
                AccessState::probe(),
                &overview,
                Some(index),
                &expansion,
                scan,
                care,
                assistant.models(),
            )
        })
    }
}

fn step_name(step: &ScenarioStep) -> &'static str {
    match step {
        ScenarioStep::Scan => "scan",
        ScenarioStep::ReopenCache => "reopen_cache",
        ScenarioStep::Rescan { .. } => "rescan",
        ScenarioStep::CreateFile { .. } => "create_file",
        ScenarioStep::AwaitChange { .. } => "await_change",
        ScenarioStep::Recommendations => "recommendations",
        ScenarioStep::TrashRecommendations { .. } => "trash_recommendations",
        ScenarioStep::RestoreLast => "restore_last",
        ScenarioStep::FindDuplicates => "find_duplicates",
        ScenarioStep::ReplaceWithClone => "replace_with_clone",
        ScenarioStep::PurgeTrash => "purge_trash",
        ScenarioStep::DownloadModel { .. } => "download_model",
        ScenarioStep::AskAssistant { .. } => "ask_assistant",
        ScenarioStep::ScrollDiskList { .. } => "scroll_disk_list",
        ScenarioStep::UpdateDiskScreen => "update_disk_screen",
        ScenarioStep::Snapshots { .. } => "snapshots",
    }
}
