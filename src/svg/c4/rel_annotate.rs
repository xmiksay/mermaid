//! Placing every planned relation's label: one obstacle index for the
//! whole scene, labels placed in source order, each one an obstacle for
//! every later one.

use std::collections::HashMap;

use crate::parse::C4RelStyle;

use super::super::label_layout::rect_at;
use super::outline::Rect;
use super::rel::{RelPlan, ARROWHEAD, ARROWHEAD_PAD};
use super::rel_label::{inflate, Obstacles, Owner, LABEL_CLEARANCE, SHAPE_CLEARANCE};
use super::rel_place::place_label;
use super::rel_route::BOUNDARY_HEADER_H;
use super::text::RelLabel;
use super::Placement;

/// How far a label keeps from a boundary's border, and how much of a line's
/// arrowhead end it keeps clear of.
const FRAME_EDGE_CLEARANCE: f64 = 3.0;
const ARROW_ZONE: f64 = 24.0;

/// The box around the last `length` px of a line, walked from its end,
/// grown by `pad`.
fn end_zone<'p>(mut from_end: impl Iterator<Item = &'p (f64, f64)>, length: f64, pad: f64) -> Rect {
    let Some(&first) = from_end.next() else {
        return (0.0, 0.0, 0.0, 0.0);
    };
    let (mut prev, mut walked) = (first, 0.0);
    let mut pts = vec![first];
    for &p in from_end {
        let d = (p.0 - prev.0).hypot(p.1 - prev.1);
        if walked + d >= length {
            // Stop exactly `length` along: a long last segment must not
            // stretch the zone over the whole line.
            let t = if d > 1e-9 { (length - walked) / d } else { 0.0 };
            pts.push((prev.0 + t * (p.0 - prev.0), prev.1 + t * (p.1 - prev.1)));
            break;
        }
        walked += d;
        pts.push(p);
        prev = p;
    }
    let span = super::outline::bounds(pts.iter().map(|&(x, y)| (x, y, 0.0, 0.0)));
    span.map_or((0.0, 0.0, 0.0, 0.0), |s| inflate(s.to_rect(), pad))
}

pub(super) fn place_labels(
    plans: &mut [RelPlan],
    placement: &Placement,
    rel_styles: &HashMap<(&str, &str), &C4RelStyle>,
) {
    // One spatial index for every label: shapes, headers, frame borders,
    // every relation's line (tagged with its plan index) and, as they are
    // placed, the labels themselves.
    let mut obstacles = Obstacles::default();
    for (_, r) in &placement.leaves {
        obstacles.add_rect(inflate(*r, SHAPE_CLEARANCE));
    }
    for b in &placement.boundaries {
        let (x, y, w, h) = b.rect;
        obstacles.add_rect((x, y, w, BOUNDARY_HEADER_H));
        // A label must not straddle a boundary's dashed border either.
        let t = FRAME_EDGE_CLEARANCE;
        for edge in [
            (x - t, y - t, w + 2.0 * t, 2.0 * t),
            (x - t, y + h - t, w + 2.0 * t, 2.0 * t),
            (x - t, y - t, 2.0 * t, h + 2.0 * t),
            (x + w - t, y - t, 2.0 * t, h + 2.0 * t),
        ] {
            obstacles.add_firm_rect(edge);
        }
    }
    for (j, p) in plans.iter().enumerate() {
        for w in p.line.windows(2) {
            obstacles.add_segment((w[0], w[1]), Some(j));
        }
        // The arrowhead end of every line stays clear of labels, so a label
        // never hides where a line meets its shape.
        obstacles.add_rect(end_zone(p.line.iter().rev(), ARROWHEAD, ARROWHEAD_PAD));
        obstacles.add_firm_rect(end_zone(p.line.iter().rev(), ARROW_ZONE, SHAPE_CLEARANCE));
        if p.rel.bidirectional {
            obstacles.add_rect(end_zone(p.line.iter(), ARROWHEAD, ARROWHEAD_PAD));
            obstacles.add_firm_rect(end_zone(p.line.iter(), ARROW_ZONE, SHAPE_CLEARANCE));
        }
    }
    for i in 0..plans.len() {
        let Some(label) = RelLabel::new(plans[i].rel, placement.narrow_labels) else {
            continue;
        };
        let r = plans[i].rel;
        // Relations between the same two elements run side by side by
        // construction; they are kept apart via their label rects instead.
        let foreign = |owner: Owner| match owner {
            None => true,
            Some(j) => {
                let o = plans[j].rel;
                j != i
                    && !((o.from == r.from && o.to == r.to) || (o.from == r.to && o.to == r.from))
            }
        };
        let spot = place_label(&plans[i].line, (label.w, label.h), &obstacles, &foreign);
        let (mut mx, mut my) = spot.centre;
        let mut leader = spot.leader;
        if let Some(s) = rel_styles.get(&r.written_pair()) {
            let (dx, dy) = (s.offset_x.unwrap_or(0.0), s.offset_y.unwrap_or(0.0));
            mx += dx;
            my += dy;
            // The leader still starts on the label's edge.
            if let Some((on_label, _)) = &mut leader {
                *on_label = (on_label.0 + dx, on_label.1 + dy);
            }
        }
        let rect = rect_at((mx, my), (label.w, label.h));
        obstacles.add_rect(inflate(rect, LABEL_CLEARANCE));
        plans[i].label = Some((label, (mx, my)));
        plans[i].leader = leader;
    }
}

#[cfg(test)]
mod zone_tests {
    use super::*;

    #[test]
    fn end_zone_covers_only_the_line_end() {
        // A long straight line: the zone is the last 14px, not the line.
        let line = [(0.0, 0.0), (300.0, 0.0)];
        let z = end_zone(line.iter().rev(), 14.0, 6.0);
        assert_eq!(z, (280.0, -6.0, 26.0, 12.0));
        let short = end_zone(line.iter(), 14.0, 0.0);
        assert_eq!(short, (0.0, 0.0, 14.0, 0.0));
    }
}
