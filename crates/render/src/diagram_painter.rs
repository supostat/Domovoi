use domovoi_index::Category;
use domovoi_theme::{Palette, format_size};

use crate::arc_segment::ArcSegment;
use crate::category_color::{category_color, faded};
use crate::center_role::CenterRole;
use crate::center_text::center_text;
use crate::diagram_strings::DiagramStrings;
use crate::diagram_tree::DiagramTree;
use crate::glyph_atlas::{ATLAS_SIDE, GlyphAtlas};
use crate::glyph_instance::GlyphInstance;
use crate::scene::Scene;
use crate::shape_instance::{SHAPE_ARC, SHAPE_DISC, SHAPE_RECT, ShapeInstance};
use crate::shaped_line::ShapedLine;
use crate::sunburst_layout::SunburstLayout;
use crate::text_style::{FontFamily, TextStyle};
use crate::tile_role::TileRole;
use crate::transition::HOVER_DIM_OPACITY;
use crate::treemap_layout::TreemapLayout;

const RING_FADE: f32 = 0.16;
const GROUP_FILL: f32 = 0.15;
const GROUP_STROKE: f32 = 0.55;
const TILE_FILL: f32 = 0.82;
const SIZE_OPACITY: f32 = 0.8;
const OUTER_CORNER: f32 = 5.0;
const NESTED_CORNER: f32 = 3.0;
const HAIRLINE: f32 = 1.0;
const SIZE_LINE_OFFSET: f32 = 14.0;
const SEPARATOR: &str = " · ";
const LABEL: TextStyle = TextStyle {
    family: FontFamily::Interface,
    weight: 600.0,
    size: 11.5,
};
const SIZE_LABEL: TextStyle = TextStyle {
    family: FontFamily::Monospace,
    weight: 400.0,
    size: 10.5,
};

#[derive(Debug)]
pub struct DiagramPainter<'a> {
    pub tree: &'a DiagramTree,
    pub palette: &'a Palette,
    pub strings: &'a DiagramStrings,
    pub scale: f32,
    pub hovered: Option<usize>,
    pub dim_progress: f32,
    pub center: Option<&'a [String; 3]>,
}

impl DiagramPainter<'_> {
    pub fn sunburst(
        &self,
        layout: &SunburstLayout,
        previous: Option<&SunburstLayout>,
        progress: f32,
        atlas: &mut GlyphAtlas,
    ) -> Scene {
        let mut scene = self.empty_scene(layout.size, layout.size, progress);
        let center = layout.center();
        for segment in &layout.segments {
            let node = self.tree.node(segment.node);
            let opacity = (1.0 - segment.ring as f32 * RING_FADE) * self.dim_factor(segment.node);
            let geometry = arc_geometry(segment);
            let previous = previous
                .and_then(|layout| layout.segments.iter().find(|candidate| candidate.node == segment.node))
                .map_or([geometry[0], geometry[0], geometry[2], geometry[3]], arc_geometry);
            scene.shapes.push(ShapeInstance {
                geometry,
                previous,
                fill: faded(category_color(self.palette, node.category), opacity),
                stroke: self.hidden_stroke(node.category),
                center: [center.x, center.y],
                stroke_width: HAIRLINE,
                kind: SHAPE_ARC,
                dashed: u32::from(node.category == Category::Hidden),
                ..ShapeInstance::default()
            });
        }
        let disc = [0.0, 0.0, 0.0, layout.center_radius()];
        scene.shapes.push(ShapeInstance {
            geometry: disc,
            previous: disc,
            fill: self.palette.surface_2.premultiplied(),
            stroke: self.palette.line.premultiplied(),
            center: [center.x, center.y],
            stroke_width: HAIRLINE,
            kind: SHAPE_DISC,
            ..ShapeInstance::default()
        });
        let shown = self.hovered.unwrap_or(layout.focus);
        let node = self.tree.node(shown);
        let caption = if self.hovered.is_some() {
            self.strings.category_name(node.category)
        } else if node.parent.is_some() {
            self.strings.up
        } else {
            self.strings.used
        };
        let size = format_size(node.size);
        let [caption, name, size] = match self.center {
            Some([caption, name, size]) => [caption.as_str(), name.as_str(), size.as_str()],
            None => [caption, node.name.as_str(), size.as_str()],
        };
        for line in center_text(layout, caption, name, size) {
            let color = match line.role {
                CenterRole::Caption => self.palette.dim,
                CenterRole::Name => self.palette.text,
                CenterRole::Size => self.palette.wax,
            };
            let left = center.x - line.line.width() / 2.0;
            self.paint_line(
                &mut scene,
                atlas,
                &line.line,
                left,
                line.baseline,
                color.premultiplied(),
            );
        }
        scene
    }

    pub fn treemap(&self, layout: &TreemapLayout, atlas: &mut GlyphAtlas) -> Scene {
        let mut scene = self.empty_scene(layout.width, layout.height, 1.0);
        for tile in &layout.tiles {
            let node = self.tree.node(tile.node);
            let color = category_color(self.palette, node.category);
            let dim = self.dim_factor(tile.node);
            let geometry = [tile.rect.x, tile.rect.y, tile.rect.width, tile.rect.height];
            let (fill, stroke, corner) = match tile.role {
                TileRole::Group => (
                    faded(color, GROUP_FILL * dim),
                    faded(color, GROUP_STROKE * dim),
                    OUTER_CORNER,
                ),
                TileRole::Leaf => (
                    faded(color, TILE_FILL * dim),
                    self.hidden_stroke(node.category),
                    OUTER_CORNER,
                ),
                TileRole::Nested => (
                    faded(color, TILE_FILL * dim),
                    self.hidden_stroke(node.category),
                    NESTED_CORNER,
                ),
            };
            scene.shapes.push(ShapeInstance {
                geometry,
                previous: geometry,
                fill,
                stroke,
                corner,
                stroke_width: HAIRLINE,
                kind: SHAPE_RECT,
                dashed: u32::from(tile.role != TileRole::Group && node.category == Category::Hidden),
                ..ShapeInstance::default()
            });
        }
        for tile in &layout.tiles {
            let Some(label) = tile.label else {
                continue;
            };
            let node = self.tree.node(tile.node);
            let size = format_size(node.size);
            if label.heading {
                let text = format!("{}{SEPARATOR}{size}", node.name);
                let line = fit_end(&text, LABEL, label.max_width);
                self.paint_line(
                    &mut scene,
                    atlas,
                    &line,
                    label.origin.x,
                    label.origin.y,
                    self.palette.text.premultiplied(),
                );
            } else {
                let on_category = self.palette.on_cat;
                let name = fit_end(&node.name, LABEL, label.max_width);
                self.paint_line(
                    &mut scene,
                    atlas,
                    &name,
                    label.origin.x,
                    label.origin.y,
                    on_category.premultiplied(),
                );
                let size_line = fit_end(&size, SIZE_LABEL, label.max_width);
                self.paint_line(
                    &mut scene,
                    atlas,
                    &size_line,
                    label.origin.x,
                    label.origin.y + SIZE_LINE_OFFSET,
                    faded(on_category, SIZE_OPACITY),
                );
            }
        }
        scene
    }

    fn empty_scene(&self, width: f32, height: f32, progress: f32) -> Scene {
        Scene {
            width,
            height,
            scale: self.scale,
            progress,
            clear: self.palette.surface.premultiplied(),
            shapes: Vec::new(),
            glyphs: Vec::new(),
        }
    }

    fn dim_factor(&self, node: usize) -> f32 {
        match self.hovered {
            Some(hovered) if hovered != node => 1.0 - (1.0 - HOVER_DIM_OPACITY) * self.dim_progress,
            _ => 1.0,
        }
    }

    fn hidden_stroke(&self, category: Category) -> [f32; 4] {
        if category == Category::Hidden {
            self.palette.dim.premultiplied()
        } else {
            [0.0; 4]
        }
    }

    pub fn paint_line(
        &self,
        scene: &mut Scene,
        atlas: &mut GlyphAtlas,
        line: &ShapedLine,
        left: f32,
        baseline: f32,
        color: [f32; 4],
    ) {
        let atlas_side = ATLAS_SIDE as f32;
        let baseline_pixels = (baseline * self.scale).round();
        for glyph in line.glyphs() {
            let (whole, entry) = atlas.place(&glyph, (left + glyph.x) * self.scale, self.scale);
            let Some(entry) = entry else {
                continue;
            };
            let x = (whole + entry.left) as f32;
            let y = baseline_pixels - (glyph.y * self.scale).round() + entry.top as f32;
            scene.glyphs.push(GlyphInstance {
                rect: [
                    x / self.scale,
                    y / self.scale,
                    entry.width as f32 / self.scale,
                    entry.height as f32 / self.scale,
                ],
                texture: [
                    entry.x as f32 / atlas_side,
                    entry.y as f32 / atlas_side,
                    (entry.x + entry.width) as f32 / atlas_side,
                    (entry.y + entry.height) as f32 / atlas_side,
                ],
                color,
            });
        }
    }
}

pub fn fit_end(text: &str, style: TextStyle, max_width: f32) -> ShapedLine {
    let line = ShapedLine::shape(text, style);
    if line.width() > max_width {
        line.truncated_end(max_width)
    } else {
        line
    }
}

fn arc_geometry(segment: &ArcSegment) -> [f32; 4] {
    let (start, end) = segment.drawn_angles();
    [start, end, segment.inner_radius, segment.outer_radius]
}
