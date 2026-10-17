use crate::rect::Rect;

pub(crate) fn squarify(values: &[(usize, f32)], bounds: Rect) -> Vec<(usize, Rect)> {
    let total: f32 = values.iter().map(|(_, value)| value).sum();
    if total <= 0.0 || bounds.width <= 0.0 || bounds.height <= 0.0 {
        return Vec::new();
    }
    let scale = bounds.width * bounds.height / total;
    let mut pending: Vec<(usize, f32)> = values.iter().map(|(item, value)| (*item, value * scale)).collect();
    pending.reverse();
    let mut free = bounds;
    let mut row: Vec<(usize, f32)> = Vec::new();
    let mut placed = Vec::new();
    while let Some(candidate) = pending.last().copied() {
        let side = free.width.min(free.height);
        let mut extended = row.clone();
        extended.push(candidate);
        if row.is_empty() || worst(&extended, side) <= worst(&row, side) {
            row.push(candidate);
            pending.pop();
        } else {
            lay_row(&mut row, &mut free, &mut placed);
        }
    }
    if !row.is_empty() {
        lay_row(&mut row, &mut free, &mut placed);
    }
    placed
}

fn worst(row: &[(usize, f32)], side: f32) -> f32 {
    let sum: f32 = row.iter().map(|(_, area)| area).sum();
    let largest = row.iter().map(|(_, area)| *area).fold(0.0, f32::max);
    let smallest = row.iter().map(|(_, area)| *area).fold(f32::INFINITY, f32::min);
    (side * side * largest / (sum * sum)).max(sum * sum / (side * side * smallest))
}

fn lay_row(row: &mut Vec<(usize, f32)>, free: &mut Rect, placed: &mut Vec<(usize, Rect)>) {
    let sum: f32 = row.iter().map(|(_, area)| area).sum();
    if free.width >= free.height {
        let width = sum / free.height;
        let mut y = free.y;
        for (item, area) in row.drain(..) {
            let height = area / width;
            placed.push((
                item,
                Rect {
                    x: free.x,
                    y,
                    width,
                    height,
                },
            ));
            y += height;
        }
        free.x += width;
        free.width -= width;
    } else {
        let height = sum / free.width;
        let mut x = free.x;
        for (item, area) in row.drain(..) {
            let width = area / height;
            placed.push((
                item,
                Rect {
                    x,
                    y: free.y,
                    width,
                    height,
                },
            ));
            x += width;
        }
        free.y += height;
        free.height -= height;
    }
}
