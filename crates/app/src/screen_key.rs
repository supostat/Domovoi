use crate::duplicates_model::DuplicatesModel;
use crate::duplicates_state::DuplicatesState;
use crate::section::Section;
use crate::shell_data::ShellData;
use crate::shell_palette::ShellPalette;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenKey {
    palette: ShellPalette,
    shape: Shape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Recommendations { cards: bool },
    Duplicates(DuplicatesBody),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DuplicatesBody {
    Idle,
    Searching,
    NoneFound,
    Found { clone_mode: bool, clone_available: bool },
}

impl ScreenKey {
    pub fn of(section: Section, palette: ShellPalette, data: &ShellData) -> Option<Self> {
        let shape = match section {
            Section::Recommendations => Shape::Recommendations {
                cards: !data.care.recommendations.cards.is_empty(),
            },
            Section::Duplicates => Shape::Duplicates(DuplicatesBody::of(&data.care.duplicates)),
            Section::Disk | Section::Assistant | Section::Model => return None,
        };
        Some(Self { palette, shape })
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
