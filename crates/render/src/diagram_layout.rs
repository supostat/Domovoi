use crate::diagram_mode::DiagramMode;
use crate::diagram_tree::DiagramTree;
use crate::hit::Hit;
use crate::marked_nodes::MarkedNodes;
use crate::point::Point;
use crate::rect::Rect;
use crate::small_groups::SmallGroups;
use crate::sunburst_layout::SunburstLayout;
use crate::treemap_layout::TreemapLayout;

#[derive(Clone, Debug, PartialEq)]
pub enum DiagramLayout {
    Sunburst(SunburstLayout),
    Treemap(TreemapLayout),
}

impl DiagramLayout {
    pub fn compute(
        mode: DiagramMode,
        tree: &DiagramTree,
        focus: usize,
        bounds: Rect,
        sweep: f32,
        groups: &SmallGroups,
        marked: &MarkedNodes,
    ) -> Self {
        match mode {
            DiagramMode::Sunburst => Self::Sunburst(SunburstLayout::compute(
                tree,
                focus,
                bounds.width.min(bounds.height),
                sweep,
                groups,
                marked,
            )),
            DiagramMode::Treemap => {
                Self::Treemap(TreemapLayout::compute(tree, focus, bounds.width, bounds.height, marked))
            }
        }
    }

    pub fn focus(&self) -> usize {
        match self {
            Self::Sunburst(layout) => layout.focus,
            Self::Treemap(layout) => layout.focus,
        }
    }

    pub fn hit_test(&self, point: Point) -> Option<Hit> {
        match self {
            Self::Sunburst(layout) => layout.hit_test(point),
            Self::Treemap(layout) => layout.hit_test(point).map(Hit::Node),
        }
    }

    pub fn in_center(&self, point: Point) -> bool {
        match self {
            Self::Sunburst(layout) => {
                let center = layout.center();
                (point.x - center.x).hypot(point.y - center.y) <= layout.center_radius()
            }
            Self::Treemap(_) => false,
        }
    }
}
