use domovoi_app::{Section, ShellAction};
use domovoi_render::DiagramMode;

#[test]
fn the_tray_restore_duplicates_and_questions_wait_for_the_session() {
    for action in [
        ShellAction::SendCardsToTray,
        ShellAction::OpenTray,
        ShellAction::ConfirmTray,
        ShellAction::AddAnswerToTray(0),
        ShellAction::Restore(0),
        ShellAction::ToggleDuplicateSearch,
        ShellAction::ApplyDuplicates,
        ShellAction::AskAssistant,
    ] {
        assert!(action.needs_session(), "{action:?} reads or changes the index");
    }
}

#[test]
fn navigation_card_choice_and_the_diagram_stay_available() {
    for action in [
        ShellAction::Navigate(Section::Disk),
        ShellAction::Navigate(Section::Recommendations),
        ShellAction::Navigate(Section::Duplicates),
        ShellAction::ToggleCard(0),
        ShellAction::Focus(0),
        ShellAction::ChangeMode(DiagramMode::Treemap),
        ShellAction::ShowInFinder(0),
        ShellAction::ToggleCopy(0),
        ShellAction::SelectDuplicateAction(true),
        ShellAction::CloseTray,
        ShellAction::OpenTrash,
        ShellAction::StopAnswer,
        ShellAction::StopScan,
    ] {
        assert!(!action.needs_session(), "{action:?} works on the data already shown");
    }
}
