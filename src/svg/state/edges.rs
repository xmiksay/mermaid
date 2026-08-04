//! Transition rendering: the curved connector between two states, its label,
//! and clipping each end to the boundary of the shape it touches.

use crate::parse::StateKind;
use crate::svg::builder::{curve_basis_path, SvgBuilder};
use crate::svg::geometry::{clip_circle, clip_rect, clip_rhombus};
use crate::svg::theme::Theme;

use super::{StateEndClip, CHOICE_H, CHOICE_W, PSEUDO_R};

/// Draws the transition's path and returns its clipped polyline, so the
/// caller can collect a label slot (if any) and resolve every transition
/// label's position in one pass after the loop.
pub(super) fn draw_transition(
    svg: &mut SvgBuilder,
    pts: &[(f64, f64)],
    start: &StateEndClip,
    end: &StateEndClip,
    theme: &Theme,
) -> Vec<(f64, f64)> {
    let flow_edge_stroke = &theme.flow_edge_stroke;
    let n = pts.len();
    if n < 2 {
        return Vec::new();
    }

    let first = clip_end(pts[1], start);
    let last = clip_end(pts[n - 2], end);

    let mut clipped = Vec::with_capacity(n);
    clipped.push(first);
    for p in &pts[1..n - 1] {
        clipped.push(*p);
    }
    clipped.push(last);

    let d = curve_basis_path(&clipped);
    svg.path(
        &d,
        &format!(
            "fill=\"none\" stroke=\"{flow_edge_stroke}\" stroke-width=\"1.5\" \
             marker-end=\"url(#state-arrow)\""
        ),
    );
    clipped
}

fn clip_end(from: (f64, f64), clip: &StateEndClip) -> (f64, f64) {
    match clip.kind {
        Some(kind) => clip_to_state(from, clip.center, clip.size, kind),
        None => clip_rect(from, clip.center, clip.size),
    }
}

fn clip_to_state(
    from: (f64, f64),
    center: (f64, f64),
    size: (f64, f64),
    kind: StateKind,
) -> (f64, f64) {
    match kind {
        StateKind::Start | StateKind::End | StateKind::History { .. } => {
            clip_circle(from, center, PSEUDO_R)
        }
        StateKind::Choice => clip_rhombus(from, center, (CHOICE_W, CHOICE_H)),
        _ => clip_rect(from, center, size),
    }
}
