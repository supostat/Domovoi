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
    RemoveApplication(usize),
    RemoveClickedApplication,
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
    UnmarkTrayRow(usize),
    DragFromList,
    DragEnded,
    DropOnTray,
    StopDelete,
    CloseTray,
    PurgeSpace,
    ReleasePurge,
    StopPurge,
    ClosePurge,
    AskAssistant,
    AskAboutCard(usize),
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
                | Self::UnmarkTrayRow(_)
                | Self::DragFromList
                | Self::DropOnTray
                | Self::AddAnswerToTray(_)
                | Self::RemoveApplication(_)
                | Self::RemoveClickedApplication
                | Self::ToggleDuplicateSearch
                | Self::ApplyDuplicates
                | Self::AskAssistant
                | Self::AskAboutCard(_)
                | Self::ExportCsv
        )
    }
}
