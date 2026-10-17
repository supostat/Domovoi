use domovoi_render::DiagramMode;

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
}
