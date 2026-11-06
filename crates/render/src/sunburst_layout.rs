use std::f32::consts::TAU;

use crate::arc_segment::ArcSegment;
use crate::diagram_tree::DiagramTree;
use crate::effective_sizes::EffectiveSizes;
use crate::hit::Hit;
use crate::marked_nodes::MarkedNodes;
use crate::point::Point;
use crate::segment_kind::SegmentKind;
use crate::small_groups::SmallGroups;

pub const RING_COUNT: u32 = 4;
pub const SMALL_GROUP_SPAN: f32 = 0.006;
const INNER_SHARE: f32 = 0.19;
const OUTER_MARGIN: f32 = 4.0;
const RING_GAP: f32 = 1.5;
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

struct SmallRun {
    start: f32,
    count: usize,
    bytes: u64,
}

impl SunburstLayout {
    pub fn compute(
        tree: &DiagramTree,
        focus: usize,
        size: f32,
        sweep: f32,
        groups: &SmallGroups,
        marked: &MarkedNodes,
    ) -> Self {
        let mut layout = Self {
            size,
            focus,
            sweep: sweep.clamp(0.0, 1.0),
            segments: Vec::new(),
        };
        let sizes = EffectiveSizes::of(tree, marked, focus);
        layout.walk(tree, &sizes, focus, (0.0, TAU), 0, groups);
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

    pub fn collapsed(&self) -> impl Iterator<Item = &ArcSegment> {
        self.segments
            .iter()
            .filter(|segment| segment.kind == SegmentKind::Collapsed)
    }

    pub fn groups(&self) -> impl Iterator<Item = &ArcSegment> {
        self.segments
            .iter()
            .filter(|segment| matches!(segment.kind, SegmentKind::SmallGroup { .. }))
    }

    pub fn hit_test(&self, point: Point) -> Option<Hit> {
        let center = self.center();
        let (dx, dy) = (point.x - center.x, point.y - center.y);
        let radius = dx.hypot(dy);
        let angle = dx.atan2(-dy).rem_euclid(TAU);
        self.segments
            .iter()
            .find(|segment| {
                segment.kind != SegmentKind::Collapsed
                    && radius >= segment.inner_radius
                    && radius <= segment.outer_radius
                    && angle >= segment.start_angle
                    && angle < segment.end_angle
            })
            .map(ArcSegment::hit)
    }

    pub fn point_at(&self, angle: f32, radius: f32) -> Point {
        let center = self.center();
        Point {
            x: center.x + radius * angle.sin(),
            y: center.y - radius * angle.cos(),
        }
    }

    fn walk(
        &mut self,
        tree: &DiagramTree,
        sizes: &EffectiveSizes,
        node: usize,
        (start, end): (f32, f32),
        ring: u32,
        groups: &SmallGroups,
    ) {
        let parent_size = sizes.size(node);
        if parent_size == 0 {
            return;
        }
        let limit = self.sweep * TAU;
        let opened = groups.is_open(node);
        let mut angle = start;
        let mut small: Option<SmallRun> = None;
        for child in tree.node(node).children.clone() {
            let child_size = sizes.size(child);
            let span = (end - start) * (child_size as f32 / parent_size as f32);
            let child_end = angle + span;
            if sizes.is_collapsed(child) {
                if angle < limit {
                    self.segments
                        .push(self.segment(child, SegmentKind::Collapsed, ring, angle, angle, 0.0));
                }
            } else if span > 0.0 && angle < limit {
                if span > SMALL_GROUP_SPAN || opened {
                    self.segments
                        .push(self.segment(child, SegmentKind::Node, ring, angle, child_end.min(limit), span));
                    if ring + 1 < RING_COUNT {
                        self.walk(tree, sizes, child, (angle, child_end), ring + 1, groups);
                    }
                } else {
                    let run = small.get_or_insert(SmallRun {
                        start: angle,
                        count: 0,
                        bytes: 0,
                    });
                    run.count += 1;
                    run.bytes += child_size;
                }
            }
            angle = child_end;
        }
        if let Some(run) = small {
            let kind = SegmentKind::SmallGroup {
                count: run.count,
                bytes: run.bytes,
            };
            let group_end = angle.min(limit);
            self.segments
                .push(self.segment(node, kind, ring, run.start, group_end, group_end - run.start));
        }
    }

    fn segment(&self, node: usize, kind: SegmentKind, ring: u32, start: f32, end: f32, span: f32) -> ArcSegment {
        let radius = self.inner_radius() + ring as f32 * self.ring_width();
        ArcSegment {
            node,
            kind,
            ring,
            start_angle: start,
            end_angle: end,
            pad: if span > WIDE_SPAN { WIDE_PAD } else { NARROW_PAD },
            inner_radius: radius + RING_GAP,
            outer_radius: radius + self.ring_width() - RING_GAP,
        }
    }
}
