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

/// Total arc-length of a polyline — used by [`super::label_layout`] to bound
/// how far a colliding label may be nudged along its own edge.
pub(super) fn polyline_length(pts: &[(f64, f64)]) -> f64 {
    pts.windows(2)
        .map(|w| {
            let dx = w[1].0 - w[0].0;
            let dy = w[1].1 - w[0].1;
            (dx * dx + dy * dy).sqrt()
        })
        .sum()
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

/// A routed polyline (`>= 2` points) with its first point replaced by
/// `first(pts[1])` and its last by `last(pts[n - 2])`: each end clipped on the
/// ray from its neighbour to the end shape.
pub(super) fn clip_polyline(
    pts: &[(f64, f64)],
    first: impl FnOnce((f64, f64)) -> (f64, f64),
    last: impl FnOnce((f64, f64)) -> (f64, f64),
) -> Vec<(f64, f64)> {
    let n = pts.len();
    let mut clipped = Vec::with_capacity(n);
    clipped.push(first(pts[1]));
    clipped.extend(&pts[1..n - 1]);
    clipped.push(last(pts[n - 2]));
    clipped
}

/// [`clip_polyline`] onto the `a`- and `b`-sized rects centred on the
/// polyline's first and last points.
pub(super) fn clip_rect_ends(pts: &[(f64, f64)], a: (f64, f64), b: (f64, f64)) -> Vec<(f64, f64)> {
    let (s, t) = (pts[0], pts[pts.len() - 1]);
    clip_polyline(pts, |p| clip_rect(p, s, a), |p| clip_rect(p, t, b))
}

/// Keep edge geometry on a `w`×`h` canvas: `pts` are points on every drawn
/// edge curve. Where they overflow a side, the drawing is shifted / the
/// canvas grown so they sit `pad` inside it; elsewhere nothing changes.
/// Returns the shift `(dx, dy)` and the new canvas size.
pub(super) fn fit_edges(
    pts: impl IntoIterator<Item = (f64, f64)>,
    (w, h): (f64, f64),
    pad: f64,
) -> ((f64, f64), (f64, f64)) {
    let (mut lo, mut hi) = ((f64::INFINITY, f64::INFINITY), (0.0_f64, 0.0_f64));
    for (x, y) in pts {
        lo = (lo.0.min(x), lo.1.min(y));
        hi = (hi.0.max(x), hi.1.max(y));
    }
    let fit = |lo: f64, hi: f64, extent: f64| {
        let shift = if lo < 0.0 { pad - lo } else { 0.0 };
        let grown = extent + shift;
        (
            shift,
            if hi + shift > grown {
                hi + shift + pad
            } else {
                grown
            },
        )
    };
    let (dx, w) = fit(lo.0, hi.0, w);
    let (dy, h) = fit(lo.1, hi.1, h);
    ((dx, dy), (w, h))
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
    fn fit_edges_only_moves_overflowing_sides() {
        let inside = [(5.0, 5.0), (95.0, 45.0)];
        assert_eq!(
            fit_edges(inside, (100.0, 50.0), 24.0),
            ((0.0, 0.0), (100.0, 50.0))
        );
        // Off the left by 6 and the bottom by 10: shift right 30, grow both.
        let out = [(-6.0, 5.0), (50.0, 60.0)];
        assert_eq!(
            fit_edges(out, (100.0, 50.0), 24.0),
            ((30.0, 0.0), (130.0, 84.0))
        );
        assert_eq!(
            fit_edges([], (100.0, 50.0), 24.0),
            ((0.0, 0.0), (100.0, 50.0))
        );
    }

    #[test]
    fn polyline_midpoint_empty_is_origin() {
        assert_eq!(polyline_midpoint(&[]), (0.0, 0.0));
    }
}
