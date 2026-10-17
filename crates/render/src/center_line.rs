use crate::center_role::CenterRole;
use crate::shaped_line::ShapedLine;

#[derive(Debug)]
pub struct CenterLine {
    pub role: CenterRole,
    pub line: ShapedLine,
    pub baseline: f32,
    pub max_width: f32,
    pub minimum_size: f32,
}
