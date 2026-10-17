use domovoi_downloader::ModelStore;
use objc2::MainThreadMarker;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

use crate::app_delegate::AppDelegate;
use crate::main_menu::main_menu;
use crate::session_paths::SessionPaths;

pub fn run_interactive(marker: MainThreadMarker) -> Result<(), String> {
    let paths = SessionPaths::for_user().map_err(|error| error.to_string())?;
    paths.create_directories().map_err(|error| error.to_string())?;
    let models = ModelStore::open(&paths.models).map_err(|error| error.to_string())?;
    let application = NSApplication::sharedApplication(marker);
    application.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    application.setMainMenu(Some(&main_menu(marker)));
    let delegate = AppDelegate::new(marker, paths, models);
    application.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    application.activate();
    application.run();
    Ok(())
}
