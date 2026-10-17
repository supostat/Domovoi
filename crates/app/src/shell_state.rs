use std::path::PathBuf;

use domovoi_render::{DiagramMode, NodeOrigin, ROOT};
use domovoi_theme::Palette;
use objc2::rc::Retained;
use objc2::runtime::{NSObjectProtocol, ProtocolObject};

use crate::access_state::AccessState;
use crate::disk_overview::DiskOverview;
use crate::event_watch::EventWatch;
use crate::expansion::Expansion;
use crate::hover_tip::HoverTip;
use crate::scan_job::ScanJob;
use crate::scan_status::ScanStatus;
use crate::screen_views::ScreenViews;
use crate::session::Session;
use crate::shell_data::ShellData;
use crate::shell_palette::ShellPalette;
use crate::stage::Stage;

#[derive(Debug)]
pub(crate) struct ShellState {
    pub stage: Stage,
    pub access: AccessState,
    pub palette: ShellPalette,
    pub session: Option<Session>,
    pub overview: Option<DiskOverview>,
    pub expansion: Expansion,
    pub data: Option<ShellData>,
    pub focus: usize,
    pub mode: DiagramMode,
    pub views: Option<ScreenViews>,
    pub scan: Option<ScanJob>,
    pub last_scan: Option<ScanStatus>,
    pub watch: Option<EventWatch>,
    pub settings_opened: bool,
    pub scan_without_access: bool,
    pub contrast_observer: Option<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
}

impl ShellState {
    pub(crate) fn new(mode: DiagramMode) -> Self {
        Self {
            stage: Stage::Access(AccessState::Missing),
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
            mode,
            views: None,
            scan: None,
            last_scan: None,
            watch: None,
            settings_opened: false,
            scan_without_access: false,
            contrast_observer: None,
        }
    }

    pub(crate) fn overview(&self) -> &DiskOverview {
        self.overview.as_ref().expect("the disk is measured before any scan")
    }

    pub(crate) fn shell_data(&self) -> ShellData {
        let scan = self.last_scan.clone().unwrap_or_else(|| ScanStatus {
            bytes: 0,
            files: 0,
            sweep: 1.0,
            threads: self.session.as_ref().map_or(0, Session::workers),
            current: String::new(),
            full_first_scan: false,
        });
        ShellData::measured(
            self.access,
            self.overview(),
            self.session.as_ref().and_then(Session::index),
            &self.expansion,
            scan,
        )
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
}
