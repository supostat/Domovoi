use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{NSMenu, NSMenuItem};
use objc2_foundation::NSString;

use crate::strings::{
    ALWAYS_EXPAND_SMALL, APP_NAME, CHECK_FOR_UPDATES, CLOSE_WINDOW, HIDE_APP, MINIMIZE, QUIT_APP, RESCAN_DISK,
    VIEW_MENU, WINDOW_MENU,
};

pub fn main_menu(marker: MainThreadMarker) -> Retained<NSMenu> {
    let bar = NSMenu::new(marker);
    let application = submenu(
        marker,
        APP_NAME,
        &[
            (CHECK_FOR_UPDATES, sel!(checkForUpdates:), ""),
            (RESCAN_DISK, sel!(rescanDisk:), "r"),
            (HIDE_APP, sel!(hide:), "h"),
            (QUIT_APP, sel!(terminate:), "q"),
        ],
    );
    let view = submenu(
        marker,
        VIEW_MENU,
        &[(ALWAYS_EXPAND_SMALL, sel!(toggleSmallGroups:), "")],
    );
    let window = submenu(
        marker,
        WINDOW_MENU,
        &[
            (MINIMIZE, sel!(performMiniaturize:), "m"),
            (CLOSE_WINDOW, sel!(performClose:), "w"),
        ],
    );
    bar.addItem(&application);
    bar.addItem(&view);
    bar.addItem(&window);
    bar
}

fn submenu(marker: MainThreadMarker, title: &str, entries: &[(&str, Sel, &str)]) -> Retained<NSMenuItem> {
    let menu = NSMenu::initWithTitle(NSMenu::alloc(marker), &NSString::from_str(title));
    for (entry, action, key) in entries {
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(marker),
                &NSString::from_str(entry),
                Some(*action),
                &NSString::from_str(key),
            )
        };
        menu.addItem(&item);
    }
    let holder = NSMenuItem::new(marker);
    holder.setSubmenu(Some(&menu));
    holder
}
