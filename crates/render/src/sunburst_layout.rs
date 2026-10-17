use std::f32::consts::TAU;

use crate::arc_segment::ArcSegment;
use crate::diagram_tree::DiagramTree;
use crate::point::Point;

pub const RING_COUNT: u32 = 4;
const INNER_SHARE: f32 = 0.19;
const OUTER_MARGIN: f32 = 4.0;
const RING_GAP: f32 = 1.5;
const VISIBLE_SPAN: f32 = 0.006;
const WIDE_SPAN: f32 = 0.025;
const WIDE_PAD: f32 = 0.004;
const NARROW_PAD: f32 = 0.001;
const CENTER_INSET: f32 = 5.0;

#[derive(Clone, Debug, PartialEq)]
pub struct SunburstLayout {
    pub size: f32,
    pub focus: usize,
    pub sweep: f32,
    pub segments: Vec<ArcSegment>,
}

impl SunburstLayout {
    pub fn compute(tree: &DiagramTree, focus: usize, size: f32, sweep: f32) -> Self {
        let mut layout = Self {
            size,
            focus,
            sweep: sweep.clamp(0.0, 1.0),
            segments: Vec::new(),
        };
        layout.walk(tree, focus, 0.0, TAU, 0);
        layout
    }

    pub fn center(&self) -> Point {
        Point {
            x: self.size / 2.0,
            y: self.size / 2.0,
        }
    }

    pub fn inner_radius(&self) -> f32 {
        self.size * INNER_SHARE
    }

    pub fn ring_width(&self) -> f32 {
        (self.size / 2.0 - self.inner_radius() - OUTER_MARGIN) / RING_COUNT as f32
    }

    pub fn center_radius(&self) -> f32 {
        self.inner_radius() - CENTER_INSET
    }

    pub fn ring_angle_sum(&self, ring: u32) -> f32 {
        self.segments
            .iter()
            .filter(|segment| segment.ring == ring)
            .map(|segment| segment.end_angle - segment.start_angle)
            .sum()
    }

    pub fn hit_test(&self, point: Point) -> Option<usize> {
        let center = self.center();
        let (dx, dy) = (point.x - center.x, point.y - center.y);
        let radius = dx.hypot(dy);
        let angle = dx.atan2(-dy).rem_euclid(TAU);
        self.segments
            .iter()
            .find(|segment| {
                radius >= segment.inner_radius
                    && radius <= segment.outer_radius
                    && angle >= segment.start_angle
                    && angle < segment.end_angle
            })
            .map(|segment| segment.node)
    }

    pub fn point_at(&self, angle: f32, radius: f32) -> Point {
        let center = self.center();
        Point {
            x: center.x + radius * angle.sin(),
            y: center.y - radius * angle.cos(),
        }
    }

    fn walk(&mut self, tree: &DiagramTree, node: usize, start: f32, end: f32, ring: u32) {
        let parent_size = tree.node(node).size;
        if parent_size == 0 {
            return;
        }
        let limit = self.sweep * TAU;
        let mut angle = start;
        for child in tree.node(node).children.clone() {
            let span = (end - start) * (tree.node(child).size as f32 / parent_size as f32);
            let child_end = angle + span;
            if span > VISIBLE_SPAN && angle < limit {
                let radius = self.inner_radius() + ring as f32 * self.ring_width();
                self.segments.push(ArcSegment {
                    node: child,
                    ring,
                    start_angle: angle,
                    end_angle: child_end.min(limit),
                    pad: if span > WIDE_SPAN { WIDE_PAD } else { NARROW_PAD },
                    inner_radius: radius + RING_GAP,
                    outer_radius: radius + self.ring_width() - RING_GAP,
                });
                if ring + 1 < RING_COUNT {
                    self.walk(tree, child, angle, child_end, ring + 1);
                }
            }
            angle = child_end;
        }
    }
}
