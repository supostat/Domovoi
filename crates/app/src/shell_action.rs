use domovoi_render::DiagramMode;

use crate::section::Section;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellAction {
    OpenSettings,
    ScanWithoutAccess,
    Relaunch,
    Rescan,
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
    AskAssistant,
    StopAnswer,
    AddAnswerToTray(usize),
    ToggleAssistant,
    DownloadModel(usize),
    PauseDownload(usize),
    CancelDownload(usize),
    RemoveModel(usize),
    ActivateModel(usize),
    ChooseUnloadDelay,
    SelectUnloadDelay(usize),
    ChangeSource,
    ChooseModelFile,
    TrashOrphan(usize),
}

impl ShellAction {
    pub fn needs_session(self) -> bool {
        matches!(
            self,
            Self::Rescan
                | Self::OpenTray
                | Self::SendCardsToTray
                | Self::ConfirmTray
                | Self::AddAnswerToTray(_)
                | Self::Restore(_)
                | Self::ToggleDuplicateSearch
                | Self::ApplyDuplicates
                | Self::AskAssistant
        )
    }
}
