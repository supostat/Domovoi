use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant, SystemTime};

use domovoi_agent::AgentLimits;
use domovoi_downloader::{
    Catalog, CatalogEntry, DownloadOutcome, Downloader, ModelSource, ModelStore, Sha256Hasher, is_excluded_from_backup,
};
use domovoi_index::ModelEvent;
use domovoi_inference::{CancelFlag, InferenceBackend};
use domovoi_inference_proto::Timeouts;
use domovoi_scanner::CancelToken;
use objc2::MainThreadMarker;
use serde_json::{Value, json};

use crate::access_state::AccessState;
use crate::assistant::Assistant;
use crate::assistant_settings::AssistantSettings;
use crate::cache_opening::CacheOpening;
use crate::care_models::CareModels;
use crate::chat_model::ChatModel;
use crate::disk_overview::DiskOverview;
use crate::duplicate_summary::DuplicateSummary;
use crate::duplicates_model::DuplicatesModel;
use crate::event_effect::EventEffect;
use crate::expansion::{Expansion, NODE_BUDGET};
use crate::inference_client::InferenceClient;
use crate::recommendations_model::RecommendationsModel;
use crate::safety_badge::SafetyBadge;
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
use crate::tray_model::TrayModel;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

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
    let totals = run.session.recommendations().map(|recommendations| {
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
    session: Session,
    duplicates: Option<DuplicateSummary>,
    model: Option<(CatalogEntry, PathBuf)>,
    snapshots: Vec<PathBuf>,
}

impl EndToEnd {
    fn open(scenario: &Scenario) -> Result<Self, String> {
        let paths = SessionPaths::in_directory(&scenario.state);
        Ok(Self {
            root: scenario.root.clone(),
            session: Session::open(&scenario.root, &paths, APP_VERSION).map_err(|error| error.to_string())?,
            paths,
            duplicates: None,
            model: None,
            snapshots: Vec::new(),
        })
    }

    fn perform(&mut self, marker: MainThreadMarker, step: &ScenarioStep) -> Result<Value, String> {
        match step {
            ScenarioStep::Scan => {
                self.session
                    .scan(None, CancelToken::default())
                    .map_err(|error| error.to_string())?;
                let index = self.session.index().expect("a finished scan installs an index");
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
                self.session = reopened;
                Ok(json!({ "step": "reopen_cache", "last_event_id": self.session.last_event_id() }))
            }
            ScenarioStep::CreateFile { path, bytes } => {
                fs::write(path, vec![b'x'; *bytes]).map_err(|error| format!("{}: {error}", path.display()))?;
                Ok(json!({ "step": "create_file", "path": path, "bytes": bytes }))
            }
            ScenarioStep::AwaitChange { path, timeout_seconds } => {
                let allocated = self.await_change(path, Duration::from_secs(*timeout_seconds))?;
                Ok(json!({ "step": "await_change", "path": path, "allocated": allocated }))
            }
            ScenarioStep::Recommendations => {
                let model = self.recommendations();
                let rules: Vec<String> = model.cards.iter().map(|card| card.provenance.label()).collect();
                let blocked: Vec<&str> = model.cards.iter().filter_map(|card| card.blocked.as_deref()).collect();
                Ok(json!({ "step": "recommendations", "cards": rules, "blocked": blocked, "total": model.total() }))
            }
            ScenarioStep::TrashRecommendations { level } => self.trash_recommendations(*level),
            ScenarioStep::RestoreLast => {
                let last = self
                    .session
                    .trash_journal()
                    .entries()
                    .len()
                    .checked_sub(1)
                    .ok_or("the trash journal is empty")?;
                let restored = self.session.restore(last).map_err(|error| format!("{error:?}"))?;
                Ok(json!({ "step": "restore_last", "restored": restored }))
            }
            ScenarioStep::FindDuplicates => {
                let started = Instant::now();
                self.session
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
            ScenarioStep::Snapshots { directory, scale } => {
                let written = self.snapshots(marker, directory, *scale)?;
                self.snapshots.extend(written.iter().cloned());
                Ok(json!({ "step": "snapshots", "files": written }))
            }
        }
    }

    fn recommendations(&self) -> RecommendationsModel {
        let index = self.session.index().expect("recommendations follow a scan");
        self.session
            .recommendations()
            .map(|recommendations| {
                RecommendationsModel::build(index, self.session.rules(), recommendations, SystemTime::now())
            })
            .unwrap_or_default()
    }

    fn duplicates_model(&mut self, seconds: u64) -> Result<DuplicatesModel, String> {
        let index = self.session.index().expect("duplicates follow a scan");
        let duplicates = self.session.duplicates().expect("a finished search stores duplicates");
        let candidates = duplicates
            .groups
            .iter()
            .map(|group| group.copies.len() as u64 + 1)
            .sum::<u64>()
            + duplicates.skipped.len() as u64;
        self.duplicates = Some(DuplicateSummary { candidates, seconds });
        let mut model =
            DuplicatesModel::found(index, duplicates, candidates, seconds).map_err(|error| error.to_string())?;
        model.assess_clones(|keep, copy| self.session.clone_plan(keep, copy).map(drop));
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
            self.session.add_to_tray(target.node, target.rule_id.as_deref());
        }
        let moved = self
            .session
            .tray_reclaimable()
            .map_err(|error| error.to_string())?
            .bytes;
        let outcomes = self.session.move_tray_to_trash().map_err(|error| error.to_string())?;
        let model = TrayModel::default().with_outcome(&outcomes, moved, self.session.trash_journal());
        let trashed: Vec<String> = self
            .session
            .trash_journal()
            .entries()
            .iter()
            .map(|entry| entry.trashed.clone())
            .collect();
        Ok(json!({
            "step": "trash_recommendations",
            "moved": moved,
            "failures": model.outcome.map(|outcome| outcome.failures).unwrap_or_default(),
            "trashed": trashed,
        }))
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
            .session
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
        self.session
            .apply_model_event(&event)
            .map_err(|error| error.to_string())?;
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
        chat.ask(question);
        let answer = self
            .session
            .ask(
                &mut client,
                &entry,
                limits,
                question,
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

    fn await_change(&mut self, path: &Path, timeout: Duration) -> Result<u64, String> {
        let (_stream, events) = self.session.watch().map_err(|error| error.to_string())?;
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(index) = self.session.index()
                && let Some(node) = index.find(path)
            {
                return Ok(index.node(node).allocated);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            let event = match events.recv_timeout(remaining) {
                Ok(event) => event,
                Err(RecvTimeoutError::Timeout) => {
                    return Err(format!("{} did not reach the index in {timeout:?}", path.display()));
                }
                Err(RecvTimeoutError::Disconnected) => return Err("the volume event stream stopped".to_owned()),
            };
            let mut batch = vec![event];
            batch.extend(events.try_iter());
            match self.session.apply_events(&batch).map_err(|error| error.to_string())? {
                EventEffect::Applied | EventEffect::Ignored => {}
                EventEffect::RescanRequired => return Err("the volume asked for a full rescan".to_owned()),
            }
        }
    }

    fn snapshots(&mut self, marker: MainThreadMarker, directory: &Path, scale: f64) -> Result<Vec<PathBuf>, String> {
        let overview = DiskOverview::measure(&self.root).map_err(|error| error.to_string())?;
        let recommendations = self.recommendations();
        let duplicates = match self.duplicates {
            Some(summary) => self.duplicates_model(summary.seconds)?,
            None => DuplicatesModel::idle(),
        };
        let index = self.session.index().expect("snapshots follow a scan");
        let tray = TrayModel::build(index, self.session.tray(), self.session.trash_journal())
            .map_err(|error| error.to_string())?;
        let expansion = Expansion::around(index, index.root(), NODE_BUDGET);
        let root = index.node(index.root());
        let scan = ScanStatus {
            bytes: root.allocated,
            files: u64::from(root.files),
            sweep: 1.0,
            threads: self.session.workers(),
            current: self.root.display().to_string(),
            full_first_scan: false,
        };
        let care = CareModels {
            recommendations,
            duplicates,
            tray,
        };
        let models = ModelStore::open(&self.paths.models).map_err(|error| error.to_string())?;
        let mut assistant = Assistant::new(models, Catalog::bundled(), AssistantSettings::default());
        assistant.library.finish_scan()?;
        let data = ShellData::measured(
            AccessState::probe(),
            &overview,
            Some(index),
            &expansion,
            scan,
            care,
            assistant.models(),
        );
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
}

fn step_name(step: &ScenarioStep) -> &'static str {
    match step {
        ScenarioStep::Scan => "scan",
        ScenarioStep::ReopenCache => "reopen_cache",
        ScenarioStep::CreateFile { .. } => "create_file",
        ScenarioStep::AwaitChange { .. } => "await_change",
        ScenarioStep::Recommendations => "recommendations",
        ScenarioStep::TrashRecommendations { .. } => "trash_recommendations",
        ScenarioStep::RestoreLast => "restore_last",
        ScenarioStep::FindDuplicates => "find_duplicates",
        ScenarioStep::ReplaceWithClone => "replace_with_clone",
        ScenarioStep::DownloadModel { .. } => "download_model",
        ScenarioStep::AskAssistant { .. } => "ask_assistant",
        ScenarioStep::Snapshots { .. } => "snapshots",
    }
}
