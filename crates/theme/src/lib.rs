#![deny(unsafe_code)]

mod age;
mod appearance;
mod count;
mod digits;
#[allow(unsafe_code)]
mod ffi;
#[rustfmt::skip]
mod generated;
mod motion;
mod plural;
mod reduce_motion;
mod rgba;
mod shadow;
mod size;

pub use age::{DateSource, format_age};
pub use appearance::palette_for;
pub use count::format_count;
pub use generated::{FONT_STACKS, FontStacks, Palette, RADII, Radii};
pub use motion::Motion;
pub use plural::PluralForms;
pub use reduce_motion::{ReduceMotionObservation, observe_reduce_motion, reduce_motion};
pub use rgba::Rgba;
pub use shadow::Shadow;
pub use size::format_size;
