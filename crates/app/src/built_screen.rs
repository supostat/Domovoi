use domovoi_render::{DiagramMode, DiagramState, ROOT};
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::NSView;
use objc2_foundation::{NSRect, NSSize};

use crate::access_screen::AccessScreen;
use crate::main_screen::MainScreen;
use crate::probes::Probes;
use crate::scanning_screen::ScanningScreen;
use crate::screen::Screen;
use crate::shell_actions::ShellActions;
use crate::shell_data::ShellData;
use crate::shell_layout::WINDOW;
use crate::shell_palette::ShellPalette;
use crate::window_frame::WindowFrame;

#[derive(Debug)]
pub(crate) enum BuiltScreen {
    Access(AccessScreen),
    Scanning(ScanningScreen),
    Main(MainScreen),
}

impl BuiltScreen {
    pub(crate) fn build(
        marker: MainThreadMarker,
        screen: Screen,
        data: &ShellData,
        palette: &ShellPalette,
        actions: &ShellActions,
    ) -> Self {
        match screen {
            Screen::Access => Self::Access(AccessScreen::build(marker, data.access, palette, actions)),
            Screen::Scanning => Self::Scanning(ScanningScreen::build(marker, data, palette, actions)),
            Screen::Main => Self::Main(MainScreen::build(
                marker,
                data,
                ROOT,
                DiagramMode::Sunburst,
                palette,
                actions,
            )),
        }
    }

    pub(crate) fn root(&self) -> Retained<NSView> {
        match self {
            Self::Access(screen) => screen.root.clone().into_super(),
            Self::Scanning(screen) => screen.frame.root(),
            Self::Main(screen) => screen.frame.root(),
        }
    }

    pub(crate) fn probes(&self) -> &Probes {
        match self {
            Self::Access(screen) => &screen.probes,
            Self::Scanning(screen) => &screen.probes,
            Self::Main(screen) => &screen.probes,
        }
    }

    pub(crate) fn window(&self) -> NSSize {
        match self {
            Self::Access(screen) => screen.size(),
            Self::Scanning(_) | Self::Main(_) => WINDOW,
        }
    }

    pub(crate) fn diagram(&self, data: &ShellData, palette: &ShellPalette) -> Option<(NSRect, DiagramState)> {
        let (frame, state) = match self {
            Self::Access(_) => return None,
            Self::Scanning(screen) => (screen.diagram_frame, ScanningScreen::diagram_state(data, palette)),
            Self::Main(screen) => (
                screen.diagram_frame,
                MainScreen::diagram_state(data, ROOT, DiagramMode::Sunburst, palette),
            ),
        };
        let mut in_window = frame;
        in_window.origin.x += WindowFrame::pane_origin_x();
        Some((in_window, state))
    }
}
