use domovoi_render::DiagramMode;

use crate::availability::Availability;
use crate::duplicates_model::DuplicatesModel;
use crate::duplicates_state::DuplicatesState;
use crate::section::Section;
use crate::shell_data::ShellData;
use crate::shell_palette::ShellPalette;

#[derive(Clone, Debug, PartialEq)]
pub struct ScreenKey {
    palette: ShellPalette,
    shape: Shape,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Shape {
    Disk {
        level: Vec<String>,
        mode: DiagramMode,
        in_cloud: bool,
    },
    Recommendations {
        cards: bool,
    },
    Duplicates(DuplicatesBody),
    Assistant(AssistantBody),
    Model,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DuplicatesBody {
    Idle,
    Searching,
    NoneFound,
    Found { clone_mode: bool, clone_available: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AssistantBody {
    Transcript,
    Unavailable(Availability),
}

impl ScreenKey {
    pub fn of(section: Section, palette: ShellPalette, data: &ShellData, focus: usize, mode: DiagramMode) -> Self {
        let shape = match section {
            Section::Disk => Shape::Disk {
                level: data.crumbs(focus).into_iter().map(|crumb| crumb.title).collect(),
                mode,
                in_cloud: data.in_cloud_bytes > 0,
            },
            Section::Recommendations => Shape::Recommendations {
                cards: !data.care.recommendations.cards.is_empty(),
            },
            Section::Duplicates => Shape::Duplicates(DuplicatesBody::of(&data.care.duplicates)),
            Section::Assistant => Shape::Assistant(AssistantBody::of(data.ai.chat.availability)),
            Section::Model => Shape::Model,
        };
        Self { palette, shape }
    }
}

impl DuplicatesBody {
    fn of(model: &DuplicatesModel) -> Self {
        match &model.state {
            DuplicatesState::Idle => Self::Idle,
            DuplicatesState::Searching { .. } => Self::Searching,
            DuplicatesState::Found { groups, .. } if groups.is_empty() => Self::NoneFound,
            DuplicatesState::Found { .. } => Self::Found {
                clone_mode: model.clone_mode,
                clone_available: model.clone_available(),
            },
        }
    }
}

impl AssistantBody {
    fn of(availability: Availability) -> Self {
        if availability.shows_transcript() {
            Self::Transcript
        } else {
            Self::Unavailable(availability)
        }
    }
}
