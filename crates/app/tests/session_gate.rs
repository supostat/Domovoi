use domovoi_app::{Section, ShellAction, ShellGate};
use domovoi_render::DiagramMode;

const SESSION_ACTIONS: [ShellAction; 9] = [
    ShellAction::Rescan,
    ShellAction::SendCardsToTray,
    ShellAction::OpenTray,
    ShellAction::ConfirmTray,
    ShellAction::AddAnswerToTray(0),
    ShellAction::Restore(0),
    ShellAction::ToggleDuplicateSearch,
    ShellAction::ApplyDuplicates,
    ShellAction::AskAssistant,
];

const FREE_ACTIONS: [ShellAction; 13] = [
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
];

const IDLE: ShellGate = ShellGate {
    session_open: true,
    tray_open: false,
    searching: false,
};

const BUSY: ShellGate = ShellGate {
    session_open: false,
    tray_open: false,
    searching: false,
};

#[test]
fn the_rescan_tray_restore_duplicates_and_questions_wait_for_the_session() {
    for action in SESSION_ACTIONS {
        assert!(action.needs_session(), "{action:?} reads or changes the index");
    }
}

#[test]
fn navigation_card_choice_and_the_diagram_stay_available() {
    for action in FREE_ACTIONS {
        assert!(!action.needs_session(), "{action:?} works on the data already shown");
    }
}

#[test]
fn a_rescan_waits_for_the_session_and_a_closed_tray() {
    assert!(IDLE.allows(ShellAction::Rescan));
    assert!(!BUSY.allows(ShellAction::Rescan), "a busy session cannot be rescanned");
    assert!(
        !ShellGate {
            tray_open: true,
            ..IDLE
        }
        .allows(ShellAction::Rescan),
        "the open tray keeps measuring through the session"
    );
}

#[test]
fn the_session_is_lent_only_while_the_tray_is_closed() {
    assert!(IDLE.lends_session());
    assert!(!BUSY.lends_session(), "a busy session is not home to lend");
    assert!(
        !ShellGate {
            tray_open: true,
            ..IDLE
        }
        .lends_session(),
        "the open tray keeps measuring through the session"
    );
}

#[test]
fn a_rescan_is_allowed_exactly_when_the_session_can_be_lent() {
    for session_open in [true, false] {
        for tray_open in [true, false] {
            let gate = ShellGate {
                session_open,
                tray_open,
                searching: false,
            };
            assert_eq!(gate.allows(ShellAction::Rescan), gate.lends_session(), "{gate:?}");
        }
    }
}

#[test]
fn a_running_search_is_stopped_without_the_session() {
    let searching = ShellGate {
        searching: true,
        ..BUSY
    };
    assert!(
        searching.allows(ShellAction::ToggleDuplicateSearch),
        "the search button is its stop"
    );
    assert!(!BUSY.allows(ShellAction::ToggleDuplicateSearch));
}

#[test]
fn a_busy_session_refuses_what_needs_it_and_keeps_the_rest() {
    for action in SESSION_ACTIONS {
        assert!(!BUSY.allows(action), "{action:?} waits for the session");
    }
    for action in FREE_ACTIONS {
        assert!(BUSY.allows(action), "{action:?} stays available");
    }
}

#[test]
fn the_open_tray_holds_back_only_the_rescan() {
    let tray = ShellGate {
        tray_open: true,
        ..IDLE
    };
    for action in SESSION_ACTIONS.into_iter().chain(FREE_ACTIONS) {
        assert_eq!(
            tray.allows(action),
            action != ShellAction::Rescan,
            "{action:?} with the tray sheet open"
        );
    }
}
