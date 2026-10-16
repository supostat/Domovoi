use domovoi_theme::{FONT_STACKS, Palette, RADII, Radii, Rgba, palette_for};
use objc2_app_kit::{
    NSAppearance, NSAppearanceName, NSAppearanceNameAccessibilityHighContrastDarkAqua, NSAppearanceNameAqua,
    NSAppearanceNameDarkAqua, NSAppearanceNameVibrantDark,
};

const CHANNEL_TOLERANCE: f32 = 1e-6;

fn srgb_bytes_and_alpha(color: Rgba) -> ([u8; 3], f32) {
    let [red, green, blue, alpha] = color.straight();
    ([red, green, blue].map(|channel| (channel * 255.0).round() as u8), alpha)
}

fn appearance(name: &NSAppearanceName) -> objc2::rc::Retained<NSAppearance> {
    NSAppearance::appearanceNamed(name).expect("system appearance exists")
}

#[test]
fn wax_matches_the_book_in_both_themes() {
    assert_eq!(srgb_bytes_and_alpha(Palette::DARK.wax), ([0xE3, 0xAE, 0x52], 1.0));
    assert_eq!(srgb_bytes_and_alpha(Palette::LIGHT.wax), ([0xA6, 0x6C, 0x10], 1.0));
}

#[test]
fn model_category_matches_the_book_in_both_themes() {
    assert_eq!(srgb_bytes_and_alpha(Palette::DARK.c_model), ([0xE3, 0xAE, 0x52], 1.0));
    assert_eq!(srgb_bytes_and_alpha(Palette::LIGHT.c_model), ([0xB0, 0x77, 0x14], 1.0));
}

#[test]
fn soft_tokens_are_premultiplied_by_their_alpha() {
    let [red, green, blue, alpha] = Palette::DARK.wax_soft.premultiplied();
    let expected = [0xE3_u8, 0xAE, 0x52].map(|byte| f32::from(byte) / 255.0 * 0.14);
    for (channel, expected) in [red, green, blue].into_iter().zip(expected) {
        assert!(
            (channel - expected).abs() < CHANNEL_TOLERANCE,
            "{channel} != {expected}"
        );
    }
    assert!((alpha - 0.14).abs() < CHANNEL_TOLERANCE);
    assert_eq!(
        srgb_bytes_and_alpha(Palette::LIGHT.risk_soft),
        ([0xBE, 0x43, 0x38], 0.12)
    );
    assert_eq!(
        srgb_bytes_and_alpha(Palette::DARK.shadow.color),
        ([0x00, 0x00, 0x00], 0.5)
    );
}

#[test]
fn colors_are_four_packed_floats_for_the_gpu() {
    assert_eq!(size_of::<Rgba>(), 16);
    let bytes = bytemuck::bytes_of(&Palette::DARK.bg);
    let first_channel = f32::from_ne_bytes(bytes[..4].try_into().expect("four bytes"));
    assert!((first_channel - f32::from(0x11_u8) / 255.0).abs() < CHANNEL_TOLERANCE);
}

#[test]
fn ns_color_carries_straight_srgb_components() {
    let color = Palette::DARK.wax_soft.ns_color();
    assert!((color.redComponent() - f64::from(0xE3_u8) / 255.0).abs() < 1e-6);
    assert!((color.alphaComponent() - 0.14).abs() < 1e-6);
}

#[test]
fn shared_tokens_match_the_book() {
    assert_eq!(
        RADII,
        Radii {
            s: 6.0,
            m: 10.0,
            l: 14.0
        }
    );
    assert_eq!(FONT_STACKS.ui.first(), Some(&"Onest"));
    assert_eq!(FONT_STACKS.mono.first(), Some(&"JetBrains Mono"));
    assert_eq!(FONT_STACKS.display.first(), Some(&"Ruslan Display"));
}

#[test]
fn palette_follows_the_appearance_family() {
    let light = unsafe { NSAppearanceNameAqua };
    let dark_names = unsafe {
        [
            NSAppearanceNameDarkAqua,
            NSAppearanceNameVibrantDark,
            NSAppearanceNameAccessibilityHighContrastDarkAqua,
        ]
    };
    assert_eq!(palette_for(&appearance(light)), &Palette::LIGHT);
    for name in dark_names {
        assert_eq!(palette_for(&appearance(name)), &Palette::DARK, "{name}");
    }
}
