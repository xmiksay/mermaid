//! Relation endpoints, and the routes between a boundary and one of its own
//! members (straight grid connectors live in `rel_straight`).

use std::collections::HashMap;

use crate::parse::C4Element;

use super::super::label_layout::segment_rect_overlap;
use super::grid_route::detour;
use super::outline::{border_toward, center, contains, outline_for, Outline, Rect};
use super::rel::{ARROWHEAD, ARROWHEAD_PAD};
use super::Placement;

/// Boundary frames are drawn with `rx="2.5"`.
const BOUNDARY_RX: f64 = 2.5;
/// Height of a boundary's header band (title + `[kind]`).
pub(super) const BOUNDARY_HEADER_H: f64 = 40.0;
/// A boundary↔member route slides along its face in steps of this much (up
/// to `END_SLIDES` each way) to stay clear of other relations' ends.
const END_SLIDE: f64 = 24.0;
const END_SLIDES: usize = 4;

/// Other relations' ends and lines an enclosure route keeps clear of.
type Nearby<'a> = (&'a [(f64, f64)], &'a [&'a [(f64, f64)]]);

/// A relation's path control points, and whether it is drawn as a spline.
pub(super) type RoutePath = (Vec<(f64, f64)>, bool);

/// A relation endpoint: its outline, rect and whether it is a boundary.
pub(super) type Endpoint = (Outline, Rect, bool);

/// Every placed shape and boundary by alias. When an alias is declared
/// twice, the first declaration (in source pre-order) is the endpoint — in
/// both layout modes, which both emit their lists in pre-order.
pub(super) fn endpoint_shapes<'p>(
    elements: &[C4Element],
    placement: &'p Placement,
) -> HashMap<&'p str, Endpoint> {
    let mut first_is_boundary: HashMap<&str, bool> = HashMap::new();
    let mut stack: Vec<&C4Element> = elements.iter().rev().collect();
    while let Some(el) = stack.pop() {
        first_is_boundary
            .entry(el.alias.as_str())
            .or_insert(el.boundary_kind.is_some());
        stack.extend(el.members.iter().rev());
    }
    let mut out: HashMap<&'p str, Endpoint> = HashMap::new();
    for b in &placement.boundaries {
        if first_is_boundary.get(b.alias.as_str()) == Some(&true) {
            out.entry(b.alias.as_str())
                .or_insert((Outline::RoundRect(BOUNDARY_RX), b.rect, true));
        }
    }
    for (el, r) in &placement.leaves {
        if first_is_boundary.get(el.alias.as_str()) == Some(&false) {
            out.entry(el.alias.as_str())
                .or_insert((outline_for(el.kind), *r, false));
        }
    }
    out
}

/// For a boundary and one of its own members: the shortest straight run
/// from the member's outline to the frame — down, left, right or up — that
/// crosses no other shape and no boundary header, oriented from `from` to
/// `to`. Along the chosen face it slides away from every point in `taken`
/// (other relations' ends), so it never lands on another arrow. `None`
/// unless one endpoint is a boundary enclosing the other.
pub(super) fn enclosure_route(
    from: Endpoint,
    to: Endpoint,
    placement: &Placement,
    (taken, lines): Nearby,
    label: (f64, f64),
) -> Option<Vec<(f64, f64)>> {
    let contains = |o: Rect, i: Rect| {
        i.0 >= o.0 && i.1 >= o.1 && i.0 + i.2 <= o.0 + o.2 && i.1 + i.3 <= o.1 + o.3
    };
    let (frame, member, outward) = if from.2 && contains(from.1, to.1) {
        (from.1, to, false)
    } else if to.2 && contains(to.1, from.1) {
        (to.1, from, true)
    } else {
        return None;
    };
    let (outline, m, _) = member;
    let (cx, cy) = center(m);
    let blockers: Vec<Rect> = placement
        .leaves
        .iter()
        .map(|(_, r)| *r)
        .filter(|r| *r != m)
        .chain(
            placement
                .boundaries
                .iter()
                .map(|b| (b.rect.0, b.rect.1, b.rect.2, BOUNDARY_HEADER_H)),
        )
        // A long title wraps below the fixed header band.
        .chain(placement.boundaries.iter().map(|b| b.header_text()))
        .collect();
    // (ray start inside the member, frame point) per side and slide.
    let mut rays = Vec::new();
    for k in 0..=2 * END_SLIDES {
        let o = (k.div_ceil(2) as f64) * END_SLIDE * if k % 2 == 1 { 1.0 } else { -1.0 };
        if o.abs() <= m.2 / 2.0 - END_SLIDE {
            let x = cx + o;
            rays.push(((x, cy), (x, frame.1 + frame.3), o));
            rays.push(((x, cy), (x, frame.1), o));
        }
        if o.abs() <= m.3 / 2.0 - END_SLIDE / 2.0 {
            let y = cy + o;
            rays.push(((cx, y), (frame.0, y), o));
            rays.push(((cx, y), (frame.0 + frame.2, y), o));
        }
    }
    let (blocked, .., b, t) = rays
        .into_iter()
        .map(|(inside, t, o)| {
            let b = border_toward(outline, m, inside, t);
            let blocked = blockers
                .iter()
                .any(|&r| segment_rect_overlap(b, t, r) > 0.0);
            let crowded = taken
                .iter()
                .any(|q| (q.0 - b.0).hypot(q.1 - b.1) < END_SLIDE / 2.0)
                || runs_along(b, t, lines);
            let len = (t.0 - b.0).hypot(t.1 - b.1);
            // Too short to hold the label clear of the arrowhead: along a
            // vertical run the label takes its height, else its width.
            let along = if (t.0 - inside.0).abs() < 1e-9 {
                label.1
            } else {
                label.0
            };
            let short = label != (0.0, 0.0) && len < along + 2.0 * (ARROWHEAD + ARROWHEAD_PAD);
            (blocked, crowded, short, len.round(), o.abs(), b, t)
        })
        .min_by(|x, y| {
            (x.0, x.1, x.2)
                .cmp(&(y.0, y.1, y.2))
                .then(x.3.total_cmp(&y.3))
                .then(x.4.total_cmp(&y.4))
        })?;
    let route = if blocked {
        // Every straight way out runs through a shape or a header: go round
        // them to the nearest point of the frame instead.
        enclosure_detour((outline, m), frame, &blockers)?
    } else {
        vec![b, t]
    };
    Some(if outward {
        route
    } else {
        route.into_iter().rev().collect()
    })
}

/// A detour from member `m` to its frame's nearest reachable edge point
/// round `blockers`, clipped to the member's outline.
fn enclosure_detour(
    (outline, m): (Outline, Rect),
    frame: Rect,
    blockers: &[Rect],
) -> Option<Vec<(f64, f64)>> {
    let (cx, cy) = center(m);
    let (fx, fy, fw, fh) = frame;
    let mut targets = [(cx, fy + fh), (fx, cy), (fx + fw, cy), (cx, fy)];
    targets.sort_by(|a, b| {
        (a.0 - cx)
            .hypot(a.1 - cy)
            .total_cmp(&(b.0 - cx).hypot(b.1 - cy))
    });
    targets.into_iter().find_map(|t| {
        let goal = (t.0 - 1.0, t.1 - 1.0, 2.0, 2.0);
        let path = detour((cx, cy), t, [m, goal], blockers, &[])?;
        let first_out = path.iter().position(|&p| !contains(outline, m, p))?;
        let entry = border_toward(outline, m, path[first_out - 1], path[first_out]);
        Some(
            std::iter::once(entry)
                .chain(path[first_out..].iter().copied())
                .collect(),
        )
    })
}

/// Whether the segment `a`–`b` runs on top of one of `lines` for more than a
/// few pixels: within 3px of it, heading the same way.
fn runs_along(a: (f64, f64), b: (f64, f64), lines: &[&[(f64, f64)]]) -> bool {
    let len = (b.0 - a.0).hypot(b.1 - a.1);
    if len < 1e-9 {
        return false;
    }
    let u = ((b.0 - a.0) / len, (b.1 - a.1) / len);
    let segs: Vec<((f64, f64), (f64, f64))> = lines
        .iter()
        .flat_map(|l| l.windows(2).map(|w| (w[0], w[1])))
        .filter(|(c, d)| {
            let l = (d.0 - c.0).hypot(d.1 - c.1);
            l > 1e-9 && ((d.0 - c.0) * u.0 + (d.1 - c.1) * u.1).abs() / l > 0.99
        })
        .collect();
    let steps = len.ceil() as usize;
    let mut run = 0.0;
    for k in 0..=steps {
        let t = k as f64 / steps.max(1) as f64;
        let p = (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
        let near = segs.iter().any(|&(c, d)| {
            let (dx, dy) = (d.0 - c.0, d.1 - c.1);
            let s = (((p.0 - c.0) * dx + (p.1 - c.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
            (p.0 - c.0 - s * dx).hypot(p.1 - c.1 - s * dy) < 3.0
        });
        run = if near {
            run + len / steps.max(1) as f64
        } else {
            0.0
        };
        if run > 4.0 {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enclosure_route_runs_from_member_to_nearest_clear_frame_side() {
        let frame = (0.0, 0.0, 400.0, 300.0);
        let member = (150.0, 200.0, 100.0, 60.0);
        let p = Placement::default();
        let b = (Outline::RoundRect(BOUNDARY_RX), frame, true);
        let m = (Outline::RoundRect(6.0), member, false);
        let close = |got: Vec<(f64, f64)>, want: [(f64, f64); 2]| {
            assert_eq!(got.len(), 2);
            for (g, w) in got.iter().zip(want) {
                assert!(
                    (g.0 - w.0).abs() < 1e-6 && (g.1 - w.1).abs() < 1e-6,
                    "{got:?} vs {want:?}"
                );
            }
        };
        // From the frame (bottom edge, nearest) to the member's bottom face.
        let route = enclosure_route(b, m, &p, (&[], &[]), (0.0, 0.0)).expect("encloses");
        close(route, [(200.0, 300.0), (200.0, 260.0)]);
        let back = enclosure_route(m, b, &p, (&[], &[]), (0.0, 0.0)).expect("encloses");
        close(back, [(200.0, 260.0), (200.0, 300.0)]);
        // Another relation already ends at that spot: slide along the face.
        let slid =
            enclosure_route(b, m, &p, (&[(200.0, 260.0)], &[]), (0.0, 0.0)).expect("encloses");
        close(slid, [(224.0, 300.0), (224.0, 260.0)]);
        // Not an enclosure: unrelated shapes.
        let other = (Outline::RoundRect(6.0), (500.0, 0.0, 10.0, 10.0), false);
        assert!(enclosure_route(other, m, &p, (&[], &[]), (0.0, 0.0)).is_none());
    }

    #[test]
    fn a_walled_in_member_detours_to_its_frame() {
        // Shapes close every straight way out: the route goes round them
        // and still ends on the frame, clear of them all.
        let frame = (0.0, 0.0, 600.0, 400.0);
        let member = (250.0, 170.0, 100.0, 60.0);
        let walls = [
            (230.0, 260.0, 140.0, 40.0),
            (230.0, 100.0, 140.0, 40.0),
            (130.0, 150.0, 60.0, 100.0),
            (410.0, 150.0, 60.0, 100.0),
        ];
        let route = enclosure_detour((Outline::RoundRect(6.0), member), frame, &walls)
            .expect("a way round");
        let last = route[route.len() - 1];
        let on_frame = |p: (f64, f64)| {
            p.0.abs() < 6.0
                || (p.0 - 600.0).abs() < 6.0
                || p.1.abs() < 6.0
                || (p.1 - 400.0).abs() < 6.0
        };
        assert!(on_frame(last), "{route:?}");
        for w in route.windows(2) {
            assert!(
                walls
                    .iter()
                    .all(|&r| segment_rect_overlap(w[0], w[1], r) == 0.0),
                "{route:?}"
            );
        }
    }
}
