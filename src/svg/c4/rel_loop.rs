//! Self-relations: a loop on the cheapest face of the shape, and the room
//! the layouts reserve for it.

use std::collections::HashMap;

use crate::parse::{C4Element, C4Relation};

use super::super::label_layout::{rect_overlap_area, segment_rect_overlap};
use super::outline::{border_toward, center, Rect};
use super::rel::{ARROWHEAD, ARROWHEAD_PAD};
use super::rel_label::inflate;
use super::rel_route::{Endpoint, BOUNDARY_HEADER_H};
use super::text::RelLabel;
use super::Placement;

/// How far a self-relation loop reaches out of its shape, and half the
/// spacing of its two ends.
const LOOP_REACH: f64 = 40.0;
const LOOP_HALF_SPAN: f64 = 16.0;
const LOOP_LABEL_GAP: f64 = 12.0;
/// A loop keeps this far from any other shape.
const LOOP_CLEARANCE: f64 = 8.0;
/// A second loop on a face nests round the first, this much wider and
/// further out.
const LOOP_NEST: f64 = 12.0;

/// Room a shape with self-relations needs beside its right side for the
/// loop and its label, keyed by alias; layouts reserve it so the loop never
/// runs into a neighbour or out of a boundary frame.
pub(super) fn loop_room(rels: &[C4Relation], narrow: bool) -> HashMap<&str, f64> {
    // (loops so far, widest label) per shape.
    let mut loops: HashMap<&str, (usize, f64)> = HashMap::new();
    for r in rels.iter().filter(|r| r.from == r.to) {
        let label_w = RelLabel::new(r, narrow).map_or(0.0, |l| l.w);
        let e = loops.entry(r.from.as_str()).or_insert((0, 0.0));
        *e = (e.0 + 1, e.1.max(label_w));
    }
    loops
        .into_iter()
        .map(|(alias, (n, w))| {
            // Loops share the two side faces before they nest.
            let nested = n.saturating_sub(1) / 2;
            (
                alias,
                LOOP_REACH + nested as f64 * LOOP_NEST + w + LOOP_LABEL_GAP,
            )
        })
        .collect()
}

/// A self-relation loop as B-spline control points whose two ends sit on
/// the drawn outline. Each face — right, left (the layouts reserve room
/// there), top, bottom — is scored by what its loop would hit: another shape
/// or a boundary header (hard), another relation's end on that face
/// (`taken`), another relation's line through the loop (`lines`), or a
/// boundary border; the cheapest face wins, earlier on ties. A face that
/// already holds loops of this shape gets the new one nested round them.
pub(super) fn self_loop(
    ep: Endpoint,
    placement: &Placement,
    taken: &[(f64, f64)],
    lines: &[&[(f64, f64)]],
) -> Vec<(f64, f64)> {
    const HARD: u32 = 8;
    const END: u32 = 3;
    const LINE: u32 = 4;
    const BORDER: u32 = 1;
    let faces = [(1.0, 0.0), (-1.0, 0.0), (0.0, -1.0), (0.0, 1.0)];
    let (r, (cx, cy)) = (ep.1, center(ep.1));
    let on_face = |n: (f64, f64), q: &(f64, f64)| {
        if n.0 != 0.0 {
            (q.0 - (cx + n.0 * r.2 / 2.0)).abs() < 2.0 && (q.1 - cy).abs() <= r.3 / 2.0
        } else {
            (q.1 - (cy + n.1 * r.3 / 2.0)).abs() < 2.0 && (q.0 - cx).abs() <= r.2 / 2.0
        }
    };
    // Loops already on a face: each left two ends on it, at its loop spots.
    let level = |n: (f64, f64)| {
        (0..=taken.len())
            .find(|&k| {
                let pts = loop_on(ep, n, k);
                let ends = [pts[0], pts[pts.len() - 1]];
                !ends
                    .iter()
                    .any(|e| taken.iter().any(|q| (q.0 - e.0).hypot(q.1 - e.1) < 1.0))
            })
            .unwrap_or(taken.len())
    };
    let cost = |n: (f64, f64)| {
        if !nest_fits(r, n, level(n)) {
            return HARD;
        }
        let pts = loop_on(ep, n, level(n));
        let Some(span) = super::outline::bounds(pts.iter().map(|&(x, y)| (x, y, 0.0, 0.0))) else {
            return HARD;
        };
        let reach = span.to_rect();
        let (x0, y0, x1, y1) = (span.x0, span.y0, span.x1, span.y1);
        let hits = |o: Rect| rect_overlap_area(reach, o) > 0.0;
        let hard = placement
            .leaves
            .iter()
            .any(|(_, o)| *o != r && hits(inflate(*o, LOOP_CLEARANCE)))
            || placement
                .boundaries
                .iter()
                .any(|b| hits((b.rect.0, b.rect.1, b.rect.2, BOUNDARY_HEADER_H)));
        let crossed = lines.iter().any(|l| {
            l.windows(2)
                .any(|w| segment_rect_overlap(w[0], w[1], inflate(reach, 2.0)) > 0.0)
        });
        let crosses_border = placement.boundaries.iter().any(|b| {
            let f = b.rect;
            let inside = x0 >= f.0 && y0 >= f.1 && x1 <= f.0 + f.2 && y1 <= f.1 + f.3;
            hits(f) && !inside
        });
        u32::from(hard) * HARD
            + u32::from(taken.iter().any(|q| on_face(n, q))) * END
            + u32::from(crossed) * LINE
            + u32::from(crosses_border) * BORDER
    };
    let n = faces
        .into_iter()
        .min_by_key(|&n| cost(n))
        .unwrap_or(faces[0]);
    loop_on(ep, n, level(n))
}

/// Half the face with outward normal `n` along it.
fn half_along(r: Rect, n: (f64, f64)) -> f64 {
    if n.0 != 0.0 {
        r.3 / 2.0
    } else {
        r.2 / 2.0
    }
}

/// Whether the face is long enough for a loop nested `level` deep a full
/// `LOOP_NEST` outside the one inside it.
fn nest_fits(r: Rect, n: (f64, f64), level: usize) -> bool {
    let along = half_along(r, n);
    level == 0 || LOOP_HALF_SPAN.min(along / 2.0) + level as f64 * LOOP_NEST <= along - 6.0
}

/// The loop on the face with outward normal `n` (an axis unit vector),
/// nested `level` loops out.
fn loop_on((outline, r, _): Endpoint, n: (f64, f64), level: usize) -> Vec<(f64, f64)> {
    let (cx, cy) = center(r);
    // Tangent along the face, and the half-extents across / along it.
    let t = (-n.1, n.0);
    let (across, along) = if n.0 != 0.0 {
        (r.2 / 2.0, r.3 / 2.0)
    } else {
        (r.3 / 2.0, r.2 / 2.0)
    };
    let nest = level as f64 * LOOP_NEST;
    let a = (LOOP_HALF_SPAN.min(along / 2.0) + nest).min((along - 6.0).max(0.0));
    let at = |s: f64, d: f64| (cx + t.0 * s + n.0 * d, cy + t.1 * s + n.1 * d);
    let far = across + LOOP_REACH + nest;
    let start = border_toward(outline, r, at(-a, 0.0), at(-a, far));
    let end = border_toward(outline, r, at(a, 0.0), at(a, far));
    let mid = across + LOOP_REACH / 2.0;
    vec![start, at(-a, mid), at(-a, far), at(a, far), at(a, mid), end]
}

/// Extra room, by boundary alias, a boundary needs below its members for a
/// relation between it and one of its own members: that relation runs from
/// the member to the frame edge, and its label has to fit there clear of
/// the arrowhead.
pub(super) fn enclosure_room<'e>(
    elements: &'e [C4Element],
    rels: &[C4Relation],
    narrow: bool,
) -> HashMap<&'e str, f64> {
    let mut inside: HashMap<&str, Vec<&str>> = HashMap::new();
    fn walk<'e>(el: &'e C4Element, out: &mut HashMap<&'e str, Vec<&'e str>>) -> Vec<&'e str> {
        let mut all = Vec::new();
        for m in &el.members {
            all.push(m.alias.as_str());
            all.extend(walk(m, out));
        }
        if el.boundary_kind.is_some() {
            out.insert(el.alias.as_str(), all.clone());
        }
        all
    }
    for el in elements {
        walk(el, &mut inside);
    }
    let mut room: HashMap<&'e str, f64> = HashMap::new();
    for r in rels.iter().filter(|r| r.from != r.to) {
        for (b, m) in [(&r.from, &r.to), (&r.to, &r.from)] {
            let Some((&alias, members)) = inside.get_key_value(b.as_str()) else {
                continue;
            };
            if !members.contains(&m.as_str()) {
                continue;
            }
            let need =
                RelLabel::new(r, narrow).map_or(0.0, |l| l.h + 2.0 * (ARROWHEAD + ARROWHEAD_PAD));
            let e = room.entry(alias).or_insert(0.0);
            *e = e.max(need);
        }
    }
    room
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::svg::c4::outline::{contains, Outline};

    const BOX: Rect = (100.0, 100.0, 200.0, 80.0);

    fn on_outline(o: Outline, r: Rect, p: (f64, f64)) -> bool {
        let probes = [(0.05, 0.0), (-0.05, 0.0), (0.0, 0.05), (0.0, -0.05)];
        let n = probes
            .iter()
            .filter(|(dx, dy)| contains(o, r, (p.0 + dx, p.1 + dy)))
            .count();
        n > 0 && n < 4
    }

    #[test]
    fn self_loop_leaves_and_returns_on_the_right_border() {
        let o = Outline::RoundRect(6.0);
        let none = Placement::default();
        let pts = self_loop((o, BOX, false), &none, &[], &[]);
        let (first, last) = (pts[0], pts[pts.len() - 1]);
        assert!(on_outline(o, BOX, first) && on_outline(o, BOX, last));
        assert_ne!(first, last);
        assert!(
            pts.iter().all(|p| p.0 >= 300.0 - 1e-6),
            "stays outside the box"
        );
        // A queue's round end still gets exact border ends.
        let q = Outline::RoundRect(40.0);
        let pts = self_loop((q, BOX, false), &none, &[], &[]);
        assert!(on_outline(q, BOX, pts[0]) && on_outline(q, BOX, pts[5]));
    }

    #[test]
    fn self_loop_moves_to_a_free_face() {
        let o = Outline::RoundRect(6.0);
        let mut p = Placement::default();
        // Another relation ends on the right face: the loop goes left.
        let pts = self_loop((o, BOX, false), &p, &[(300.0, 140.0)], &[]);
        assert!(pts.iter().all(|q| q.0 <= 100.0 + 1e-6), "{pts:?}");
        // Neighbours hugging the left and the top: the bottom is next.
        let el = |alias: &str| crate::parse::C4Element {
            kind: crate::parse::C4ElementKind::Container,
            alias: alias.into(),
            label: alias.into(),
            descr: None,
            technology: None,
            sprite: None,
            tags: None,
            link: None,
            external: false,
            boundary_alias: None,
            boundary_label: None,
            boundary_kind: None,
            boundary_type: None,
            members: vec![],
        };
        p.leaves.push((el("left"), (20.0, 100.0, 60.0, 80.0)));
        p.leaves.push((el("above"), (100.0, 40.0, 200.0, 50.0)));
        let pts = self_loop((o, BOX, false), &p, &[(300.0, 140.0)], &[]);
        assert!(pts.iter().all(|q| q.1 >= 180.0 - 1e-6), "{pts:?}");
    }
}
