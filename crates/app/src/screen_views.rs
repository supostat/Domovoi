use domovoi_render::DiagramView;
use objc2::rc::Retained;

use crate::access_screen::AccessScreen;
use crate::assistant_screen::AssistantScreen;
use crate::duplicates_screen::DuplicatesScreen;
use crate::main_screen::MainScreen;
use crate::model_screen::ModelScreen;
use crate::recommendations_screen::RecommendationsScreen;
use crate::scanning_screen::ScanningScreen;
use crate::shell_data::ShellData;

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
    Assistant(AssistantScreen),
    Duplicates(DuplicatesScreen),
    Model(ModelScreen),
}

impl ScreenViews {
    pub(crate) fn update(&self, data: &ShellData) {
        match self {
            Self::Recommendations(screen) => screen.update(data),
            Self::Duplicates(screen) => screen.update(data),
            Self::Access(_) | Self::Scanning { .. } | Self::Main { .. } | Self::Assistant(_) | Self::Model(_) => {
                unreachable!("only screens with a key update in place")
            }
        }
    }

    pub(crate) fn diagram(&self) -> Option<&DiagramView> {
        match self {
            Self::Scanning { diagram, .. } | Self::Main { diagram, .. } => Some(diagram),
            Self::Access(_) | Self::Recommendations(_) | Self::Assistant(_) | Self::Duplicates(_) | Self::Model(_) => {
                None
            }
        }
    }
}
