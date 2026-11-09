use std::fs::{self, File};
use std::iter;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::mpsc::RecvTimeoutError;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use block2::RcBlock;

use domovoi_agent::{AgentLimits, FocusContext};
use domovoi_apfs_sys::SnapshotClass;
use domovoi_downloader::{
    Catalog, CatalogEntry, DownloadOutcome, Downloader, ModelSource, ModelStore, Sha256Hasher, is_excluded_from_backup,
};
use domovoi_index::{ModelEvent, NodeId, Reclaimable, SpaceRecovery};
use domovoi_inference::{CancelFlag, InferenceBackend};
use domovoi_inference_proto::Timeouts;
use domovoi_render::{DiagramLayout, DiagramMode, NodeOrigin, SunburstLayout};
use domovoi_scanner::{CancelToken, VolumeEvent};
use domovoi_theme::Palette;
use objc2::MainThreadMarker;
use objc2_foundation::NSTimer;
use serde_json::{Value, json};

use crate::access_state::AccessState;
use crate::apply_job::ApplyJob;
use crate::assistant::Assistant;
use crate::assistant_settings::AssistantSettings;
use crate::cache_opening::CacheOpening;
use crate::card_question::card_focus;
use crate::care_models::CareModels;
use crate::chat_model::ChatModel;
use crate::clone_job::CloneJob;
use crate::csv_export::csv_lines;
use crate::delete_job::DeleteJob;
use crate::delete_outcome::DeleteOutcome;
use crate::delete_status::DeleteStatus;
use crate::disk_overview::DiskOverview;
use crate::disk_probe::DiskProbe;
use crate::drop_kind::DropKind;
use crate::duplicate_summary::DuplicateSummary;
use crate::duplicates_model::DuplicatesModel;
use crate::event_effect::EventEffect;
use crate::event_queue::{EventQueue, HEAVY_DELTA_NODES};
use crate::expansion::{Expansion, NODE_BUDGET};
use crate::inference_client::InferenceClient;
use crate::main_screen::MainScreen;
use crate::preview_targets::PreviewTargets;
use crate::provenance::Provenance;
use crate::purge_job::{PurgeJob, available_capacity};
use crate::question::Question;
use crate::recommendations_model::RecommendationsModel;
use crate::safety_badge::SafetyBadge;
use crate::scan_job::ScanJob;
use crate::scan_notice::scan_finished_line;
use crate::scan_stage::ScanStage;
use crate::scan_status::ScanStatus;
use crate::scenario::Scenario;
use crate::scenario_step::ScenarioStep;
use crate::screen::Screen;
use crate::service_address::ServiceAddress;
use crate::session::Session;
use crate::session_paths::SessionPaths;
use crate::shell_data::ShellData;
use crate::shell_palette::ShellPalette;
use crate::shell_snapshot::write_snapshots;
use crate::snapshot_options::SnapshotOptions;
use crate::strings::CARD_QUESTION;
use crate::tmutil_commands::tmutil_commands;
use crate::tray_measure_job::TrayMeasureJob;
use crate::tray_model::TrayModel;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const EVENT_POLL: Duration = Duration::from_millis(50);
const SCAN_POLL: Duration = Duration::from_millis(50);
const SESSION_BETWEEN_STEPS: &str = "the session is home between steps";
const DIAGRAM_SIDE: f32 = 440.0;
const CSV_LINE_END: &str = "\r\n";

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
    chat: ChatModel,
    scan_elapsed: Option<Duration>,
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
            chat: ChatModel::default(),
            scan_elapsed: None,
        })
    }

    fn perform(&mut self, marker: MainThreadMarker, step: &ScenarioStep) -> Result<Value, String> {
        match step {
            ScenarioStep::Scan => self.scan(),
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
            ScenarioStep::Recommendations => Ok(self.recommendations_report()),
            ScenarioStep::DeleteRecommendations { level } => self.delete_recommendations(*level),
            ScenarioStep::RemoveApplication { path } => self.remove_application(path),
            ScenarioStep::MarkFromList { paths } => self.mark_from_list(paths),
            ScenarioStep::UnmarkTrayRow { row } => self.unmark_tray_row(*row),
            ScenarioStep::FindDuplicates => {
                let started = Instant::now();
                self.session_mut()
                    .find_duplicates(None, CancelToken::default())
                    .map_err(|error| error.to_string())?;
                let model = self.duplicates_model(started.elapsed().as_secs());
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
            } => self.ask_assistant(question, FocusContext::default(), *max_tokens_per_step),
            ScenarioStep::AskAboutCard {
                card,
                max_tokens_per_step,
            } => self.ask_about_card(card, *max_tokens_per_step),
            ScenarioStep::ScrollDiskList { path } => self.scroll_disk_list(marker, path),
            ScenarioStep::UpdateDiskScreen => self.update_disk_screen(),
            ScenarioStep::Snapshots { directory, scale } => {
                let written = self.snapshots(marker, directory, *scale)?;
                self.snapshots.extend(written.iter().cloned());
                Ok(json!({ "step": "snapshots", "files": written }))
            }
            ScenarioStep::ScanNotice => {
                let elapsed = self.scan_elapsed.ok_or("a scan notice follows a scan")?;
                Ok(json!({
                    "step": "scan_notice",
                    "line": scan_finished_line(elapsed),
                    "seconds": elapsed.as_secs(),
                }))
            }
            ScenarioStep::SmallGroups { path } => self.small_groups(path),
            ScenarioStep::PreviewTargets { path } => self.preview_targets(path),
            ScenarioStep::ExportCsv { path } => self.export_csv(path),
            ScenarioStep::HiddenSpace => self.hidden_space(),
            ScenarioStep::TmutilCommands => self.tmutil_commands(),
            ScenarioStep::PurgeSpace { timeout_seconds } => self.purge_space(Duration::from_secs(*timeout_seconds)),
        }
    }

    fn scan(&mut self) -> Result<Value, String> {
        let started = Instant::now();
        self.session_mut()
            .scan(None, CancelToken::default())
            .map_err(|error| error.to_string())?;
        self.scan_elapsed = Some(started.elapsed());
        let index = self.session().index().expect("a finished scan installs an index");
        let root = index.node(index.root());
        Ok(json!({ "step": "scan", "files": root.files, "bytes": root.allocated, "nodes": index.len() }))
    }

    fn hidden_space(&self) -> Result<Value, String> {
        let overview = DiskOverview::measure(&self.root).map_err(|error| error.to_string())?;
        let index = self.session().index().expect("the hidden space follows a scan");
        let hidden = overview.hidden(index);
        Ok(json!({
            "step": "hidden_space",
            "total": hidden.total.value,
            "purgeable": hidden.purgeable.value,
            "unaccounted": hidden.unaccounted,
            "discrepancy": hidden.is_discrepancy(),
            "hidden_folders": hidden.hidden_folders,
            "other_user_homes": hidden.other_user_homes,
            "time_machine": {
                "auto_backup": hidden.time_machine.auto_backup,
                "mobile_backups": hidden.time_machine.mobile_backups,
            },
            "snapshots": hidden.snapshots.len(),
        }))
    }

    fn tmutil_commands(&self) -> Result<Value, String> {
        let overview = DiskOverview::measure(&self.root).map_err(|error| error.to_string())?;
        let snapshots = &overview.volumes.snapshots;
        let time_machine = snapshots
            .iter()
            .filter(|snapshot| snapshot.class == SnapshotClass::TimeMachine)
            .count();
        let commands = tmutil_commands(snapshots);
        Ok(json!({
            "step": "tmutil_commands",
            "lines": commands.lines().count(),
            "time_machine": time_machine,
            "other": snapshots.len() - time_machine,
        }))
    }

    fn purge_space(&self, timeout: Duration) -> Result<Value, String> {
        let recovery = SpaceRecovery::load().ok_or("this macOS offers no disk-space recovery functions")?;
        let job = PurgeJob::start(recovery, self.root.clone(), available_capacity, timeout);
        while !job.is_finished() {
            thread::sleep(SCAN_POLL);
        }
        let progress = job.progress();
        let (report, error) = match job.finish() {
            Ok(report) => (Some(report), None),
            Err(error) => (None, Some(error.to_string())),
        };
        Ok(json!({
            "step": "purge_space",
            "estimate": report.map_or(progress.estimate, |report| report.estimate),
            "freed": report.map(|report| report.freed),
            "available_before": report.map(|report| report.available_before),
            "available_after": report.map(|report| report.available_after),
            "finish": report.map(|report| report.finish),
            "error": error,
        }))
    }

    fn session(&self) -> &Session {
        self.session.as_ref().expect(SESSION_BETWEEN_STEPS)
    }

    fn rescan(&mut self, path: &Path) -> Result<Value, String> {
        let timer = unsafe {
            NSTimer::timerWithTimeInterval_repeats_block(
                SCAN_POLL.as_secs_f64(),
                true,
                &RcBlock::new(|_: NonNull<NSTimer>| {}),
            )
        };
        let expected = DiskOverview::measure(&self.root)
            .map_err(|error| error.to_string())?
            .volumes
            .data_used
            .value;
        let session = self.session.take().expect(SESSION_BETWEEN_STEPS);
        let mut job = ScanJob::start(session, false, expected, timer);
        while !job.is_finished() {
            job.absorb_progress();
            thread::sleep(SCAN_POLL);
        }
        job.absorb_progress();
        let files = job.status.files;
        let sweep = job.status.sweep;
        self.scan_elapsed = Some(job.started.elapsed());
        let (session, outcome) = job.finish();
        self.session = Some(session);
        outcome.map_err(|error| error.to_string())?;
        let index = self.session().index().expect("a finished rescan installs an index");
        Ok(json!({
            "step": "rescan",
            "found": index.find(path).is_some(),
            "files": files,
            "sweep": sweep,
            "last_event_id": self.session().last_event_id(),
        }))
    }

    fn session_mut(&mut self) -> &mut Session {
        self.session.as_mut().expect(SESSION_BETWEEN_STEPS)
    }

    fn recommendations_report(&self) -> Value {
        let model = self.recommendations();
        let rules: Vec<String> = model.cards.iter().map(|card| card.provenance.label()).collect();
        let paths: Vec<&[String]> = model.cards.iter().map(|card| card.paths.as_slice()).collect();
        let blocked: Vec<&str> = model.cards.iter().filter_map(|card| card.blocked.as_deref()).collect();
        let projects: Vec<Value> = model
            .cards
            .iter()
            .flat_map(|card| &card.rows)
            .map(|row| {
                json!({
                    "name": row.name,
                    "path": row.path,
                    "artifacts": row.artifacts,
                    "age": row.age,
                    "stale": row.stale,
                    "selected": row.selected,
                    "amount": row.amount,
                })
            })
            .collect();
        json!({
            "step": "recommendations",
            "cards": rules,
            "card_paths": paths,
            "blocked": blocked,
            "projects": projects,
            "total": model.total()
        })
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

    fn duplicates_model(&mut self, seconds: u64) -> DuplicatesModel {
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
        let mut model = DuplicatesModel::found(index, duplicates, candidates, seconds);
        model.assess_clones(|keep, copy| session.clone_assessment(keep, copy));
        model
    }

    fn delete_recommendations(&mut self, level: SafetyBadge) -> Result<Value, String> {
        let targets: Vec<_> = self
            .recommendations()
            .cards
            .into_iter()
            .filter(|card| card.level == level && card.selectable())
            .flat_map(|card| card.targets)
            .collect();
        for target in &targets {
            self.session_mut().add_target_to_tray(target);
        }
        let (measured, measured_files, sizes) = self.measured_tray()?;
        let expected = measured.total();
        let free_before = DiskOverview::measure(&self.root)
            .map_err(|error| error.to_string())?
            .free_bytes();
        let (subjects, refusals) = self
            .session_mut()
            .take_tray_for_delete(&sizes)
            .map_err(|error| error.to_string())?;
        let taken: Vec<PathBuf> = subjects.iter().map(|subject| subject.physical.clone()).collect();
        let job = DeleteJob::start(subjects);
        while !job.is_finished() {
            thread::sleep(SCAN_POLL);
        }
        let last = DeleteStatus::from(job.progress());
        let sheet = measured.with_deleting(last);
        let report = job.finish();
        let outcome = DeleteOutcome::from_report(&report, &refusals);
        self.session_mut()
            .settle_delete(&report)
            .map_err(|error| error.to_string())?;
        let free_after = DiskOverview::measure(&self.root)
            .map_err(|error| error.to_string())?
            .free_bytes();
        Ok(json!({
            "step": "delete_recommendations",
            "expected": expected,
            "measured_files": measured_files,
            "files_done": last.files_done,
            "files_total": last.files_total,
            "deleting_line": sheet.deleting_line(),
            "deleting_share": sheet.deleting_share(),
            "deleted": outcome.objects,
            "freed": outcome.freed,
            "failures": outcome.failures,
            "paths_left": taken.iter().filter(|path| path.symlink_metadata().is_ok()).count(),
            "free_before": free_before,
            "free_after": free_after,
        }))
    }

    fn remove_application(&mut self, bundle: &Path) -> Result<Value, String> {
        let node = self
            .session()
            .index()
            .and_then(|index| index.find(bundle))
            .ok_or_else(|| format!("{} is not indexed", bundle.display()))?;
        if !self.session_mut().add_application_to_tray(node) {
            return Err(format!("{} is not an application bundle", bundle.display()));
        }
        let name_only: Vec<PathBuf> = self
            .session()
            .tray()
            .applications()
            .iter()
            .flat_map(|application| application.name_only.clone())
            .collect();
        let (_, _, sizes) = self.measured_tray()?;
        let (subjects, refusals) = self
            .session_mut()
            .take_tray_for_delete(&sizes)
            .map_err(|error| error.to_string())?;
        let taken: Vec<PathBuf> = subjects.iter().map(|subject| subject.physical.clone()).collect();
        let job = DeleteJob::start(subjects);
        while !job.is_finished() {
            thread::sleep(SCAN_POLL);
        }
        let report = job.finish();
        self.session_mut()
            .settle_delete(&report)
            .map_err(|error| error.to_string())?;
        Ok(json!({
            "step": "remove_application",
            "deleted": taken,
            "refused": refusals.len(),
            "failures": report.failed.len(),
            "name_only": name_only,
            "paths_left": taken.iter().filter(|path| path.symlink_metadata().is_ok()).count(),
        }))
    }

    fn mark_from_list(&mut self, paths: &[PathBuf]) -> Result<Value, String> {
        let level = paths
            .first()
            .and_then(|path| path.parent())
            .ok_or("mark_from_list drops at least one folder with a parent")?
            .to_path_buf();
        let hover = self.session().judge_drop(paths).hover();
        let verdict = self.session_mut().drop_into_tray(paths);
        let (marked_rows, collapsed_segments) = self.marks_on_level(&level)?;
        let marked: Vec<&Path> = self
            .session()
            .tray()
            .items()
            .iter()
            .map(|item| item.path.as_path())
            .collect();
        Ok(json!({
            "step": "mark_from_list",
            "hover": hover.title,
            "hover_kind": drop_kind_name(hover.kind),
            "added": verdict.allowed.len(),
            "refused": verdict.refused.len(),
            "refusal_line": verdict.refusal_line(),
            "summary": self.session().tray_summary().title(),
            "tray": marked,
            "marked_rows": marked_rows,
            "collapsed_segments": collapsed_segments,
        }))
    }

    fn marks_on_level(&mut self, level: &Path) -> Result<(Vec<String>, usize), String> {
        let (data, focus) = self.live_disk(level)?;
        let marked_rows = data
            .level_rows(focus)
            .into_iter()
            .filter(|row| row.marked)
            .map(|row| row.name)
            .collect();
        let palette = ShellPalette {
            tokens: &Palette::DARK,
            high_contrast: false,
        };
        let diagram = MainScreen::diagram_state(&data, focus, DiagramMode::Sunburst, &palette);
        let DiagramLayout::Sunburst(layout) = &diagram.layout else {
            unreachable!("the main diagram is built as a sunburst");
        };
        Ok((marked_rows, layout.collapsed().count()))
    }

    fn unmark_tray_row(&mut self, row: usize) -> Result<Value, String> {
        let (before, _, _) = self.measured_tray()?;
        let nodes = before
            .row_nodes
            .get(row)
            .cloned()
            .ok_or_else(|| format!("the tray has no row {row}"))?;
        let index = self.session().index().expect("a tray row follows a scan");
        let level = nodes
            .first()
            .and_then(|node| index.parent(*node))
            .map(|parent| index.path(parent))
            .ok_or_else(|| format!("tray row {row} has no folder above it"))?;
        for node in nodes {
            self.session_mut().remove_from_tray(node);
        }
        let (after, _, _) = self.measured_tray()?;
        let (marked_rows, collapsed_segments) = self.marks_on_level(&level)?;
        Ok(json!({
            "step": "unmark_tray_row",
            "rows_before": before.rows.len(),
            "rows_after": after.rows.len(),
            "summary": self.session().tray_summary().title(),
            "marked_rows": marked_rows,
            "collapsed_segments": collapsed_segments,
        }))
    }

    fn measured_tray(&self) -> Result<(TrayModel, u64, Vec<Reclaimable>), String> {
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
        let model = TrayModel::measured(index, session.tray(), &measured.sizes);
        Ok((model, measured.walked.files, measured.sizes))
    }

    fn replace_with_clone(&mut self) -> Result<Value, String> {
        let model = self.duplicates_model(self.duplicates.map_or(0, |summary| summary.seconds));
        let (keep, copy) = model
            .groups()
            .iter()
            .flat_map(|group| group.copies.iter().map(move |copy| (group, copy)))
            .find(|(_, copy)| copy.can_check(true))
            .and_then(|(group, copy)| Some((group.keep.node?, copy.node?)))
            .ok_or("no copy can be replaced with a clone")?;
        let session = self.session.take().expect(SESSION_BETWEEN_STEPS);
        let (session, outcome) = CloneJob::start(session, vec![(keep, copy)]).finish();
        self.session = Some(session);
        let outcome = outcome.map_err(|error| error.to_string())?;
        if let Some((path, error)) = outcome.failures.first() {
            return Err(format!("{} was not replaced: {error:?}", path.display()));
        }
        Ok(json!({ "step": "replace_with_clone", "freed": outcome.freed }))
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

    fn ask_about_card(&mut self, provenance: &Provenance, max_tokens_per_step: Option<u32>) -> Result<Value, String> {
        let card = self
            .recommendations()
            .cards
            .into_iter()
            .find(|card| &card.provenance == provenance)
            .ok_or_else(|| format!("no card of {}", provenance.label()))?;
        let focus = FocusContext {
            card: Some(card_focus(&card)),
            ..FocusContext::default()
        };
        let mut report = self.ask_assistant(CARD_QUESTION, focus, max_tokens_per_step)?;
        report["step"] = "ask_about_card".into();
        report["question"] = CARD_QUESTION.into();
        report["card"] = provenance.label().into();
        Ok(report)
    }

    fn ask_assistant(
        &mut self,
        question: &str,
        focus: FocusContext,
        max_tokens_per_step: Option<u32>,
    ) -> Result<Value, String> {
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
        let asked = Question {
            text: question.to_owned(),
            history: self.chat.past_turns(),
            focus,
        };
        self.chat.ask(question);
        let session = self.session.as_ref().expect(SESSION_BETWEEN_STEPS);
        let chat = &mut self.chat;
        let answer = session
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
        self.chat.record_answer(&answer);
        let tools: Vec<&str> = answer.steps.iter().map(|step| step.tool.name()).collect();
        Ok(json!({
            "step": "ask_assistant",
            "service_pid": client.service_pid(),
            "tools": tools,
            "answer_characters": answer.text.chars().count(),
            "hypotheses": answer.hypotheses.len(),
            "actions": answer.actions.len(),
            "history_turns": asked.history.len(),
            "chat_entries": self.chat.entries.len(),
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

    fn small_groups(&mut self, path: &Path) -> Result<Value, String> {
        let (data, focus) = self.live_disk(path)?;
        let layout = SunburstLayout::compute(
            &data.tree,
            focus,
            DIAGRAM_SIDE,
            1.0,
            &domovoi_render::SmallGroups::default(),
            &domovoi_render::MarkedNodes::default(),
        );
        let groups = layout.groups().filter(|group| group.ring == 0).count();
        Ok(json!({
            "step": "small_groups",
            "groups": groups,
            "ring_sum": layout.ring_angle_sum(0),
            "segments": layout.segments.iter().filter(|segment| segment.ring == 0).count(),
        }))
    }

    fn preview_targets(&mut self, path: &Path) -> Result<Value, String> {
        let (data, focus) = self.live_disk(path)?;
        let rows = data.level_rows(focus);
        let nodes: Vec<usize> = rows.iter().map(|row| row.node).collect();
        let index = self.session().index().expect("preview targets follow a scan");
        let targets = PreviewTargets::from_level(&data.tree, index, &nodes);
        let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
        Ok(json!({ "step": "preview_targets", "rows": names, "targets": targets.paths.len() }))
    }

    fn export_csv(&self, path: &Path) -> Result<Value, String> {
        let index = self.session().index().expect("an export follows a scan");
        let lines = csv_lines(index, index.root());
        fs::write(path, &lines).map_err(|error| format!("{}: {error}", path.display()))?;
        let rows = lines.split(CSV_LINE_END).filter(|row| !row.is_empty()).count() - 1;
        let mut nodes = 0_usize;
        let mut pending = vec![index.root()];
        while let Some(id) = pending.pop() {
            nodes += 1;
            pending.extend(index.children(id));
        }
        Ok(json!({ "step": "export_csv", "path": path, "rows": rows, "nodes": nodes }))
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
            Some(summary) => self.duplicates_model(summary.seconds),
            None => DuplicatesModel::idle(),
        };
        let index = self.session().index().expect("live data follows a scan");
        let (tray, _, _) = self.measured_tray()?;
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
        let mut data = ShellData {
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
        };
        data.mark_tray(index, self.session().tray());
        Ok(data)
    }
}

fn drop_kind_name(kind: DropKind) -> &'static str {
    match kind {
        DropKind::Add => "add",
        DropKind::Denied => "denied",
        DropKind::Already => "already",
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
        ScenarioStep::DeleteRecommendations { .. } => "delete_recommendations",
        ScenarioStep::RemoveApplication { .. } => "remove_application",
        ScenarioStep::MarkFromList { .. } => "mark_from_list",
        ScenarioStep::UnmarkTrayRow { .. } => "unmark_tray_row",
        ScenarioStep::FindDuplicates => "find_duplicates",
        ScenarioStep::ReplaceWithClone => "replace_with_clone",
        ScenarioStep::DownloadModel { .. } => "download_model",
        ScenarioStep::AskAssistant { .. } => "ask_assistant",
        ScenarioStep::AskAboutCard { .. } => "ask_about_card",
        ScenarioStep::ScrollDiskList { .. } => "scroll_disk_list",
        ScenarioStep::UpdateDiskScreen => "update_disk_screen",
        ScenarioStep::Snapshots { .. } => "snapshots",
        ScenarioStep::ScanNotice => "scan_notice",
        ScenarioStep::SmallGroups { .. } => "small_groups",
        ScenarioStep::PreviewTargets { .. } => "preview_targets",
        ScenarioStep::ExportCsv { .. } => "export_csv",
        ScenarioStep::HiddenSpace => "hidden_space",
        ScenarioStep::TmutilCommands => "tmutil_commands",
        ScenarioStep::PurgeSpace { .. } => "purge_space",
    }
}
