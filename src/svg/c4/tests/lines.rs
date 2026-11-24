//! Line-level invariants: arrowheads stay visible, and two relations never
//! draw on top of each other.

use super::*;
use crate::svg::label_layout::segment_rect_overlap;

type Pt = (f64, f64);

/// The arrowhead at the `tip` end of `line` (walked from the tip): the
/// marker is 9px long and 9px wide.
fn arrowhead(from_tip: &[Pt]) -> Option<[Pt; 3]> {
    let tip = *from_tip.first()?;
    let back = from_tip
        .iter()
        .find(|q| (q.0 - tip.0).hypot(q.1 - tip.1) > 0.5)?;
    let (dx, dy) = (back.0 - tip.0, back.1 - tip.1);
    let len = dx.hypot(dy);
    let (ux, uy) = (dx / len, dy / len);
    let base = (tip.0 + ux * 9.0, tip.1 + uy * 9.0);
    let (px, py) = (-uy * 4.5, ux * 4.5);
    Some([tip, (base.0 + px, base.1 + py), (base.0 - px, base.1 - py)])
}

fn in_triangle(p: Pt, [a, b, c]: [Pt; 3]) -> bool {
    let side = |o: Pt, q: Pt| (q.0 - o.0) * (p.1 - o.1) - (q.1 - o.1) * (p.0 - o.0);
    let (d1, d2, d3) = (side(a, b), side(b, c), side(c, a));
    !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
}

fn touches(r: Rect, t: [Pt; 3]) -> bool {
    let inside = |p: Pt| p.0 > r.0 && p.0 < r.0 + r.2 && p.1 > r.1 && p.1 < r.1 + r.3;
    let corners = [
        (r.0, r.1),
        (r.0 + r.2, r.1),
        (r.0, r.1 + r.3),
        (r.0 + r.2, r.1 + r.3),
    ];
    t.iter().any(|&p| inside(p))
        || corners.iter().any(|&c| in_triangle(c, t))
        || (0..3).any(|k| segment_rect_overlap(t[k], t[(k + 1) % 3], r) > 0.0)
}

/// No label covers any arrowhead (requirements 3 and 5: every arrowhead
/// stays visible).
pub(super) fn check_arrowheads(s: &Scene) {
    let heads: Vec<(String, [Pt; 3])> = s
        .rels
        .iter()
        .flat_map(|p| {
            let tag = format!("{} -> {}", p.rel.from, p.rel.to);
            let rev: Vec<Pt> = p.line.iter().rev().copied().collect();
            let mut out = vec![(tag.clone(), arrowhead(&rev))];
            if p.rel.bidirectional {
                out.push((tag, arrowhead(&p.line)));
            }
            out.into_iter().filter_map(|(t, h)| Some((t, h?)))
        })
        .collect();
    for p in &s.rels {
        let Some(label) = p.label_rect() else {
            continue;
        };
        for (tag, head) in &heads {
            assert!(
                !touches(label, *head),
                "label {:?} covers the arrowhead of {tag}",
                p.rel.label
            );
        }
    }
}

fn seg_dist(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 < 1e-12 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    };
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}

/// No two relations run on top of each other: no stretch longer than 4px of
/// one line lies within 2px of another *running the same way* (within 5°) —
/// a crossing at any angle spends a few pixels that close, so direction is
/// what tells a shared stretch from a crossing.
pub(super) fn check_no_shared_stretch(s: &Scene) {
    let bbox = |l: &[Pt]| {
        l.iter()
            .fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |b, p| {
                (b.0.min(p.0), b.1.min(p.1), b.2.max(p.0), b.3.max(p.1))
            })
    };
    for (i, a) in s.rels.iter().enumerate() {
        let ba = bbox(&a.line);
        for b in &s.rels[i + 1..] {
            let bb = bbox(&b.line);
            if ba.0 > bb.2 + 2.0 || bb.0 > ba.2 + 2.0 || ba.1 > bb.3 + 2.0 || bb.1 > ba.3 + 2.0 {
                continue;
            }
            let near = |p: Pt, dir: Pt| {
                p.0 > bb.0 - 2.0
                    && p.0 < bb.2 + 2.0
                    && p.1 > bb.1 - 2.0
                    && p.1 < bb.3 + 2.0
                    && b.line.windows(2).any(|w| {
                        let (dx, dy) = (w[1].0 - w[0].0, w[1].1 - w[0].1);
                        let len = dx.hypot(dy);
                        len > 1e-9
                            && seg_dist(p, w[0], w[1]) < 2.0
                            && (dir.0 * dx + dir.1 * dy).abs() / len > 0.996
                    })
            };
            let mut run = 0.0;
            for w in a.line.windows(2) {
                let len = (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1);
                if len < 1e-9 {
                    continue;
                }
                let dir = ((w[1].0 - w[0].0) / len, (w[1].1 - w[0].1) / len);
                let steps = len.ceil().max(1.0) as usize;
                for k in 1..=steps {
                    let t = k as f64 / steps as f64;
                    let p = (
                        w[0].0 + t * (w[1].0 - w[0].0),
                        w[0].1 + t * (w[1].1 - w[0].1),
                    );
                    run = if near(p, dir) {
                        run + len / steps as f64
                    } else {
                        0.0
                    };
                    assert!(
                        run <= 4.0,
                        "{} -> {} and {} -> {} run together at {p:?}",
                        a.rel.from,
                        a.rel.to,
                        b.rel.from,
                        b.rel.to
                    );
                }
            }
        }
    }
}
