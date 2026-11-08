#![deny(unsafe_code)]

mod age;
mod appearance;
mod civil_date;
mod count;
mod digits;
#[allow(unsafe_code)]
mod ffi;
#[rustfmt::skip]
mod generated;
mod iso_date;
mod memory;
mod motion;
mod plural;
mod reduce_motion;
mod rgba;
mod shadow;
mod size;

pub use age::{DateSource, format_age, format_elapsed};
pub use appearance::palette_for;
pub use count::format_count;
pub use digits::{NO_BREAK_SPACE, group_thousands};
pub use generated::{FONT_STACKS, FontStacks, Palette, RADII, Radii};
pub use iso_date::format_iso_utc;
pub use memory::format_memory;
pub use motion::Motion;
pub use plural::PluralForms;
pub use reduce_motion::{ReduceMotionObservation, observe_reduce_motion, reduce_motion};
pub use rgba::Rgba;
pub use shadow::Shadow;
pub use size::format_size;
