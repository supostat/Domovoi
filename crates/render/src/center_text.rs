use crate::center_line::CenterLine;
use crate::center_role::CenterRole;
use crate::fitted_line::fit_to_width;
use crate::sunburst_layout::SunburstLayout;
use crate::text_style::{FontFamily, TextStyle};

const CHORD_MARGIN: f32 = 0.035;
const LINES: [(CenterRole, f32, f32, f32, f32, FontFamily, f32); 3] = [
    (
        CenterRole::Caption,
        -0.05,
        0.07,
        0.03,
        0.022,
        FontFamily::Interface,
        500.0,
    ),
    (
        CenterRole::Name,
        0.008,
        0.02,
        0.042,
        0.026,
        FontFamily::Interface,
        600.0,
    ),
    (
        CenterRole::Size,
        0.06,
        0.062,
        0.036,
        0.024,
        FontFamily::Monospace,
        500.0,
    ),
];

pub fn chord_width(layout: &SunburstLayout, offset_share: f32) -> f32 {
    let radius = layout.center_radius();
    let offset = layout.size * offset_share;
    2.0 * (radius * radius - offset * offset).max(0.0).sqrt() - layout.size * CHORD_MARGIN
}

pub fn center_text(layout: &SunburstLayout, caption: &str, name: &str, size: &str) -> Vec<CenterLine> {
    let center = layout.center();
    LINES
        .iter()
        .zip([caption, name, size])
        .map(
            |((role, baseline, chord_offset, base, minimum, family, weight), text)| {
                let max_width = chord_width(layout, *chord_offset);
                let style = TextStyle {
                    family: *family,
                    weight: *weight,
                    size: layout.size * base,
                };
                let minimum_size = layout.size * minimum;
                CenterLine {
                    role: *role,
                    line: fit_to_width(text, style, minimum_size, max_width),
                    baseline: center.y + layout.size * baseline,
                    max_width,
                    minimum_size,
                }
            },
        )
        .collect()
}
