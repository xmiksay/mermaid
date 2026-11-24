//! Geometry invariants every rendered C4 scene must satisfy, checked on the
//! laid-out scene rather than the SVG text.

use super::*;
use crate::svg::label_layout::{rect_overlap_area, segment_rect_overlap};

pub(super) fn c4(src: &str) -> C4Diagram {
    match crate::parse::parse(src) {
        Ok(crate::parse::Diagram::C4(d)) => d,
        other => panic!("not a C4 diagram: {other:?}"),
    }
}

pub(super) fn scene(d: &C4Diagram) -> Scene<'_> {
    layout_scene(d, &HashMap::new(), &Theme::default())
}

/// Every element, depth-first in declaration order.
fn all_elements(d: &C4Diagram) -> Vec<&C4Element> {
    let mut out = Vec::new();
    let mut stack: Vec<&C4Element> = d.elements.iter().rev().collect();
    while let Some(el) = stack.pop() {
        out.push(el);
        stack.extend(el.members.iter().rev());
    }
    out
}

/// Every alias nested (at any depth) under `el`.
fn members(el: &C4Element) -> Vec<&str> {
    let mut out = Vec::new();
    let mut stack: Vec<&C4Element> = el.members.iter().collect();
    while let Some(m) = stack.pop() {
        out.push(m.alias.as_str());
        stack.extend(m.members.iter());
    }
    out
}

pub(super) fn contains(outer: Rect, inner: Rect) -> bool {
    inner.0 >= outer.0 - 1e-6
        && inner.1 >= outer.1 - 1e-6
        && inner.0 + inner.2 <= outer.0 + outer.2 + 1e-6
        && inner.1 + inner.3 <= outer.1 + outer.3 + 1e-6
}

/// Boundary frames hold exactly their members: members inside, foreign
/// shapes and non-nested frames outside.
pub(super) fn check_frames(d: &C4Diagram, p: &Placement) {
    let els = all_elements(d);
    for b in &p.boundaries {
        let Some(el) = els
            .iter()
            .find(|e| e.alias == b.alias && e.boundary_kind.is_some())
        else {
            continue;
        };
        let inside = members(el);
        for (leaf, r) in &p.leaves {
            if inside.contains(&leaf.alias.as_str()) {
                assert!(contains(b.rect, *r), "{} escapes {}", leaf.alias, b.alias);
            } else {
                assert_eq!(
                    rect_overlap_area(b.rect, *r),
                    0.0,
                    "foreign shape {} overlaps frame {}",
                    leaf.alias,
                    b.alias
                );
            }
        }
        for other in &p.boundaries {
            let nested = contains(b.rect, other.rect) || contains(other.rect, b.rect);
            if other.alias != b.alias && !nested {
                assert_eq!(
                    rect_overlap_area(b.rect, other.rect),
                    0.0,
                    "frames {} and {} overlap",
                    b.alias,
                    other.alias
                );
            }
        }
    }
}

/// The boundary's header text block (title + `[kind]` line).
fn header_text(d: &C4Diagram, b: &BoundaryBox) -> Option<Rect> {
    let el = all_elements(d)
        .into_iter()
        .find(|e| e.alias == b.alias && e.boundary_kind.is_some())?;
    let w = header_min_w(el, Theme::default().font_size) - 28.0;
    Some((b.rect.0 + 14.0, b.rect.1 + 4.0, w, 36.0))
}

/// Distance from `p` to the outline of `r` (inside or out).
fn border_distance(p: (f64, f64), r: Rect) -> f64 {
    let (x0, y0, x1, y1) = (r.0, r.1, r.0 + r.2, r.1 + r.3);
    let dx = (x0 - p.0).max(0.0).max(p.0 - x1);
    let dy = (y0 - p.1).max(0.0).max(p.1 - y1);
    if dx > 0.0 || dy > 0.0 {
        dx.hypot(dy)
    } else {
        (p.0 - x0).min(x1 - p.0).min(p.1 - y0).min(y1 - p.1)
    }
}

/// Longest stretch of `line` whose consecutive points all satisfy `near`.
fn longest_run(line: &[(f64, f64)], near: impl Fn((f64, f64)) -> bool) -> f64 {
    let (mut run, mut best) = (0.0_f64, 0.0_f64);
    for w in line.windows(2) {
        run = if near(w[0]) && near(w[1]) {
            run + (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1)
        } else {
            0.0
        };
        best = best.max(run);
    }
    best
}

/// A point where the drawn line turns back on itself (two consecutive
/// segments of over a pixel pointing nearly opposite ways).
pub(super) fn spike(line: &[(f64, f64)]) -> Option<(f64, f64)> {
    line.windows(3).find_map(|w| {
        let v1 = (w[1].0 - w[0].0, w[1].1 - w[0].1);
        let v2 = (w[2].0 - w[1].0, w[2].1 - w[1].1);
        let (l1, l2) = (v1.0.hypot(v1.1), v2.0.hypot(v2.1));
        let back = l1 > 1.0 && l2 > 1.0 && (v1.0 * v2.0 + v1.1 * v2.1) / (l1 * l2) < -0.95;
        back.then_some(w[1])
    })
}

/// Length of `line` inside the drawn outline shrunk by 3px — sampled per
/// pixel, so a line ending on a rounded corner or cap doesn't count.
pub(super) fn inside_length(line: &[(f64, f64)], o: outline::Outline, r: Rect) -> f64 {
    let inner = match o {
        outline::Outline::RoundRect(rad) => outline::Outline::RoundRect((rad - 3.0).max(0.0)),
        other => other,
    };
    let rect = shrink(r, 3.0);
    let mut len = 0.0;
    for w in line.windows(2) {
        let d = (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1);
        let steps = d.ceil().max(1.0) as usize;
        for k in 0..steps {
            let t = (k as f64 + 0.5) / steps as f64;
            let p = (
                w[0].0 + t * (w[1].0 - w[0].0),
                w[0].1 + t * (w[1].1 - w[0].1),
            );
            if outline::contains(inner, rect, p) {
                len += d / steps as f64;
            }
        }
    }
    len
}

fn shrink(r: Rect, m: f64) -> Rect {
    (r.0 + m, r.1 + m, r.2 - 2.0 * m, r.3 - 2.0 * m)
}

/// A point on the outline: just inside along some probe, just outside
/// along another.
fn on_outline(o: outline::Outline, r: Rect, p: (f64, f64)) -> bool {
    let probes = [(0.05, 0.0), (-0.05, 0.0), (0.0, 0.05), (0.0, -0.05)];
    let n = probes
        .iter()
        .filter(|(dx, dy)| outline::contains(o, r, (p.0 + dx, p.1 + dy)))
        .count();
    n > 0 && n < 4
}

/// Every invariant at once: labels clear shapes, headers and each other;
/// frames hold their members; relation ends sit on their endpoints' drawn
/// outlines; no line runs through any shape, its own endpoints included, or
/// hugs a shape's border; no line doubles back on itself; and no line
/// crosses a boundary's header text.
pub(super) fn check_scene(d: &C4Diagram) {
    let s = scene(d);
    let p = &s.placement;
    check_frames(d, p);
    super::lines::check_arrowheads(&s);
    super::lines::check_no_shared_stretch(&s);

    let headers: Vec<(&str, Rect)> = p
        .boundaries
        .iter()
        .filter_map(|b| Some((b.alias.as_str(), header_text(d, b)?)))
        .collect();
    let labels: Vec<(&str, Rect)> = s
        .rels
        .iter()
        .filter_map(|r| Some((r.rel.label.as_str(), r.label_rect()?)))
        .collect();
    for (i, (name, l)) in labels.iter().enumerate() {
        for (el, r) in &p.leaves {
            let hit = rect_overlap_area(*l, *r);
            assert_eq!(hit, 0.0, "label {name:?} overlaps shape {}", el.alias);
        }
        for (b, h) in &headers {
            let hit = rect_overlap_area(*l, *h);
            assert_eq!(hit, 0.0, "label {name:?} overlaps the {b} header");
        }
        for (other, r) in &labels[i + 1..] {
            let hit = rect_overlap_area(*l, *r);
            assert_eq!(hit, 0.0, "labels {name:?} and {other:?} overlap");
        }
    }

    let ends = rel_route::endpoint_shapes(&d.elements, p);
    for plan in &s.rels {
        let r = plan.rel;
        let tag = format!("{} -> {}", r.from, r.to);
        let (Some(from), Some(to)) = (ends.get(r.from.as_str()), ends.get(r.to.as_str())) else {
            panic!("{tag}: planned without endpoints");
        };
        let (first, last) = (plan.path[0], plan.path[plan.path.len() - 1]);
        assert!(
            on_outline(from.0, from.1, first),
            "{tag}: start {first:?} off its outline"
        );
        assert!(
            on_outline(to.0, to.1, last),
            "{tag}: end {last:?} off its outline"
        );
        for (el, rect) in &p.leaves {
            let through = inside_length(&plan.line, outline::outline_for(el.kind), *rect);
            assert!(
                through <= 1.0,
                "{tag} runs {through:.0}px through {}",
                el.alias
            );
            let hug = longest_run(&plan.line, |q| border_distance(q, *rect) <= 4.0);
            assert!(
                hug <= 10.0,
                "{tag} runs {hug:.0}px along {}'s border",
                el.alias
            );
        }
        if let Some(at) = spike(&plan.line) {
            panic!("{tag} doubles back on itself at {at:?}");
        }
        {
            for (b, h) in &headers {
                let through: f64 = plan
                    .line
                    .windows(2)
                    .map(|w| segment_rect_overlap(w[0], w[1], *h))
                    .sum();
                assert_eq!(through, 0.0, "{tag} crosses the {b} header text");
            }
        }
    }
}
