use std::cell::RefCell;
use std::rc::Weak;

use domovoi_render::DiagramMode;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObjectProtocol, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSMenuItem, NSMenuItemValidation};
use objc2_foundation::{NSInteger, NSObject};

use crate::action_handler::ActionHandler;
use crate::section::Section;
use crate::shell_action::ShellAction;

pub(crate) const SUNBURST_SEGMENT: usize = 0;
pub(crate) const TREEMAP_SEGMENT: usize = 1;
pub(crate) const TRASH_SEGMENT: usize = 0;
pub(crate) const CLONE_SEGMENT: usize = 1;

#[derive(Default)]
pub struct ShellActionsState {
    handler: RefCell<Option<Weak<dyn ActionHandler>>>,
}

impl std::fmt::Debug for ShellActionsState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("ShellActionsState").finish_non_exhaustive()
    }
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = ShellActionsState]
    #[derive(Debug)]
    pub struct ShellActions;

    unsafe impl NSObjectProtocol for ShellActions {}

    impl ShellActions {
        #[unsafe(method(openSettings:))]
        fn open_settings(&self, _sender: Option<&AnyObject>) {
            self.dispatch(ShellAction::OpenSettings);
        }

        #[unsafe(method(scanWithoutAccess:))]
        fn scan_without_access(&self, _sender: Option<&AnyObject>) {
            self.dispatch(ShellAction::ScanWithoutAccess);
        }

        #[unsafe(method(relaunch:))]
        fn relaunch(&self, _sender: Option<&AnyObject>) {
            self.dispatch(ShellAction::Relaunch);
        }

        #[unsafe(method(stopScan:))]
        fn stop_scan(&self, _sender: Option<&AnyObject>) {
            self.dispatch(ShellAction::StopScan);
        }

        #[unsafe(method(selectMode:))]
        fn select_mode(&self, sender: &AnyObject) {
            let mode = if tag_of(sender) == SUNBURST_SEGMENT { DiagramMode::Sunburst } else { DiagramMode::Treemap };
            self.dispatch(ShellAction::ChangeMode(mode));
        }

        #[unsafe(method(openTray:))]
        fn open_tray(&self, _sender: Option<&AnyObject>) {
            self.dispatch(ShellAction::OpenTray);
        }

        #[unsafe(method(toggleCard:))]
        fn toggle_card(&self, sender: &AnyObject) {
            self.dispatch(ShellAction::ToggleCard(tag_of(sender)));
        }

        #[unsafe(method(sendCardsToTray:))]
        fn send_cards_to_tray(&self, _sender: Option<&AnyObject>) {
            self.dispatch(ShellAction::SendCardsToTray);
        }

        #[unsafe(method(toggleDuplicateSearch:))]
        fn toggle_duplicate_search(&self, _sender: Option<&AnyObject>) {
            self.dispatch(ShellAction::ToggleDuplicateSearch);
        }

        #[unsafe(method(selectDuplicateAction:))]
        fn select_duplicate_action(&self, sender: &AnyObject) {
            self.dispatch(ShellAction::SelectDuplicateAction(tag_of(sender) == CLONE_SEGMENT));
        }

        #[unsafe(method(toggleCopy:))]
        fn toggle_copy(&self, sender: &AnyObject) {
            self.dispatch(ShellAction::ToggleCopy(tag_of(sender)));
        }

        #[unsafe(method(applyDuplicates:))]
        fn apply_duplicates(&self, _sender: Option<&AnyObject>) {
            self.dispatch(ShellAction::ApplyDuplicates);
        }

        #[unsafe(method(confirmTray:))]
        fn confirm_tray(&self, _sender: Option<&AnyObject>) {
            self.dispatch(ShellAction::ConfirmTray);
        }

        #[unsafe(method(closeTray:))]
        fn close_tray(&self, _sender: Option<&AnyObject>) {
            self.dispatch(ShellAction::CloseTray);
        }

        #[unsafe(method(openTrash:))]
        fn open_trash(&self, _sender: Option<&AnyObject>) {
            self.dispatch(ShellAction::OpenTrash);
        }

        #[unsafe(method(restore:))]
        fn restore(&self, sender: &AnyObject) {
            self.dispatch(ShellAction::Restore(tag_of(sender)));
        }

        #[unsafe(method(askAssistant:))]
        fn ask_assistant(&self, _sender: Option<&AnyObject>) {
            self.dispatch(ShellAction::AskAssistant);
        }

        #[unsafe(method(stopAnswer:))]
        fn stop_answer(&self, _sender: Option<&AnyObject>) {
            self.dispatch(ShellAction::StopAnswer);
        }

        #[unsafe(method(addAnswerToTray:))]
        fn add_answer_to_tray(&self, sender: &AnyObject) {
            self.dispatch(ShellAction::AddAnswerToTray(tag_of(sender)));
        }

        #[unsafe(method(openModelSection:))]
        fn open_model_section(&self, _sender: Option<&AnyObject>) {
            self.dispatch(ShellAction::Navigate(Section::Model));
        }

        #[unsafe(method(toggleAssistant:))]
        fn toggle_assistant(&self, _sender: Option<&AnyObject>) {
            self.dispatch(ShellAction::ToggleAssistant);
        }

        #[unsafe(method(downloadModel:))]
        fn download_model(&self, sender: &AnyObject) {
            self.dispatch(ShellAction::DownloadModel(tag_of(sender)));
        }

        #[unsafe(method(pauseDownload:))]
        fn pause_download(&self, sender: &AnyObject) {
            self.dispatch(ShellAction::PauseDownload(tag_of(sender)));
        }

        #[unsafe(method(cancelDownload:))]
        fn cancel_download(&self, sender: &AnyObject) {
            self.dispatch(ShellAction::CancelDownload(tag_of(sender)));
        }

        #[unsafe(method(removeModel:))]
        fn remove_model(&self, sender: &AnyObject) {
            self.dispatch(ShellAction::RemoveModel(tag_of(sender)));
        }

        #[unsafe(method(activateModel:))]
        fn activate_model(&self, sender: &AnyObject) {
            self.dispatch(ShellAction::ActivateModel(tag_of(sender)));
        }

        #[unsafe(method(chooseUnloadDelay:))]
        fn choose_unload_delay(&self, _sender: Option<&AnyObject>) {
            self.dispatch(ShellAction::ChooseUnloadDelay);
        }

        #[unsafe(method(selectUnloadDelay:))]
        fn select_unload_delay(&self, sender: &AnyObject) {
            self.dispatch(ShellAction::SelectUnloadDelay(tag_of(sender)));
        }

        #[unsafe(method(changeSource:))]
        fn change_source(&self, _sender: Option<&AnyObject>) {
            self.dispatch(ShellAction::ChangeSource);
        }

        #[unsafe(method(chooseModelFile:))]
        fn choose_model_file(&self, _sender: Option<&AnyObject>) {
            self.dispatch(ShellAction::ChooseModelFile);
        }

        #[unsafe(method(trashOrphan:))]
        fn trash_orphan(&self, sender: &AnyObject) {
            self.dispatch(ShellAction::TrashOrphan(tag_of(sender)));
        }

        #[unsafe(method(focusNode:))]
        fn focus_node(&self, sender: &AnyObject) {
            self.dispatch(ShellAction::Focus(tag_of(sender)));
        }

        #[unsafe(method(showNodeInFinder:))]
        fn show_node_in_finder(&self, sender: &AnyObject) {
            self.dispatch(ShellAction::ShowInFinder(tag_of(sender)));
        }

        #[unsafe(method(showClickedRowInFinder:))]
        fn show_clicked_row_in_finder(&self, _sender: Option<&AnyObject>) {
            self.dispatch(ShellAction::ShowClickedRowInFinder);
        }

        #[unsafe(method(openClickedRow:))]
        fn open_clicked_row(&self, _sender: Option<&AnyObject>) {
            self.dispatch(ShellAction::OpenClickedRow);
        }
    }

    unsafe impl NSMenuItemValidation for ShellActions {
        #[unsafe(method(validateMenuItem:))]
        fn validate_menu_item(&self, item: &NSMenuItem) -> bool {
            item.action()
                .and_then(|action| Self::action_of(action, item.tag()))
                .is_some_and(|action| self.allows(action))
        }
    }
);

impl ShellActions {
    pub fn new(marker: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(marker).set_ivars(ShellActionsState::default());
        unsafe { msg_send![super(this), init] }
    }

    pub fn connect(&self, handler: Weak<dyn ActionHandler>) {
        *self.ivars().handler.borrow_mut() = Some(handler);
    }

    pub fn target(&self) -> &AnyObject {
        self.as_ref()
    }

    pub(crate) fn dispatch(&self, action: ShellAction) {
        let handler = self.ivars().handler.borrow().as_ref().and_then(Weak::upgrade);
        if let Some(handler) = handler {
            handler.perform(action);
        }
    }

    fn allows(&self, action: ShellAction) -> bool {
        let handler = self.ivars().handler.borrow().as_ref().and_then(Weak::upgrade);
        handler.is_some_and(|handler| handler.allows(action))
    }

    fn action_of(selector: Sel, tag: NSInteger) -> Option<ShellAction> {
        let node = usize::try_from(tag).ok();
        if selector == sel!(showNodeInFinder:) {
            node.map(ShellAction::ShowInFinder)
        } else if selector == sel!(showClickedRowInFinder:) {
            Some(ShellAction::ShowClickedRowInFinder)
        } else if selector == sel!(focusNode:) {
            node.map(ShellAction::Focus)
        } else if selector == sel!(selectUnloadDelay:) {
            node.map(ShellAction::SelectUnloadDelay)
        } else {
            None
        }
    }
}

fn tag_of(sender: &AnyObject) -> usize {
    let tag: NSInteger = unsafe { msg_send![sender, tag] };
    usize::try_from(tag).expect("controls carry a non-negative position tag")
}
