use crate::rect::Rect;
use crate::tile_label::TileLabel;
use crate::tile_role::TileRole;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreemapTile {
    pub node: usize,
    pub rect: Rect,
    pub role: TileRole,
    pub label: Option<TileLabel>,
}
