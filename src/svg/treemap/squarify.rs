//! Squarified tiling (Bruls/Huizing/van Wijk): pack values into a rectangle,
//! greedily extending each row while the worst aspect ratio keeps improving.

use super::Rect;

/// Squarified treemap: pack `values` into `area`, one output rect per value, in
/// input order, keeping rows near square by the worst-aspect-ratio heuristic.
pub(super) fn squarify(values: &[f64], area: Rect) -> Vec<Rect> {
    let total: f64 = values.iter().sum();
    if values.is_empty() || total <= 0.0 || area.w <= 0.0 || area.h <= 0.0 {
        return vec![area; values.len()];
    }
    // Scale values so their sum equals the rectangle's area; then row lengths
    // fall directly out of the packed sub-areas.
    let scale = (area.w * area.h) / total;
    let areas: Vec<f64> = values.iter().map(|v| v * scale).collect();

    let mut out = Vec::with_capacity(values.len());
    let (mut x, mut y, mut w, mut h) = (area.x, area.y, area.w, area.h);
    let mut i = 0;
    while i < areas.len() {
        let short = w.min(h);
        // Greedily extend the current row while the worst aspect ratio improves.
        let mut end = i + 1;
        let mut best = worst(&areas[i..end], short);
        while end < areas.len() {
            let cand = worst(&areas[i..end + 1], short);
            if cand > best {
                break;
            }
            best = cand;
            end += 1;
        }
        let row = &areas[i..end];
        let row_sum: f64 = row.iter().sum();
        if w <= h {
            // Horizontal row across the top of the remaining area.
            let row_h = row_sum / w;
            let mut rx = x;
            for &a in row {
                let rw = a / row_h;
                out.push(Rect {
                    x: rx,
                    y,
                    w: rw,
                    h: row_h,
                });
                rx += rw;
            }
            y += row_h;
            h -= row_h;
        } else {
            // Vertical column down the left of the remaining area.
            let col_w = row_sum / h;
            let mut ry = y;
            for &a in row {
                let rh = a / col_w;
                out.push(Rect {
                    x,
                    y: ry,
                    w: col_w,
                    h: rh,
                });
                ry += rh;
            }
            x += col_w;
            w -= col_w;
        }
        i = end;
    }
    out
}

/// Worst (largest) aspect ratio produced by laying `row` along a side of
/// length `side` — the Bruls/Huizing/van Wijk objective.
fn worst(row: &[f64], side: f64) -> f64 {
    let s: f64 = row.iter().sum();
    if s <= 0.0 || side <= 0.0 {
        return f64::INFINITY;
    }
    let rmax = row.iter().cloned().fold(f64::MIN, f64::max);
    let rmin = row.iter().cloned().fold(f64::MAX, f64::min);
    let side2 = side * side;
    let s2 = s * s;
    (side2 * rmax / s2).max(s2 / (side2 * rmin))
}
