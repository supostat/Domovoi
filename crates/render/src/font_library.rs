use std::path::Path;
use std::ptr;
use std::sync::OnceLock;

use domovoi_theme::FONT_STACKS;
use objc2_core_foundation::{CFDictionary, CFNumber, CFRetained, CFString, CFType, CFURL};
use objc2_core_text::{
    CTFont, CTFontDescriptor, CTFontManagerRegisterFontsForURL, CTFontManagerScope, kCTFontFamilyNameAttribute,
    kCTFontVariationAttribute,
};

use crate::text_style::{FontFamily, TextStyle};

const FONT_FILES: [&str; 2] = ["Onest/Onest[wght].ttf", "JetBrainsMono/JetBrainsMono[wght].ttf"];
const WEIGHT_AXIS: i32 = i32::from_be_bytes(*b"wght");
static REGISTRATION: OnceLock<Result<(), String>> = OnceLock::new();

pub fn register_fonts(directory: &Path) -> Result<(), String> {
    REGISTRATION
        .get_or_init(|| {
            for file in FONT_FILES {
                let path = directory.join(file);
                let url =
                    CFURL::from_file_path(&path).ok_or_else(|| format!("{} is not a file URL", path.display()))?;
                let registered =
                    unsafe { CTFontManagerRegisterFontsForURL(&url, CTFontManagerScope::Process, ptr::null_mut()) };
                if !registered {
                    return Err(format!("Core Text refused the font {}", path.display()));
                }
            }
            Ok(())
        })
        .clone()
}

pub(crate) fn font_for(style: TextStyle) -> CFRetained<CTFont> {
    let family = match style.family {
        FontFamily::Interface => FONT_STACKS.ui[0],
        FontFamily::Monospace => FONT_STACKS.mono[0],
    };
    let family_name = CFString::from_str(family);
    let axis = CFNumber::new_i32(WEIGHT_AXIS);
    let weight = CFNumber::new_f64(f64::from(style.weight));
    let variation = CFDictionary::<CFNumber, CFNumber>::from_slices(&[&*axis], &[&*weight]);
    let keys: [&CFString; 2] = unsafe { [kCTFontFamilyNameAttribute, kCTFontVariationAttribute] };
    let values: [&CFType; 2] = [family_name.as_ref(), variation.as_ref()];
    let attributes = CFDictionary::<CFString, CFType>::from_slices(&keys, &values);
    unsafe {
        let descriptor = CTFontDescriptor::with_attributes(attributes.as_opaque());
        CTFont::with_font_descriptor(&descriptor, f64::from(style.size), ptr::null())
    }
}
