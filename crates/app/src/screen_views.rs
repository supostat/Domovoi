use domovoi_render::DiagramView;
use objc2::rc::Retained;

use crate::access_screen::AccessScreen;
use crate::main_screen::MainScreen;
use crate::scanning_screen::ScanningScreen;

#[derive(Debug)]
pub(crate) enum ScreenViews {
    Access(AccessScreen),
    Scanning {
        screen: ScanningScreen,
        diagram: Retained<DiagramView>,
    },
    Main {
        screen: MainScreen,
        diagram: Retained<DiagramView>,
    },
}

impl ScreenViews {
    pub(crate) fn diagram(&self) -> Option<&DiagramView> {
        match self {
            Self::Access(_) => None,
            Self::Scanning { diagram, .. } | Self::Main { diagram, .. } => Some(diagram),
        }
    }
}
