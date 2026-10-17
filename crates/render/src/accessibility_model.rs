use domovoi_theme::format_size;

use crate::accessible_segment::AccessibleSegment;
use crate::diagram_layout::DiagramLayout;
use crate::diagram_strings::DiagramStrings;
use crate::diagram_tree::DiagramTree;
use crate::rect::Rect;
use crate::sunburst_layout::SunburstLayout;
use crate::tile_role::TileRole;

const ARC_SAMPLES: usize = 16;

pub fn accessible_segments(
    tree: &DiagramTree,
    layout: &DiagramLayout,
    strings: &DiagramStrings,
) -> Vec<AccessibleSegment> {
    let label = |node: usize| {
        let record = tree.node(node);
        format!(
            "{}, {}, {}",
            record.name,
            format_size(record.size),
            strings.category_name(record.category)
        )
    };
    match layout {
        DiagramLayout::Sunburst(sunburst) => {
            let mut segments: Vec<AccessibleSegment> = sunburst
                .segments
                .iter()
                .filter(|segment| segment.ring == 0)
                .map(|segment| AccessibleSegment {
                    node: segment.node,
                    label: label(segment.node),
                    frame: arc_frame(
                        sunburst,
                        segment.start_angle,
                        segment.end_angle,
                        segment.inner_radius,
                        segment.outer_radius,
                    ),
                    is_center: false,
                })
                .collect();
            let center = sunburst.center();
            let radius = sunburst.center_radius();
            segments.push(AccessibleSegment {
                node: sunburst.focus,
                label: label(sunburst.focus),
                frame: Rect {
                    x: center.x - radius,
                    y: center.y - radius,
                    width: 2.0 * radius,
                    height: 2.0 * radius,
                },
                is_center: true,
            });
            segments
        }
        DiagramLayout::Treemap(treemap) => treemap
            .tiles
            .iter()
            .filter(|tile| tile.role != TileRole::Nested)
            .map(|tile| AccessibleSegment {
                node: tile.node,
                label: label(tile.node),
                frame: tile.rect,
                is_center: false,
            })
            .collect(),
    }
}

fn arc_frame(layout: &SunburstLayout, start: f32, end: f32, inner: f32, outer: f32) -> Rect {
    let mut points = Vec::with_capacity(2 * (ARC_SAMPLES + 1));
    for step in 0..=ARC_SAMPLES {
        let angle = start + (end - start) * step as f32 / ARC_SAMPLES as f32;
        points.push(layout.point_at(angle, inner));
        points.push(layout.point_at(angle, outer));
    }
    let left = points.iter().map(|point| point.x).fold(f32::INFINITY, f32::min);
    let right = points.iter().map(|point| point.x).fold(f32::NEG_INFINITY, f32::max);
    let top = points.iter().map(|point| point.y).fold(f32::INFINITY, f32::min);
    let bottom = points.iter().map(|point| point.y).fold(f32::NEG_INFINITY, f32::max);
    Rect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    }
}
