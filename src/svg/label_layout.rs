//! Shared pairwise edge-label overlap avoidance, used by every graph-shaped
//! renderer (flowchart, state, class, ER, block, requirement, architecture)
//! and sequence diagrams. Generalizes the state diagram's original
//! opposite-pair label stagger (#312) into a mechanism that works for any two
//! colliding labels, not just a reversed `A→B`/`B→A` pair.

use super::geometry::{polyline_length, polyline_midpoint, polyline_midpoint_offset};

/// One labeled edge's clipped path and the on-screen size of the label drawn
/// along it, as collected by a renderer's edge loop.
pub(super) struct LabelSlot {
    pub polyline: Vec<(f64, f64)>,
    pub size: (f64, f64),
}

/// Arc-length a colliding label is nudged per pass, along its own edge —
/// matches the original state-diagram stagger distance (#312).
pub(super) const LABEL_STAGGER_STEP: f64 = 15.0;
const MAX_PASSES: usize = 4;
/// A label may drift at most this many [`LABEL_STAGGER_STEP`]s from its
/// natural midpoint...
const MAX_OFFSET_STEPS: f64 = 4.0;
/// ...or this fraction of its own edge's arc length, whichever is smaller —
/// so a short edge gives up gracefully instead of sliding a label off its end.
const MAX_OFFSET_FRACTION: f64 = 0.4;

/// Overlap area of two `(x, y, w, h)` rects (top-left corner + size), `0.0` if
/// they don't intersect. Shared by every renderer that needs to test whether
/// a label's opaque background would occlude something else.
pub(super) fn rect_overlap_area(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> f64 {
    let x_overlap = (a.0 + a.2).min(b.0 + b.2) - a.0.max(b.0);
    let y_overlap = (a.1 + a.3).min(b.1 + b.3) - a.1.max(b.1);
    if x_overlap > 0.0 && y_overlap > 0.0 {
        x_overlap * y_overlap
    } else {
        0.0
    }
}

/// The `(x, y, w, h)` rect a `size`-sized label centered at `center` occupies.
fn rect_at(center: (f64, f64), size: (f64, f64)) -> (f64, f64, f64, f64) {
    (
        center.0 - size.0 / 2.0,
        center.1 - size.1 / 2.0,
        size.0,
        size.1,
    )
}

/// Resolve a draw position for each of `slots`, nudging any pair whose label
/// rects would overlap apart along their own edges.
///
/// Algorithm: start every label at its polyline's arc-length midpoint. For up
/// to [`MAX_PASSES`] passes, scan every pair `(i, j)` in input order; if their
/// current rects overlap, pull `j`'s label back toward its own source by
/// [`LABEL_STAGGER_STEP`] (clamped to `MAX_OFFSET_STEPS` steps or
/// `MAX_OFFSET_FRACTION` of its arc length, whichever is smaller). If `j` has
/// no room left, try nudging `i` forward under the same clamp instead. If
/// neither can move, leave both as-is and continue — one stuck pair must
/// never block another pair's resolution. Stop early once a full pass makes
/// no move.
///
/// This is a heuristic, not a globally optimal placement for a 3-or-more-way
/// cluster of overlapping labels: pairs are re-scanned every pass so a chain
/// of pushes eventually gets caught, but no joint optimum is computed. When
/// nothing collides this is a strict no-op — every returned point is exactly
/// [`polyline_midpoint`].
pub(super) fn layout_edge_labels(slots: &[LabelSlot]) -> Vec<(f64, f64)> {
    let n = slots.len();
    let arc_len: Vec<f64> = slots.iter().map(|s| polyline_length(&s.polyline)).collect();
    let mut offset = vec![0.0_f64; n];
    let mut pos: Vec<(f64, f64)> = slots
        .iter()
        .map(|s| polyline_midpoint(&s.polyline))
        .collect();

    for _ in 0..MAX_PASSES {
        let mut moved = false;
        for i in 0..n {
            for j in (i + 1)..n {
                let ri = rect_at(pos[i], slots[i].size);
                let rj = rect_at(pos[j], slots[j].size);
                if rect_overlap_area(ri, rj) <= 0.0 {
                    continue;
                }
                let limit_j =
                    (LABEL_STAGGER_STEP * MAX_OFFSET_STEPS).min(MAX_OFFSET_FRACTION * arc_len[j]);
                if offset[j] - LABEL_STAGGER_STEP >= -limit_j {
                    offset[j] -= LABEL_STAGGER_STEP;
                    pos[j] = polyline_midpoint_offset(&slots[j].polyline, offset[j]);
                    moved = true;
                    continue;
                }
                let limit_i =
                    (LABEL_STAGGER_STEP * MAX_OFFSET_STEPS).min(MAX_OFFSET_FRACTION * arc_len[i]);
                if offset[i] + LABEL_STAGGER_STEP <= limit_i {
                    offset[i] += LABEL_STAGGER_STEP;
                    pos[i] = polyline_midpoint_offset(&slots[i].polyline, offset[i]);
                    moved = true;
                }
                // Neither has room: leave both where they are, but keep
                // scanning — an unrelated pair further down must still get a
                // chance to resolve this pass.
            }
        }
        if !moved {
            break;
        }
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(polyline: Vec<(f64, f64)>, size: (f64, f64)) -> LabelSlot {
        LabelSlot { polyline, size }
    }

    #[test]
    fn rect_overlap_area_zero_for_disjoint_rects() {
        assert_eq!(
            rect_overlap_area((0.0, 0.0, 10.0, 10.0), (20.0, 20.0, 10.0, 10.0)),
            0.0
        );
    }

    #[test]
    fn rect_overlap_area_positive_for_overlapping_rects() {
        assert_eq!(
            rect_overlap_area((0.0, 0.0, 10.0, 10.0), (5.0, 5.0, 10.0, 10.0)),
            25.0
        );
    }

    #[test]
    fn layout_edge_labels_leaves_isolated_labels_at_midpoint() {
        // Three widely separated horizontal edges: nothing collides, so every
        // returned position must be exactly the natural midpoint (strict
        // no-op invariant).
        let slots = vec![
            slot(vec![(0.0, 0.0), (100.0, 0.0)], (40.0, 18.0)),
            slot(vec![(0.0, 200.0), (100.0, 200.0)], (40.0, 18.0)),
            slot(vec![(0.0, 400.0), (100.0, 400.0)], (40.0, 18.0)),
        ];
        let want: Vec<(f64, f64)> = slots
            .iter()
            .map(|s| polyline_midpoint(&s.polyline))
            .collect();
        let got = layout_edge_labels(&slots);
        assert_eq!(got, want);
    }

    #[test]
    fn layout_edge_labels_separates_two_overlapping_labels() {
        // Two horizontal edges on the same row: natural midpoints coincide,
        // so the labels must be nudged apart along their own edges.
        let slots = vec![
            slot(vec![(0.0, 0.0), (100.0, 0.0)], (40.0, 18.0)),
            slot(vec![(0.0, 0.0), (100.0, 0.0)], (40.0, 18.0)),
        ];
        let got = layout_edge_labels(&slots);
        let r0 = rect_at(got[0], slots[0].size);
        let r1 = rect_at(got[1], slots[1].size);
        assert_eq!(
            rect_overlap_area(r0, r1),
            0.0,
            "labels still overlap: {got:?}"
        );
    }

    #[test]
    fn layout_edge_labels_handles_three_way_cluster() {
        // Three edges all sharing (roughly) the same midpoint — a transitive
        // chain of pushes must clear every pairwise overlap.
        let slots = vec![
            slot(vec![(0.0, 0.0), (200.0, 0.0)], (20.0, 18.0)),
            slot(vec![(0.0, 0.0), (200.0, 0.0)], (20.0, 18.0)),
            slot(vec![(0.0, 0.0), (200.0, 0.0)], (20.0, 18.0)),
        ];
        let got = layout_edge_labels(&slots);
        for i in 0..got.len() {
            for j in (i + 1)..got.len() {
                let ri = rect_at(got[i], slots[i].size);
                let rj = rect_at(got[j], slots[j].size);
                assert_eq!(
                    rect_overlap_area(ri, rj),
                    0.0,
                    "labels {i} and {j} still overlap: {got:?}"
                );
            }
        }
    }

    #[test]
    fn layout_edge_labels_gives_up_gracefully_on_a_too_short_edge() {
        // A pair of near-zero-length edges: there is no room to nudge either
        // label clear (their arc length caps the offset at ~0), so the
        // algorithm must terminate without panicking or looping forever,
        // simply returning the best it could do.
        let slots = vec![
            slot(vec![(0.0, 0.0), (1.0, 0.0)], (40.0, 18.0)),
            slot(vec![(0.0, 0.0), (1.0, 0.0)], (40.0, 18.0)),
        ];
        let got = layout_edge_labels(&slots);
        assert!(got[0].0.is_finite() && got[0].1.is_finite());
        assert!(got[1].0.is_finite() && got[1].1.is_finite());
    }
}
