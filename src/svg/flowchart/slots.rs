//! Side-by-side slots for different end kinds on one node face (#402).

use std::collections::BTreeMap;

use crate::parse::EdgeHead;

use super::edges::clip_end;
use super::route::{axes, is_round, Approach, EdgeIn};

/// Gap between two neighbouring slots, beyond the widest marker.
const SLOT_MARGIN: f64 = 4.0;

/// One end on a face: edge index, which end, its kind, the cross coordinate
/// it comes from, and whether it is a plain-route end (it keeps its place but
/// still reserves a slot so the others move clear of it).
type End<'e, 'a> = (usize, usize, &'e EdgeIn<'a>, u32, f64, bool);

/// Each end's cross-axis offset on its face. Ends of one kind (the same
/// marker, or bare) coincide, as upstream draws them; different kinds sit side
/// by side, centred on the face and ordered by where their edges come from so
/// they do not cross. Kinds that do not fit on the face all stay centred.
pub(super) fn slot_offsets(ins: &[Option<EdgeIn>], aps: &[[Option<Approach>; 2]]) -> Vec<[f64; 2]> {
    let mut offs = vec![[0.0; 2]; ins.len()];
    let mut faces: BTreeMap<(&str, usize, bool), Vec<End>> = BTreeMap::new();
    for (i, (e, ap)) in ins.iter().zip(aps).enumerate() {
        let Some(e) = e else { continue };
        for (which, a) in ap.iter().enumerate() {
            let id = if which == 0 { &e.edge.from } else { &e.edge.to };
            let (f, c) = e.axes();
            if let Some(a) = a {
                let end = (i, which, e, kind(e.head(which)), a.from_cross, false);
                faces.entry((id, f, a.side)).or_default().push(end);
            } else if let Some((side, cross)) = plain_landing(e, which, f, c) {
                // A unique kind per plain end: it never merges with a slot.
                let end = (i, which, e, 4 + 2 * i as u32 + which as u32, cross, true);
                faces.entry((id, f, side)).or_default().push(end);
            }
        }
    }
    for ((_, f, _), ends) in &faces {
        // (kind, sum of source cross coordinates, count, widest marker)
        let mut kinds: Vec<(u32, f64, f64, f64)> = Vec::new();
        for &(_, which, e, k, from, _) in ends {
            let w = marker_width(e.head(which)) * e.stroke();
            match kinds.iter_mut().find(|kk| kk.0 == k) {
                Some(slot) => *slot = (k, slot.1 + from, slot.2 + 1.0, slot.3.max(w)),
                None => kinds.push((k, from, 1.0, w)),
            }
        }
        if kinds.len() < 2 || ends.iter().all(|end| end.5) {
            continue;
        }
        kinds.sort_by(|x, y| (x.1 / x.2).total_cmp(&(y.1 / y.2)).then(x.0.cmp(&y.0)));
        // Slot centres along the face: neighbours sit half of each one's
        // marker width plus the margin apart.
        let mut at = vec![0.0];
        for pair in kinds.windows(2) {
            at.push(at[at.len() - 1] + (pair[0].3 + pair[1].3) / 2.0 + SLOT_MARGIN);
        }
        let (_, which, e, ..) = ends[0];
        let node = e.node(which);
        // A round end's head sits on the curve, so only the slot centres need
        // to fit; a rect end's whole head stays on the face.
        let heads = if node.shape.is_some_and(is_round) {
            0.0
        } else {
            (kinds[0].3 + kinds[kinds.len() - 1].3) / 2.0
        };
        let span = at[at.len() - 1];
        if span + heads > axes(node.size)[1 - f] {
            continue;
        }
        for &(i, which, _, k, _, fixed) in ends {
            let rank = kinds.iter().position(|kk| kk.0 == k).unwrap_or(0);
            if !fixed {
                offs[i][which] = at[rank] - span / 2.0;
            }
        }
    }
    offs
}

/// Where a plain-route end lands: the flow face it hits (`true`: toward +`f`)
/// and its cross coordinate, or `None` on a side face or a cluster.
fn plain_landing(e: &EdgeIn, which: usize, f: usize, c: usize) -> Option<(bool, f64)> {
    let node = e.node(which);
    node.shape?;
    let pts = &e.geom.2;
    let toward = if which == 0 {
        pts[1]
    } else {
        pts[pts.len() - 2]
    };
    let tip = axes(clip_end(toward, node));
    let rel = tip[f] - node.center_on(f);
    (rel.abs() >= axes(node.size)[f] / 2.0 - 0.5).then_some((rel > 0.0, tip[c]))
}

fn kind(head: EdgeHead) -> u32 {
    match head {
        EdgeHead::Arrow => 1,
        EdgeHead::Circle => 2,
        EdgeHead::Cross => 3,
        _ => 0,
    }
}

/// A marker's width across the edge per unit of stroke width, from the defs
/// in `edges.rs`.
fn marker_width(head: EdgeHead) -> f64 {
    match head {
        EdgeHead::Arrow | EdgeHead::Circle => 12.0,
        EdgeHead::Cross => 10.0,
        _ => 0.0,
    }
}
