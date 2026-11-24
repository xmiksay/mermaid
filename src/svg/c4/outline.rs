//! The exact drawn outline of each C4 shape, so a relation's ends land on the
//! border that is actually painted (rounded corners, cylinder caps, the
//! queue's end caps) rather than on the bounding rectangle.

use crate::parse::C4ElementKind;

use super::text::{queue_rx, CYL_RY};

/// `(x, y, w, h)` with `(x, y)` the top-left corner.
pub(super) type Rect = (f64, f64, f64, f64);

/// An extent given by its corners — kept a distinct type from [`Rect`]
/// (origin + size) so the two can't be mixed up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Span {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Span {
    pub(super) fn new(x0: f64, y0: f64, x1: f64, y1: f64) -> Self {
        Self { x0, y0, x1, y1 }
    }

    pub(super) fn to_rect(self) -> Rect {
        (self.x0, self.y0, self.x1 - self.x0, self.y1 - self.y0)
    }

    pub(super) fn shift(self, (dx, dy): (f64, f64)) -> Self {
        Self::new(self.x0 + dx, self.y0 + dy, self.x1 + dx, self.y1 + dy)
    }

    pub(super) fn contains(self, p: (f64, f64)) -> bool {
        p.0 >= self.x0 && p.0 <= self.x1 && p.1 >= self.y0 && p.1 <= self.y1
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Outline {
    /// A rectangle with corner radius `r` (clamped to half the shorter side,
    /// as SVG does) — boxes, persons and boundary frames.
    RoundRect(f64),
    /// A vertical cylinder whose caps are ellipses of half-height `ry`.
    Cylinder(f64),
    /// A cylinder lying down (a queue): its end caps are ellipses of
    /// half-width `queue_rx(h)` and half-height `h/2`.
    HCylinder,
}

/// Corner radius of the plain C4 box (`rx="6"` in `draw_box`).
pub(super) const BOX_RX: f64 = 6.0;

pub(super) fn outline_for(kind: C4ElementKind) -> Outline {
    match kind {
        C4ElementKind::SystemDb | C4ElementKind::ContainerDb | C4ElementKind::ComponentDb => {
            Outline::Cylinder(CYL_RY)
        }
        C4ElementKind::SystemQueue
        | C4ElementKind::ContainerQueue
        | C4ElementKind::ComponentQueue => Outline::HCylinder,
        _ => Outline::RoundRect(BOX_RX),
    }
}

/// The hull of `rects`, `None` when there are none.
pub(super) fn bounds(rects: impl IntoIterator<Item = Rect>) -> Option<Span> {
    rects.into_iter().fold(None, |acc, (x, y, w, h)| {
        let s = acc.unwrap_or(Span::new(x, y, x + w, y + h));
        Some(Span::new(
            s.x0.min(x),
            s.y0.min(y),
            s.x1.max(x + w),
            s.y1.max(y + h),
        ))
    })
}

pub(super) fn center(r: Rect) -> (f64, f64) {
    (r.0 + r.2 / 2.0, r.1 + r.3 / 2.0)
}

/// Whether `p` lies inside (or on) the outline drawn in `rect`.
pub(super) fn contains(outline: Outline, rect: Rect, p: (f64, f64)) -> bool {
    let (x, y, w, h) = rect;
    if p.0 < x || p.0 > x + w || p.1 < y || p.1 > y + h {
        return false;
    }
    match outline {
        Outline::RoundRect(r) => {
            let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
            // max/min, not clamp: with `r` at half the size the bounds can
            // cross by a rounding error.
            let cx = p.0.max(x + r).min((x + w - r).max(x + r));
            let cy = p.1.max(y + r).min((y + h - r).max(y + r));
            let (dx, dy) = (p.0 - cx, p.1 - cy);
            dx * dx + dy * dy <= r * r + 1e-9
        }
        Outline::Cylinder(ry) => {
            let rx = w / 2.0;
            let in_ellipse = |cy: f64| {
                let ex = (p.0 - (x + rx)) / rx.max(1e-9);
                let ey = (p.1 - cy) / ry.max(1e-9);
                ex * ex + ey * ey <= 1.0 + 1e-9
            };
            if p.1 < y + ry {
                in_ellipse(y + ry)
            } else if p.1 > y + h - ry {
                in_ellipse(y + h - ry)
            } else {
                true
            }
        }
        Outline::HCylinder => {
            let (rx, ry) = (queue_rx(h), h / 2.0);
            let in_ellipse = |cx: f64| {
                let ex = (p.0 - cx) / rx.max(1e-9);
                let ey = (p.1 - (y + ry)) / ry.max(1e-9);
                ex * ex + ey * ey <= 1.0 + 1e-9
            };
            if p.0 < x + rx {
                in_ellipse(x + rx)
            } else if p.0 > x + w - rx {
                in_ellipse(x + w - rx)
            } else {
                true
            }
        }
    }
}

/// The point where segment `inside`→`outside` crosses the outline. Every
/// outline is convex, so the crossing is unique and bisection converges on it.
pub(super) fn border_toward(
    outline: Outline,
    rect: Rect,
    inside: (f64, f64),
    outside: (f64, f64),
) -> (f64, f64) {
    let (mut a, mut b) = (inside, outside);
    for _ in 0..48 {
        let m = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        if contains(outline, rect, m) {
            a = m;
        } else {
            b = m;
        }
    }
    b
}

/// Trim a route that runs from inside the `start` shape to inside the `end`
/// shape so it begins and ends exactly on their drawn outlines. A route that
/// never leaves a shape (overlapping shapes) is returned unchanged.
pub(super) fn clip_route(
    pts: &[(f64, f64)],
    start: (Outline, Rect),
    end: (Outline, Rect),
) -> Vec<(f64, f64)> {
    let Some(first_out) = pts.iter().position(|&p| !contains(start.0, start.1, p)) else {
        return pts.to_vec();
    };
    let mut out = pts.to_vec();
    if first_out > 0 {
        let entry = border_toward(start.0, start.1, pts[first_out - 1], pts[first_out]);
        out = std::iter::once(entry)
            .chain(pts[first_out..].iter().copied())
            .collect();
    }
    let Some(last_out) = out.iter().rposition(|&p| !contains(end.0, end.1, p)) else {
        return pts.to_vec();
    };
    if last_out + 1 < out.len() {
        let exit = border_toward(end.0, end.1, out[last_out + 1], out[last_out]);
        out.truncate(last_out + 1);
        out.push(exit);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECT: Rect = (0.0, 0.0, 100.0, 60.0);

    fn on_outline(outline: Outline, p: (f64, f64)) -> bool {
        on_outline_of(outline, RECT, p)
    }

    #[test]
    fn a_full_pill_survives_rounding() {
        // r = h / 2 with bounds that cross by a rounding error.
        let r = (0.0, 222.156_951_376_686_7, 100.0, 0.000_000_000_000_03);
        assert!(contains(
            Outline::RoundRect(1.0),
            r,
            (50.0, 222.156_951_376_686_7)
        ));
    }

    #[test]
    fn bounds_is_the_hull() {
        assert_eq!(bounds([]), None);
        assert_eq!(
            bounds([(0.0, 0.0, 10.0, 10.0), (-5.0, 20.0, 1.0, 1.0)]),
            Some(Span::new(-5.0, 0.0, 10.0, 21.0))
        );
    }

    #[test]
    fn clip_route_ends_on_a_box_border() {
        let other = (200.0, 200.0, 100.0, 60.0);
        let route = clip_route(
            &[center(RECT), (150.0, 30.0), center(other)],
            (Outline::RoundRect(BOX_RX), RECT),
            (Outline::RoundRect(BOX_RX), other),
        );
        let first = route[0];
        assert!((first.0 - 100.0).abs() < 1e-6 && (first.1 - 30.0).abs() < 1e-6);
        let last = *route.last().unwrap();
        assert!(on_outline_of(Outline::RoundRect(BOX_RX), other, last));
    }

    /// Just inside along some probe direction and just outside along another.
    fn on_outline_of(outline: Outline, rect: Rect, p: (f64, f64)) -> bool {
        let probes = [(0.01, 0.0), (-0.01, 0.0), (0.0, 0.01), (0.0, -0.01)];
        let inside = |(dx, dy): (f64, f64)| contains(outline, rect, (p.0 + dx, p.1 + dy));
        probes.iter().any(|&d| inside(d)) && probes.iter().any(|&d| !inside(d))
    }

    #[test]
    fn diagonal_into_a_rounded_corner_lands_on_the_arc() {
        let outline = Outline::RoundRect(20.0);
        let route = clip_route(
            &[center(RECT), (-200.0, -120.0)],
            (outline, RECT),
            (outline, (-1000.0, -1000.0, 1.0, 1.0)),
        );
        let p = route[0];
        // Inside the corner zone, the bounding-rect corner would be wrong.
        assert!(p.0 > 0.5 && p.1 > 0.5, "{p:?}");
        assert!(on_outline(outline, p));
    }

    #[test]
    fn vertical_entry_into_a_cylinder_meets_the_cap_not_the_box() {
        let outline = Outline::Cylinder(10.0);
        // Coming straight down at x=10, the top cap sits below y=0 there.
        let route = clip_route(
            &[(10.0, -200.0), (10.0, 30.0)],
            (Outline::RoundRect(0.0), (0.0, -220.0, 20.0, 10.0)),
            (outline, RECT),
        );
        let p = *route.last().unwrap();
        assert!(p.1 > 1.0 && p.1 < 10.0, "{p:?}");
        assert!(on_outline(outline, p));
    }

    #[test]
    fn queue_clips_on_its_end_cap() {
        let outline = outline_for(C4ElementKind::ContainerQueue);
        let route = clip_route(
            &[center(RECT), (-200.0, -100.0)],
            (outline, RECT),
            (Outline::RoundRect(0.0), (-300.0, -300.0, 1.0, 1.0)),
        );
        assert!(on_outline(outline, route[0]));
        assert!(route[0].0 > 0.5);
    }
}
