use objc2_app_kit::NSAppearance;
use objc2_foundation::NSArray;

use crate::ffi;
use crate::generated::Palette;

pub fn palette_for(appearance: &NSAppearance) -> &'static Palette {
    let dark_aqua = ffi::dark_aqua_appearance_name();
    let candidates = NSArray::from_slice(&[ffi::aqua_appearance_name(), dark_aqua]);
    let best_match = appearance.bestMatchFromAppearancesWithNames(&candidates);
    if best_match.is_some_and(|name| &*name == dark_aqua) {
        &Palette::DARK
    } else {
        &Palette::LIGHT
    }
}
