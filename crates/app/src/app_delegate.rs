use std::cell::OnceCell;
use std::rc::Rc;

use objc2::rc::Retained;
use objc2::runtime::NSObjectProtocol;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSApplication, NSApplicationDelegate};
use objc2_foundation::{NSNotification, NSObject};

use crate::session_paths::SessionPaths;
use crate::shell_controller::ShellController;

#[derive(Debug)]
pub(crate) struct AppDelegateState {
    paths: SessionPaths,
    controller: OnceCell<Rc<ShellController>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = AppDelegateState]
    #[derive(Debug)]
    pub(crate) struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn application_did_finish_launching(&self, _notification: &NSNotification) {
            let controller = ShellController::launch(MainThreadMarker::from(self), self.ivars().paths.clone());
            self.ivars()
                .controller
                .set(controller)
                .expect("the application finishes launching once");
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
    pub(crate) fn new(marker: MainThreadMarker, paths: SessionPaths) -> Retained<Self> {
        let this = Self::alloc(marker).set_ivars(AppDelegateState {
            paths,
            controller: OnceCell::new(),
        });
        unsafe { msg_send![super(this), init] }
    }
}
