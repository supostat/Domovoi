use crate::diagram_tree::DiagramTree;
use crate::point::Point;
use crate::rect::Rect;
use crate::squarify::squarify;
use crate::tile_label::TileLabel;
use crate::tile_role::TileRole;
use crate::treemap_tile::TreemapTile;

pub const LABEL_MIN_WIDTH: f32 = 58.0;
pub const LABEL_MIN_HEIGHT: f32 = 34.0;
pub const GROUP_MIN_WIDTH: f32 = 50.0;
pub const GROUP_MIN_HEIGHT: f32 = 46.0;
pub const VISIBLE_SIDE: f32 = 2.0;
const TILE_INSET: f32 = 2.0;
const NESTED_INSET: f32 = 1.0;
const GROUP_PADDING: f32 = 3.0;
const GROUP_HEADER: f32 = 21.0;

#[derive(Clone, Debug, PartialEq)]
pub struct TreemapLayout {
    pub width: f32,
    pub height: f32,
    pub focus: usize,
    pub tiles: Vec<TreemapTile>,
}

impl TreemapLayout {
    pub fn compute(tree: &DiagramTree, focus: usize, width: f32, height: f32) -> Self {
        let mut tiles = Vec::new();
        let focus_node = tree.node(focus);
        let top: Vec<usize> = if focus_node.children.is_empty() {
            vec![focus]
        } else {
            focus_node.children.clone()
        };
        let bounds = Rect {
            x: 0.0,
            y: 0.0,
            width,
            height,
        };
        for (node, cell) in squarify(&weights(tree, &top), bounds) {
            let rect = cell.inset(TILE_INSET);
            if rect.width < VISIBLE_SIDE || rect.height < VISIBLE_SIDE {
                continue;
            }
            let children = &tree.node(node).children;
            if !children.is_empty() && rect.width > GROUP_MIN_WIDTH && rect.height > GROUP_MIN_HEIGHT {
                tiles.push(TreemapTile {
                    node,
                    rect,
                    role: TileRole::Group,
                    label: Some(TileLabel {
                        origin: Point {
                            x: rect.x + 7.0,
                            y: rect.y + 14.0,
                        },
                        max_width: rect.width - 14.0,
                        heading: true,
                    }),
                });
                let inner = Rect {
                    x: rect.x + GROUP_PADDING,
                    y: rect.y + GROUP_HEADER,
                    width: rect.width - 2.0 * GROUP_PADDING,
                    height: rect.height - GROUP_HEADER - GROUP_PADDING,
                };
                for (child, cell) in squarify(&weights(tree, children), inner) {
                    let rect = cell.inset(NESTED_INSET);
                    if rect.width < 1.0 || rect.height < 1.0 {
                        continue;
                    }
                    tiles.push(TreemapTile {
                        node: child,
                        rect,
                        role: TileRole::Nested,
                        label: leaf_label(rect, 6.0, 15.0, 10.0),
                    });
                }
            } else {
                tiles.push(TreemapTile {
                    node,
                    rect,
                    role: TileRole::Leaf,
                    label: leaf_label(rect, 7.0, 16.0, 12.0),
                });
            }
        }
        Self {
            width,
            height,
            focus,
            tiles,
        }
    }

    pub fn hit_test(&self, point: Point) -> Option<usize> {
        self.tiles
            .iter()
            .rev()
            .find(|tile| tile.rect.contains(point))
            .map(|tile| tile.node)
    }

    pub fn tile_of(&self, node: usize) -> Option<&TreemapTile> {
        self.tiles.iter().find(|tile| tile.node == node)
    }
}

fn weights(tree: &DiagramTree, nodes: &[usize]) -> Vec<(usize, f32)> {
    nodes
        .iter()
        .map(|node| (*node, tree.node(*node).size as f32))
        .filter(|(_, size)| *size > 0.0)
        .collect()
}

fn leaf_label(rect: Rect, inset_x: f32, baseline: f32, trailing: f32) -> Option<TileLabel> {
    (rect.width > LABEL_MIN_WIDTH && rect.height > LABEL_MIN_HEIGHT).then_some(TileLabel {
        origin: Point {
            x: rect.x + inset_x,
            y: rect.y + baseline,
        },
        max_width: rect.width - trailing,
        heading: false,
    })
}
