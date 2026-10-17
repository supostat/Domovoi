use std::cell::RefCell;
use std::rc::Weak;

use domovoi_render::DiagramMode;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObjectProtocol, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSMenuItem, NSMenuItemValidation};
use objc2_foundation::{NSInteger, NSObject};

use crate::action_handler::ActionHandler;
use crate::mode_switch::SUNBURST_TAG;
use crate::shell_action::ShellAction;

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
            let tag: NSInteger = unsafe { msg_send![sender, tag] };
            let mode = if tag == SUNBURST_TAG { DiagramMode::Sunburst } else { DiagramMode::Treemap };
            self.dispatch(ShellAction::ChangeMode(mode));
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

    fn dispatch(&self, action: ShellAction) {
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
        } else {
            None
        }
    }
}

fn tag_of(sender: &AnyObject) -> usize {
    let tag: NSInteger = unsafe { msg_send![sender, tag] };
    usize::try_from(tag).expect("controls tag tree nodes with their index")
}
