use crate::shaped_line::ShapedLine;
use crate::text_style::TextStyle;

const SHRINK_ATTEMPTS: usize = 4;

pub fn fit_to_width(text: &str, style: TextStyle, minimum_size: f32, max_width: f32) -> ShapedLine {
    let mut line = ShapedLine::shape(text, style);
    for _ in 0..SHRINK_ATTEMPTS {
        let width = line.width();
        if width <= max_width || line.style.size <= minimum_size {
            break;
        }
        let size = (line.style.size * max_width / width).max(minimum_size);
        line = ShapedLine::shape(text, style.sized(size));
    }
    if line.width() > max_width && line.style.size > minimum_size {
        line = ShapedLine::shape(text, style.sized(minimum_size));
    }
    if line.width() > max_width {
        line = line.truncated_end(max_width);
    }
    line
}
