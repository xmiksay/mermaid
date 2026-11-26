//! Side-by-side slots for different end kinds on one node face (#402).

use std::collections::BTreeMap;

use crate::parse::EdgeHead;

use super::edges::clip_end;
use super::round::is_round;
use super::route::{axes, Approach, EdgeIn};

/// Gap between two neighbouring slots, beyond the widest marker.
const SLOT_MARGIN: f64 = 4.0;
/// Slot centres stay this far inside a rect face's corners.
const CORNER_INSET: f64 = 4.0;
/// Slot centres on a round outline stay within this share of its
/// half-extent, where the ray to the centre still meets it cleanly.
pub(super) const ROUND_REACH: f64 = 0.6;

/// One end on a face: edge index, which end, its kind, a cross coordinate,
/// and whether it is fixed. A movable end's coordinate is where its edge
/// comes from (orders the slots); a fixed end — a plain-route or straight end
/// that keeps its own place — carries where it lands, relative to the node.
type End<'e, 'a> = (usize, usize, &'e EdgeIn<'a>, u32, f64, bool);

/// Each end's cross-axis offset on its face. Ends of one kind (the same
/// marker, or bare) coincide, as upstream draws them; different kinds sit side
/// by side, centred on the face and ordered by where their edges come from so
/// they do not cross. Kinds are never stacked: a face too narrow compresses
/// the spacing, down to half a marker, then lets the row overrun it evenly.
/// Fixed ends take no slot; the row only moves aside where it would land on
/// one, so a lone movable end stays centred.
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
                let end = (i, which, e, kind(e.head(which)), cross, true);
                faces.entry((id, f, side)).or_default().push(end);
            }
        }
    }
    for ((_, f, _), ends) in &faces {
        let width = |&(_, which, e, ..): &End| (marker_width(e.head(which)) * e.stroke()).min(1e4);
        // Movable kinds: (kind, sum of source cross coordinates, count, widest)
        let mut kinds: Vec<(u32, f64, f64, f64)> = Vec::new();
        for end in ends.iter().filter(|end| !end.5) {
            let (k, from, w) = (end.3, end.4, width(end));
            match kinds.iter_mut().find(|kk| kk.0 == k) {
                Some(slot) => *slot = (k, slot.1 + from, slot.2 + 1.0, slot.3.max(w)),
                None => kinds.push((k, from, 1.0, w)),
            }
        }
        let fixed: Vec<(f64, f64)> = ends
            .iter()
            .filter(|end| end.5)
            .map(|end| (end.4, width(end)))
            .collect();
        if kinds.is_empty() || (kinds.len() < 2 && fixed.is_empty()) {
            continue;
        }
        kinds.sort_by(|x, y| (x.1 / x.2).total_cmp(&(y.1 / y.2)).then(x.0.cmp(&y.0)));
        let (_, which, e, ..) = ends[0];
        let node = e.node(which);
        let (round, face) = (node.shape.is_some_and(is_round), axes(node.size)[1 - f]);
        let at = row(&kinds, round, face);
        let shift = clear_fixed(&kinds, &at, &fixed, reach(round, face));
        for &(i, which, _, k, _, is_fixed) in ends {
            if let (false, Some(rank)) = (is_fixed, kinds.iter().position(|kk| kk.0 == k)) {
                offs[i][which] = at[rank] + shift;
            }
        }
    }
    offs
}

/// Slot centres for the sorted `kinds`, centred on a `face`-long face.
/// Neighbours sit half of each one's marker width plus the margin apart,
/// compressed as far as needed to keep every slot within `reach` of the face
/// centre: markers wider than the face overlap rather than leave their node.
fn row(kinds: &[(u32, f64, f64, f64)], round: bool, face: f64) -> Vec<f64> {
    let gap = |p: &[(u32, f64, f64, f64)]| (p[0].3 + p[1].3) / 2.0 + SLOT_MARGIN;
    let full: f64 = kinds.windows(2).map(gap).sum();
    let span = 2.0 * reach(round, face);
    let k = if full > span { span / full } else { 1.0 };
    let mut at = vec![0.0];
    for p in kinds.windows(2) {
        at.push(at[at.len() - 1] + gap(p) * k);
    }
    let mid = at[at.len() - 1] / 2.0;
    at.iter().map(|x| x - mid).collect()
}

/// How far a slot centre may sit from the face centre: inside the corners
/// on a rect, within the part of a round outline its ray can still reach
/// cleanly.
fn reach(round: bool, face: f64) -> f64 {
    if round {
        ROUND_REACH * face / 2.0
    } else {
        (face / 2.0 - CORNER_INSET).max(0.0)
    }
}

/// How far to move the row so no slot lands on a fixed end: for each fixed
/// end a slot would touch (within both half-widths plus the margin), the row
/// moves to the side away from it, just far enough — but never so far that
/// a slot leaves `reach` of the face centre.
fn clear_fixed(
    kinds: &[(u32, f64, f64, f64)],
    at: &[f64],
    fixed: &[(f64, f64)],
    reach: f64,
) -> f64 {
    let mut shift = 0.0;
    for &(x, w) in fixed {
        let clear = |i: usize| (kinds[i].3 + w) / 2.0 + SLOT_MARGIN;
        if !(0..at.len()).any(|i| (at[i] + shift - x).abs() < clear(i)) {
            continue;
        }
        shift = if x >= shift {
            (0..at.len())
                .map(|i| x - clear(i) - at[i])
                .fold(shift, f64::min)
        } else {
            (0..at.len())
                .map(|i| x + clear(i) - at[i])
                .fold(shift, f64::max)
        };
    }
    let (lo, hi) = (-reach - at[0], reach - at[at.len() - 1]);
    shift.max(lo.min(0.0)).min(hi.max(0.0))
}

/// Where an end without an approach point lands: the flow face it hits
/// (`true`: toward +`f`) and its cross offset from the node centre, or `None`
/// on a side face or a cluster.
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
    let (rel, cross) = (tip[f] - node.center_on(f), tip[c] - node.center_on(c));
    // On the flow face (or that side of a round outline) when the landing is
    // nearer the flow axis than the cross axis, relative to the node's size.
    let (hf, hc) = (axes(node.size)[f] / 2.0, axes(node.size)[c] / 2.0);
    (rel.abs() * hc >= cross.abs() * hf).then_some((rel > 0.0, cross))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `(kind, source sum, count, width)` rows with the given widths.
    fn kinds(widths: &[f64]) -> Vec<(u32, f64, f64, f64)> {
        widths
            .iter()
            .enumerate()
            .map(|(i, &w)| (i as u32, 0.0, 1.0, w))
            .collect()
    }

    #[test]
    fn row_spaces_neighbours_by_their_widths_centred() {
        // Arrow (18) and bare (0): 9 + 4 apart, centred.
        assert_eq!(row(&kinds(&[18.0, 0.0]), false, 200.0), vec![-6.5, 6.5]);
    }

    #[test]
    fn row_compresses_but_never_leaves_the_face() {
        // Three 18px heads on a 44px face: 22px gaps shrink to 18 so every
        // slot stays 4px inside the corners.
        let at = row(&kinds(&[18.0, 18.0, 18.0]), false, 44.0);
        assert_eq!(at, vec![-18.0, 0.0, 18.0]);
        // A 360px marker on a 60px face overlaps, but its slot stays inside.
        let at = row(&kinds(&[360.0, 18.0]), false, 60.0);
        assert!(
            at.iter().all(|x| x.abs() <= 30.0 - CORNER_INSET + 1e-9),
            "{at:?}"
        );
        assert!(at[1] > at[0], "never stacked");
        // Round: slot centres within 0.6 of the radius.
        let at = row(&kinds(&[18.0, 18.0, 18.0]), true, 40.0);
        assert!(at.iter().all(|x| x.abs() <= 12.0 + 1e-9), "{at:?}");
    }

    #[test]
    fn clear_fixed_moves_only_when_a_slot_lands_on_a_fixed_end() {
        let k = kinds(&[18.0]);
        // A bare fixed end 20 away: clear (9 + 4 = 13), no move.
        assert_eq!(clear_fixed(&k, &[0.0], &[(20.0, 0.0)], 26.0), 0.0);
        // A fixed arrow tip right at the centre: move 9 + 9 + 4 = 22 away.
        assert_eq!(clear_fixed(&k, &[0.0], &[(0.0, 18.0)], 26.0), -22.0);
        // ...but never past the face's reach.
        assert_eq!(clear_fixed(&k, &[0.0], &[(0.0, 18.0)], 10.0), -10.0);
    }
}
