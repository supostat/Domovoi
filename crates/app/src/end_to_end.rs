use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use domovoi_scanner::CancelToken;
use objc2::MainThreadMarker;
use serde_json::{Value, json};

use crate::access_state::AccessState;
use crate::cache_opening::CacheOpening;
use crate::disk_overview::DiskOverview;
use crate::event_effect::EventEffect;
use crate::expansion::{Expansion, NODE_BUDGET};
use crate::running_applications::running_bundle_ids;
use crate::scan_status::ScanStatus;
use crate::scenario::Scenario;
use crate::scenario_step::ScenarioStep;
use crate::screen::Screen;
use crate::session::Session;
use crate::shell_data::ShellData;
use crate::shell_snapshot::write_snapshots;
use crate::snapshot_options::SnapshotOptions;

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
    cache: PathBuf,
    session: Session,
    snapshots: Vec<PathBuf>,
}

impl EndToEnd {
    fn open(scenario: &Scenario) -> Result<Self, String> {
        Ok(Self {
            root: scenario.root.clone(),
            cache: scenario.cache.clone(),
            session: Session::open(&scenario.root, &scenario.cache, APP_VERSION).map_err(|error| error.to_string())?,
            snapshots: Vec::new(),
        })
    }

    fn perform(&mut self, marker: MainThreadMarker, step: &ScenarioStep) -> Result<Value, String> {
        match step {
            ScenarioStep::Scan => {
                self.session
                    .scan(None, CancelToken::default())
                    .map_err(|error| error.to_string())?;
                self.refresh()?;
                let index = self.session.index().expect("a finished scan installs an index");
                let root = index.node(index.root());
                Ok(json!({ "step": "scan", "files": root.files, "bytes": root.allocated, "nodes": index.len() }))
            }
            ScenarioStep::ReopenCache => {
                let mut reopened =
                    Session::open(&self.root, &self.cache, APP_VERSION).map_err(|error| error.to_string())?;
                match reopened.open_cached().map_err(|error| error.to_string())? {
                    CacheOpening::Opened => {}
                    opening => return Err(format!("the scan cache was not reused: {opening:?}")),
                }
                self.session = reopened;
                self.refresh()?;
                Ok(json!({ "step": "reopen_cache", "last_event_id": self.session.last_event_id() }))
            }
            ScenarioStep::AwaitChange { path, timeout_seconds } => {
                let allocated = self.await_change(path, Duration::from_secs(*timeout_seconds))?;
                self.refresh()?;
                Ok(json!({ "step": "await_change", "path": path, "allocated": allocated }))
            }
            ScenarioStep::Recommendations => {
                self.refresh()?;
                let cards = self
                    .session
                    .recommendations()
                    .map_or(0, |recommendations| recommendations.cards.len());
                Ok(json!({ "step": "recommendations", "cards": cards }))
            }
            ScenarioStep::Snapshots { directory, scale } => {
                let written = self.snapshots(marker, directory, *scale)?;
                self.snapshots.extend(written.iter().cloned());
                Ok(json!({ "step": "snapshots", "files": written }))
            }
        }
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
            match self.session.apply_event(&event).map_err(|error| error.to_string())? {
                EventEffect::Applied | EventEffect::Ignored => {}
                EventEffect::RescanRequired => return Err(format!("the volume asked for a full rescan: {event:?}")),
            }
        }
    }

    fn snapshots(&self, marker: MainThreadMarker, directory: &Path, scale: f64) -> Result<Vec<PathBuf>, String> {
        let overview = DiskOverview::measure(&self.root).map_err(|error| error.to_string())?;
        let index = self.session.index();
        let expansion = index.map_or_else(Expansion::default, |index| {
            Expansion::around(index, index.root(), NODE_BUDGET)
        });
        let scan = index.map_or_else(
            || ScanStatus {
                bytes: 0,
                files: 0,
                sweep: 0.0,
                threads: self.session.workers(),
                current: self.root.display().to_string(),
                full_first_scan: false,
            },
            |index| {
                let root = index.node(index.root());
                ScanStatus {
                    bytes: root.allocated,
                    files: u64::from(root.files),
                    sweep: 1.0,
                    threads: self.session.workers(),
                    current: self.root.display().to_string(),
                    full_first_scan: false,
                }
            },
        );
        let data = ShellData::measured(AccessState::probe(), &overview, index, &expansion, scan);
        let mut written = Vec::new();
        for dark in [true, false] {
            let options = SnapshotOptions {
                screen: Screen::Access,
                dark,
                scale,
                high_contrast: false,
                directory: directory.to_path_buf(),
            };
            written.extend(write_snapshots(marker, &data, &Screen::ALL, &options)?);
        }
        Ok(written)
    }

    fn refresh(&mut self) -> Result<(), String> {
        self.session
            .refresh_recommendations(&running_bundle_ids())
            .map_err(|error| error.to_string())
    }
}

fn step_name(step: &ScenarioStep) -> &'static str {
    match step {
        ScenarioStep::Scan => "scan",
        ScenarioStep::ReopenCache => "reopen_cache",
        ScenarioStep::AwaitChange { .. } => "await_change",
        ScenarioStep::Recommendations => "recommendations",
        ScenarioStep::Snapshots { .. } => "snapshots",
    }
}
