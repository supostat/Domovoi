use domovoi_render::DiagramMode;

use crate::section::Section;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellAction {
    OpenSettings,
    ScanWithoutAccess,
    Relaunch,
    StopScan,
    ChangeMode(DiagramMode),
    Focus(usize),
    ShowInFinder(usize),
    ShowClickedRowInFinder,
    OpenClickedRow,
    Navigate(Section),
    OpenTray,
    ToggleCard(usize),
    SendCardsToTray,
    ToggleDuplicateSearch,
    SelectDuplicateAction(bool),
    ToggleCopy(usize),
    ApplyDuplicates,
    ConfirmTray,
    CloseTray,
    OpenTrash,
    Restore(usize),
}
