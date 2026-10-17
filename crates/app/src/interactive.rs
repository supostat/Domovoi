use std::fs;

use domovoi_index::cache_directory;
use objc2::MainThreadMarker;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};

use crate::app_delegate::AppDelegate;
use crate::main_menu::main_menu;

pub fn run_interactive(marker: MainThreadMarker) -> Result<(), String> {
    let cache = cache_directory().map_err(|error| error.to_string())?;
    fs::create_dir_all(&cache).map_err(|error| format!("{}: {error}", cache.display()))?;
    let application = NSApplication::sharedApplication(marker);
    application.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    application.setMainMenu(Some(&main_menu(marker)));
    let delegate = AppDelegate::new(marker, cache);
    application.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    application.activate();
    application.run();
    Ok(())
}
