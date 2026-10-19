use std::cell::OnceCell;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObjectProtocol};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSApplication, NSApplicationDelegate, NSMenuItem, NSMenuItemValidation};
use objc2_foundation::{NSNotification, NSObject};

use domovoi_downloader::ModelStore;

use crate::action_handler::ActionHandler;
use crate::session_paths::SessionPaths;
use crate::shell_action::ShellAction;
use crate::shell_controller::ShellController;
use crate::updater_controller::UpdaterController;

#[derive(Debug)]
pub(crate) struct AppDelegateState {
    paths: SessionPaths,
    models: ModelStore,
    controller: OnceCell<Rc<ShellController>>,
    updater: OnceCell<Option<Retained<UpdaterController>>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = AppDelegateState]
    #[derive(Debug)]
    pub(crate) struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    impl AppDelegate {
        #[unsafe(method(checkForUpdates:))]
        fn check_for_updates(&self, _sender: Option<&AnyObject>) {
            if let Some(updater) = self.updater() {
                updater.check_for_updates();
            }
        }

        #[unsafe(method(rescanDisk:))]
        fn rescan_disk(&self, _sender: Option<&AnyObject>) {
            if let Some(controller) = self.ivars().controller.get() {
                controller.dispatch(ShellAction::Rescan);
            }
        }
    }

    unsafe impl NSMenuItemValidation for AppDelegate {
        #[unsafe(method(validateMenuItem:))]
        fn validate_menu_item(&self, item: &NSMenuItem) -> bool {
            match item.action() {
                Some(action) if action == sel!(checkForUpdates:) => self.updater().is_some(),
                Some(action) if action == sel!(rescanDisk:) => self
                    .ivars()
                    .controller
                    .get()
                    .is_some_and(|controller| controller.allows(ShellAction::Rescan)),
                _ => true,
            }
        }
    }

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn application_did_finish_launching(&self, _notification: &NSNotification) {
            let state = self.ivars();
            let controller =
                ShellController::launch(MainThreadMarker::from(self), state.paths.clone(), state.models.clone());
            self.ivars()
                .controller
                .set(controller)
                .expect("the application finishes launching once");
            self.ivars()
                .updater
                .set(UpdaterController::start(MainThreadMarker::from(self)))
                .expect("the updater starts once");
        }

        #[unsafe(method(applicationDidBecomeActive:))]
        fn application_did_become_active(&self, _notification: &NSNotification) {
            if let Some(controller) = self.ivars().controller.get() {
                controller.application_became_active();
            }
        }

        #[unsafe(method(applicationWillTerminate:))]
        fn application_will_terminate(&self, _notification: &NSNotification) {
            if let Some(controller) = self.ivars().controller.get() {
                controller.application_will_terminate();
            }
        }

        #[unsafe(method(applicationShouldTerminateAfterLastWindowClosed:))]
        fn should_terminate_after_last_window_closed(&self, _application: &NSApplication) -> bool {
            true
        }
    }
);

impl AppDelegate {
    fn updater(&self) -> Option<&UpdaterController> {
        self.ivars().updater.get().and_then(Option::as_deref)
    }

    pub(crate) fn new(marker: MainThreadMarker, paths: SessionPaths, models: ModelStore) -> Retained<Self> {
        let this = Self::alloc(marker).set_ivars(AppDelegateState {
            paths,
            models,
            controller: OnceCell::new(),
            updater: OnceCell::new(),
        });
        unsafe { msg_send![super(this), init] }
    }
}
