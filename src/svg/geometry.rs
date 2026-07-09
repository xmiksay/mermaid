//! Shared edge-clipping and polyline geometry used across the diagram
//! renderers. Every renderer that routes an edge to a node boundary clips the
//! endpoint to the node's shape and often labels the edge at its arc-length
//! midpoint; those routines used to be copy-pasted per module.

/// Clip a ray aimed from `from` toward `center` to the boundary of a
/// `w`×`h` rectangle centered at `center`. Returns the intersection point.
pub(super) fn clip_rect(from: (f64, f64), center: (f64, f64), (w, h): (f64, f64)) -> (f64, f64) {
    let dx = from.0 - center.0;
    let dy = from.1 - center.1;
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return center;
    }
    let hw = w / 2.0;
    let hh = h / 2.0;
    let tx = if dx.abs() > 1e-9 {
        hw / dx.abs()
    } else {
        f64::INFINITY
    };
    let ty = if dy.abs() > 1e-9 {
        hh / dy.abs()
    } else {
        f64::INFINITY
    };
    let t = tx.min(ty);
    (center.0 + dx * t, center.1 + dy * t)
}

/// Clip a ray aimed from `from` toward `center` to a circle of radius `r`.
pub(super) fn clip_circle(from: (f64, f64), center: (f64, f64), r: f64) -> (f64, f64) {
    let dx = from.0 - center.0;
    let dy = from.1 - center.1;
    let d = (dx * dx + dy * dy).sqrt().max(1e-9);
    (center.0 + dx * r / d, center.1 + dy * r / d)
}

/// Clip a ray aimed from `from` toward `center` to a `w`×`h` rhombus (diamond)
/// centered at `center`.
pub(super) fn clip_rhombus(from: (f64, f64), center: (f64, f64), (w, h): (f64, f64)) -> (f64, f64) {
    let dx = from.0 - center.0;
    let dy = from.1 - center.1;
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return center;
    }
    let hw = w / 2.0;
    let hh = h / 2.0;
    let t = 1.0 / (dx.abs() / hw + dy.abs() / hh).max(1e-9);
    (center.0 + dx * t, center.1 + dy * t)
}

/// The point at half the total arc-length along a polyline — used to place an
/// edge's label at its visual midpoint rather than the midpoint of its
/// bounding box.
pub(super) fn polyline_midpoint(pts: &[(f64, f64)]) -> (f64, f64) {
    polyline_midpoint_offset(pts, 0.0)
}

/// Like [`polyline_midpoint`], but shifts the returned point `offset` units of
/// arc-length along the polyline away from its midpoint — negative pulls toward
/// the start, positive toward the end (clamped to the polyline). Used to pull
/// the two labels of an opposite-pair transition apart along their own edges so
/// one label's opaque background does not occlude the other (#312).
pub(super) fn polyline_midpoint_offset(pts: &[(f64, f64)], offset: f64) -> (f64, f64) {
    if pts.len() < 2 {
        return pts.first().copied().unwrap_or((0.0, 0.0));
    }
    let mut segs = Vec::with_capacity(pts.len() - 1);
    let mut total = 0.0;
    for w in pts.windows(2) {
        let dx = w[1].0 - w[0].0;
        let dy = w[1].1 - w[0].1;
        let l = (dx * dx + dy * dy).sqrt();
        segs.push(l);
        total += l;
    }
    let target = (total / 2.0 + offset).clamp(0.0, total);
    let mut walked = 0.0;
    for (i, w) in pts.windows(2).enumerate() {
        if walked + segs[i] >= target {
            let t = (target - walked) / segs[i].max(1e-9);
            return (
                w[0].0 + t * (w[1].0 - w[0].0),
                w[0].1 + t * (w[1].1 - w[0].1),
            );
        }
        walked += segs[i];
    }
    pts[pts.len() / 2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clip_rect_hits_vertical_edge() {
        // Ray straight to the right clips at the right edge (hw = 5).
        let p = clip_rect((100.0, 0.0), (0.0, 0.0), (10.0, 4.0));
        assert!((p.0 - 5.0).abs() < 1e-9);
        assert!(p.1.abs() < 1e-9);
    }

    #[test]
    fn clip_rect_coincident_returns_center() {
        let p = clip_rect((1.0, 1.0), (1.0, 1.0), (10.0, 10.0));
        assert_eq!(p, (1.0, 1.0));
    }

    #[test]
    fn clip_circle_lands_on_radius() {
        let p = clip_circle((10.0, 0.0), (0.0, 0.0), 3.0);
        assert!((p.0 - 3.0).abs() < 1e-9);
        assert!(p.1.abs() < 1e-9);
    }

    #[test]
    fn clip_rhombus_on_axis_matches_half_extent() {
        // On-axis the rhombus boundary coincides with the rectangle's.
        let p = clip_rhombus((10.0, 0.0), (0.0, 0.0), (8.0, 6.0));
        assert!((p.0 - 4.0).abs() < 1e-9);
        assert!(p.1.abs() < 1e-9);
    }

    #[test]
    fn polyline_midpoint_of_straight_line() {
        let p = polyline_midpoint(&[(0.0, 0.0), (10.0, 0.0)]);
        assert!((p.0 - 5.0).abs() < 1e-9);
        assert!(p.1.abs() < 1e-9);
    }

    #[test]
    fn polyline_midpoint_empty_is_origin() {
        assert_eq!(polyline_midpoint(&[]), (0.0, 0.0));
    }
}
