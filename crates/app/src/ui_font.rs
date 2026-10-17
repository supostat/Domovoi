use domovoi_theme::FONT_STACKS;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSFont, NSFontDescriptor, NSFontFamilyAttribute, NSFontVariationAttribute};
use objc2_foundation::{NSDictionary, NSNumber, NSString};

const WEIGHT_AXIS: i32 = i32::from_be_bytes(*b"wght");

pub(crate) fn interface_font(size: f64, weight: f64) -> Retained<NSFont> {
    font(FONT_STACKS.ui[0], size, weight)
}

pub(crate) fn mono_font(size: f64, weight: f64) -> Retained<NSFont> {
    font(FONT_STACKS.mono[0], size, weight)
}

fn font(family: &str, size: f64, weight: f64) -> Retained<NSFont> {
    let axis = NSNumber::new_i32(WEIGHT_AXIS);
    let value = NSNumber::new_f64(weight);
    let variation = NSDictionary::from_slices(&[&*axis], &[&*value]);
    let family_name = NSString::from_str(family);
    let keys = unsafe { [NSFontFamilyAttribute, NSFontVariationAttribute] };
    let values: [&AnyObject; 2] = [family_name.as_ref(), variation.as_ref()];
    let attributes = NSDictionary::from_slices(&keys, &values);
    let descriptor = unsafe { NSFontDescriptor::fontDescriptorWithFontAttributes(Some(&attributes)) };
    NSFont::fontWithDescriptor_size(&descriptor, size).unwrap_or_else(|| NSFont::systemFontOfSize(size))
}
