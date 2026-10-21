use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

use domovoi_index::ModelEvent;
use domovoi_render::{DiagramMode, NodeOrigin, ROOT};
use domovoi_theme::Palette;
use objc2::rc::Retained;
use objc2::runtime::{NSObjectProtocol, ProtocolObject};

use crate::access_state::AccessState;
use crate::apply_job::ApplyJob;
use crate::assistant::Assistant;
use crate::care_models::CareModels;
use crate::disk_overview::DiskOverview;
use crate::duplicate_job::DuplicateJob;
use crate::duplicate_summary::DuplicateSummary;
use crate::duplicates_model::DuplicatesModel;
use crate::duplicates_state::DuplicatesState;
use crate::event_watch::EventWatch;
use crate::expansion::{Expansion, NODE_BUDGET};
use crate::hover_tip::HoverTip;
use crate::purge_run::PurgeRun;
use crate::recommendations_model::RecommendationsModel;
use crate::scan_job::ScanJob;
use crate::scan_stage::ScanStage;
use crate::scan_status::ScanStatus;
use crate::screen_key::ScreenKey;
use crate::screen_views::ScreenViews;
use crate::section::Section;
use crate::session::Session;
use crate::shell_data::ShellData;
use crate::shell_gate::ShellGate;
use crate::shell_palette::ShellPalette;
use crate::stage::Stage;
use crate::tray_measurement::TrayMeasurement;
use crate::tray_model::TrayModel;
use crate::tray_sheet::TraySheet;

#[derive(Debug)]
pub(crate) struct ShellState {
    pub stage: Stage,
    pub section: Section,
    pub access: AccessState,
    pub palette: ShellPalette,
    pub session: Option<Session>,
    pub overview: Option<DiskOverview>,
    pub expansion: Expansion,
    pub data: Option<ShellData>,
    pub focus: usize,
    pub focus_path: Option<PathBuf>,
    pub mode: DiagramMode,
    pub views: Option<ScreenViews>,
    pub views_key: Option<ScreenKey>,
    pub scan: Option<ScanJob>,
    pub search: Option<DuplicateJob>,
    pub duplicate_summary: Option<DuplicateSummary>,
    pub clone_mode: bool,
    pub last_scan: Option<ScanStatus>,
    pub watch: Option<EventWatch>,
    pub tray: Option<TraySheet>,
    pub tray_measurement: Option<TrayMeasurement>,
    pub purge: Option<PurgeRun>,
    pub applying: Option<ApplyJob>,
    pub settings_opened: bool,
    pub scan_without_access: bool,
    pub contrast_observer: Option<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
    pub assistant: Assistant,
    pub pending_model_events: Vec<ModelEvent>,
}

impl ShellState {
    pub(crate) fn new(mode: DiagramMode, assistant: Assistant) -> Self {
        Self {
            stage: Stage::Access(AccessState::Missing),
            section: Section::Disk,
            access: AccessState::Missing,
            palette: ShellPalette {
                tokens: &Palette::LIGHT,
                high_contrast: false,
            },
            session: None,
            overview: None,
            expansion: Expansion::default(),
            data: None,
            focus: ROOT,
            focus_path: None,
            mode,
            views: None,
            views_key: None,
            scan: None,
            search: None,
            duplicate_summary: None,
            clone_mode: false,
            last_scan: None,
            watch: None,
            tray: None,
            tray_measurement: None,
            purge: None,
            applying: None,
            settings_opened: false,
            scan_without_access: false,
            contrast_observer: None,
            assistant,
            pending_model_events: Vec::new(),
        }
    }

    pub(crate) fn gate(&self) -> ShellGate {
        ShellGate {
            session_open: self.session.is_some(),
            tray_open: self.tray.is_some(),
            searching: self.search.is_some(),
        }
    }

    pub(crate) fn screen_key(&self) -> Option<ScreenKey> {
        if self.stage != Stage::Main {
            return None;
        }
        ScreenKey::of(
            self.section,
            self.palette,
            self.data.as_ref().expect("the main stage has data"),
            self.focus,
            self.mode,
        )
    }

    pub(crate) fn overview(&self) -> &DiskOverview {
        self.overview.as_ref().expect("the disk is measured before any scan")
    }

    pub(crate) fn scan_status(&self) -> ScanStatus {
        self.last_scan.clone().unwrap_or_else(|| ScanStatus {
            bytes: 0,
            files: 0,
            sweep: 1.0,
            threads: self.session.as_ref().map_or(0, Session::workers),
            current: String::new(),
            full_first_scan: false,
            stage: ScanStage::Walking,
        })
    }

    pub(crate) fn rebuild_data(&mut self, now: SystemTime) -> io::Result<()> {
        if let Some(session) = self.session.as_mut() {
            for event in self.pending_model_events.drain(..) {
                session.apply_model_event(&event);
            }
        }
        let previous = self.data.take().map(|data| data.care);
        let care = self.care_models(previous.as_ref(), now)?;
        let index = self.session.as_ref().and_then(Session::index);
        let focus_node = index.and_then(|index| self.focus_path.as_deref().and_then(|path| index.find(path)));
        self.expansion = index.map_or_else(Expansion::default, |index| {
            Expansion::around(index, focus_node.unwrap_or_else(|| index.root()), NODE_BUDGET)
        });
        let data = ShellData::measured(
            self.access,
            self.overview(),
            index,
            &self.expansion,
            self.scan_status(),
            care,
            self.assistant.models(),
        );
        self.focus = focus_node
            .and_then(|node| data.tree_position(NodeOrigin::Indexed(node)))
            .unwrap_or(ROOT);
        self.data = Some(data);
        Ok(())
    }

    pub(crate) fn remember_focus(&mut self, focus: usize) {
        self.focus = focus;
        self.focus_path = self.node_path(focus);
    }

    pub(crate) fn hover_tip(&self, node: usize) -> Option<HoverTip> {
        let data = self.data.as_ref()?;
        HoverTip::for_node(data, self.session.as_ref().and_then(Session::index), node)
    }

    pub(crate) fn node_path(&self, node: usize) -> Option<PathBuf> {
        let origin = self.data.as_ref()?.tree.node(node).origin;
        match (origin, self.session.as_ref().and_then(Session::index)) {
            (NodeOrigin::Indexed(id), Some(index)) => Some(index.path(id)),
            _ => None,
        }
    }

    pub(crate) fn care(&self) -> &CareModels {
        &self.data.as_ref().expect("care screens show built data").care
    }

    pub(crate) fn care_mut(&mut self) -> &mut CareModels {
        &mut self.data.as_mut().expect("care screens show built data").care
    }

    fn care_models(&self, previous: Option<&CareModels>, now: SystemTime) -> io::Result<CareModels> {
        let Some((session, index)) = self
            .session
            .as_ref()
            .and_then(|session| Some((session, session.index()?)))
        else {
            return Ok(CareModels::default());
        };
        let mut recommendations = session
            .recommendations()
            .map(|recommendations| RecommendationsModel::build(index, session.rules(), recommendations, now))
            .unwrap_or_default();
        if let Some(previous) = previous {
            for card in recommendations.cards.iter_mut().filter(|card| card.selectable()) {
                if let Some(old) = previous
                    .recommendations
                    .cards
                    .iter()
                    .find(|old| old.provenance == card.provenance && old.paths == card.paths)
                {
                    card.selected = old.selected;
                }
            }
        }
        let mut duplicates = match (&self.search, session.duplicates(), self.duplicate_summary) {
            (Some(job), _, _) => DuplicatesModel {
                state: DuplicatesState::Searching {
                    stage: job.stage,
                    done: job.done,
                    total: job.total,
                },
                clone_mode: false,
            },
            (None, Some(found), Some(summary)) => {
                DuplicatesModel::found(index, found, summary.candidates, summary.seconds)?
            }
            _ => DuplicatesModel::idle(),
        };
        duplicates.clone_mode = self.clone_mode;
        if self.clone_mode {
            duplicates.assess_clones(|keep, copy| session.clone_plan(keep, copy).map(drop));
        }
        if let Some(previous) = previous {
            for copy in duplicates
                .groups_mut()
                .iter_mut()
                .flat_map(|group| group.copies.iter_mut())
            {
                if let Some(old) = previous
                    .duplicates
                    .groups()
                    .iter()
                    .flat_map(|group| &group.copies)
                    .find(|old| old.path == copy.path)
                {
                    copy.checked = old.checked;
                }
            }
        }
        let tray = TrayModel::estimated(index, session.tray(), session.trash_journal());
        Ok(CareModels {
            recommendations,
            duplicates,
            tray,
        })
    }
}
