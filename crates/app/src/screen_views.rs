use domovoi_render::DiagramView;
use objc2::rc::Retained;

use crate::access_screen::AccessScreen;
use crate::duplicates_screen::DuplicatesScreen;
use crate::main_screen::MainScreen;
use crate::recommendations_screen::RecommendationsScreen;
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
    Recommendations(RecommendationsScreen),
    Duplicates(DuplicatesScreen),
}

impl ScreenViews {
    pub(crate) fn diagram(&self) -> Option<&DiagramView> {
        match self {
            Self::Scanning { diagram, .. } | Self::Main { diagram, .. } => Some(diagram),
            Self::Access(_) | Self::Recommendations(_) | Self::Duplicates(_) => None,
        }
    }
}
