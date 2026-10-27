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
    ToggleSmallGroups,
    Focus(usize),
    ShowInFinder(usize),
    ShowClickedRowInFinder,
    OpenClickedRow,
    OpenInTerminal(usize),
    OpenClickedRowInTerminal,
    ShowProperties(usize),
    ShowClickedRowProperties,
    CopyTmutil,
    ExportCsv,
    TogglePreview,
    Navigate(Section),
    OpenTray,
    ToggleCard(usize),
    SendCardsToTray,
    ToggleDuplicateSearch,
    SelectDuplicateAction(bool),
    ToggleCopy(usize),
    ApplyDuplicates,
    DeleteTray,
    StopDelete,
    CloseTray,
    AskAssistant,
    StopAnswer,
    AddAnswerToTray(usize),
    ToggleAssistant,
    #[cfg(feature = "claude-code")]
    ToggleClaudeCode,
    DownloadModel(usize),
    PauseDownload(usize),
    CancelDownload(usize),
    RemoveModel(usize),
    ActivateModel(usize),
    ChooseUnloadDelay,
    SelectUnloadDelay(usize),
    ChangeSource,
    ChooseModelFile,
    DeleteOrphan(usize),
}

impl ShellAction {
    pub fn needs_session(self) -> bool {
        matches!(
            self,
            Self::Rescan
                | Self::OpenTray
                | Self::SendCardsToTray
                | Self::DeleteTray
                | Self::AddAnswerToTray(_)
                | Self::ToggleDuplicateSearch
                | Self::ApplyDuplicates
                | Self::AskAssistant
                | Self::ExportCsv
        )
    }
}
