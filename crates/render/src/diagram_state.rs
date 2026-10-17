use std::time::Instant;

use domovoi_theme::{Motion, Palette};

use crate::accessibility_model::accessible_segments;
use crate::accessible_segment::AccessibleSegment;
use crate::diagram_layout::DiagramLayout;
use crate::diagram_mode::DiagramMode;
use crate::diagram_painter::DiagramPainter;
use crate::diagram_strings::DiagramStrings;
use crate::diagram_tree::{DiagramTree, ROOT};
use crate::glyph_atlas::GlyphAtlas;
use crate::point::Point;
use crate::scene::Scene;
use crate::segment_focus::SegmentFocus;
use crate::sunburst_layout::SunburstLayout;
use crate::transition::Transition;

const FULL_SWEEP: f32 = 1.0;

#[derive(Debug)]
pub struct DiagramState {
    pub tree: DiagramTree,
    pub mode: DiagramMode,
    pub strings: DiagramStrings,
    pub palette: &'static Palette,
    pub width: f32,
    pub height: f32,
    pub reduce_motion: bool,
    pub sweep: f32,
    pub hovered: Option<usize>,
    pub layout: DiagramLayout,
    previous: Option<SunburstLayout>,
    zoom: Option<Transition>,
    dim: Option<Transition>,
    keyboard: SegmentFocus,
}

impl DiagramState {
    pub fn new(
        tree: DiagramTree,
        mode: DiagramMode,
        strings: DiagramStrings,
        palette: &'static Palette,
        width: f32,
        height: f32,
    ) -> Self {
        let layout = DiagramLayout::compute(mode, &tree, ROOT, width, height, FULL_SWEEP);
        Self {
            tree,
            mode,
            strings,
            palette,
            width,
            height,
            reduce_motion: false,
            sweep: FULL_SWEEP,
            hovered: None,
            layout,
            previous: None,
            zoom: None,
            dim: None,
            keyboard: SegmentFocus::default(),
        }
    }

    pub fn focus(&self) -> usize {
        self.layout.focus()
    }

    pub fn resize(&mut self, width: f32, height: f32) {
        self.width = width;
        self.height = height;
        self.relayout(self.focus());
    }

    pub fn set_mode(&mut self, mode: DiagramMode) {
        self.mode = mode;
        self.previous = None;
        self.zoom = None;
        self.relayout(self.focus());
    }

    pub fn set_sweep(&mut self, sweep: f32) {
        self.sweep = sweep;
        self.relayout(self.focus());
    }

    pub fn set_focus(&mut self, node: usize, now: Instant) {
        if node == self.focus() {
            return;
        }
        self.previous = match &self.layout {
            DiagramLayout::Sunburst(layout) => Some(layout.clone()),
            DiagramLayout::Treemap(_) => None,
        };
        self.zoom = Some(Transition::start(Motion::LevelZoom, now, self.reduce_motion));
        self.hovered = None;
        self.keyboard.reset();
        self.relayout(node);
    }

    pub fn hover(&mut self, point: Option<Point>, now: Instant) {
        let hovered = point.and_then(|point| self.layout.hit_test(point));
        if hovered != self.hovered {
            self.hovered = hovered;
            self.dim = hovered.map(|_| Transition::start(Motion::HoverDim, now, self.reduce_motion));
        }
    }

    pub fn click(&mut self, point: Point, now: Instant) {
        if self.layout.in_center(point) {
            if let Some(parent) = self.tree.node(self.focus()).parent {
                self.set_focus(parent, now);
            }
            return;
        }
        if let Some(node) = self.layout.hit_test(point) {
            let target = if self.tree.node(node).children.is_empty() {
                self.tree.node(node).parent.filter(|parent| *parent != self.focus())
            } else {
                Some(node)
            };
            if let Some(target) = target {
                self.set_focus(target, now);
            }
        }
    }

    pub fn segments(&self) -> Vec<AccessibleSegment> {
        accessible_segments(&self.tree, &self.layout, &self.strings)
    }

    pub fn keyboard_focus(&self) -> Option<AccessibleSegment> {
        let segments = self.segments();
        self.keyboard
            .position()
            .and_then(|position| segments.get(position).cloned())
    }

    pub fn move_keyboard_focus(&mut self, forward: bool, now: Instant) -> Option<AccessibleSegment> {
        let segments = self.segments();
        let position = self.keyboard.step(forward, segments.len())?;
        let segment = segments[position].clone();
        self.hovered = (!segment.is_center).then_some(segment.node);
        self.dim = self
            .hovered
            .map(|_| Transition::start(Motion::HoverDim, now, self.reduce_motion));
        Some(segment)
    }

    pub fn open_keyboard_focus(&mut self, now: Instant) {
        if let Some(segment) = self.keyboard_focus() {
            let target = if segment.is_center {
                self.tree.node(segment.node).parent
            } else {
                Some(segment.node)
            };
            if let Some(target) = target.filter(|target| !self.tree.node(*target).children.is_empty()) {
                self.set_focus(target, now);
            }
        }
    }

    pub fn is_animating(&self, now: Instant) -> bool {
        [self.zoom, self.dim]
            .into_iter()
            .flatten()
            .any(|transition| !transition.is_finished(now))
    }

    pub fn scene(&self, now: Instant, scale: f32, atlas: &mut GlyphAtlas) -> Scene {
        let painter = DiagramPainter {
            tree: &self.tree,
            palette: self.palette,
            strings: &self.strings,
            scale,
            hovered: self.hovered,
            dim_progress: self.dim.map_or(0.0, |dim| dim.progress(now)),
        };
        match &self.layout {
            DiagramLayout::Sunburst(layout) => {
                let progress = self.zoom.map_or(1.0, |zoom| zoom.progress(now));
                painter.sunburst(layout, self.previous.as_ref(), progress, atlas)
            }
            DiagramLayout::Treemap(layout) => painter.treemap(layout, atlas),
        }
    }

    fn relayout(&mut self, focus: usize) {
        self.layout = DiagramLayout::compute(self.mode, &self.tree, focus, self.width, self.height, self.sweep);
    }
}
