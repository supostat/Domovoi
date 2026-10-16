use bytemuck::{Pod, Zeroable};
use objc2::rc::Retained;
use objc2_app_kit::NSColor;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Pod, Zeroable)]
pub struct Rgba {
    premultiplied: [f32; 4],
}

impl Rgba {
    pub const fn opaque(red: u8, green: u8, blue: u8) -> Self {
        Self::translucent(red, green, blue, 1.0)
    }

    pub const fn translucent(red: u8, green: u8, blue: u8, alpha: f32) -> Self {
        Self {
            premultiplied: [
                unit_channel(red) * alpha,
                unit_channel(green) * alpha,
                unit_channel(blue) * alpha,
                alpha,
            ],
        }
    }

    pub const fn premultiplied(self) -> [f32; 4] {
        self.premultiplied
    }

    pub fn straight(self) -> [f32; 4] {
        let [red, green, blue, alpha] = self.premultiplied;
        if alpha == 0.0 {
            return [0.0; 4];
        }
        [red / alpha, green / alpha, blue / alpha, alpha]
    }

    pub fn ns_color(self) -> Retained<NSColor> {
        let [red, green, blue, alpha] = self.straight().map(f64::from);
        NSColor::colorWithSRGBRed_green_blue_alpha(red, green, blue, alpha)
    }
}

const fn unit_channel(value: u8) -> f32 {
    value as f32 / 255.0
}
