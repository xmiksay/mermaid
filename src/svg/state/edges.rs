//! Transition rendering: the curved connector between two states, its label,
//! and clipping each end to the boundary of the shape it touches.

use std::collections::HashMap;

use crate::parse::{State, StateKind, StateTransition};
use crate::sugiyama::NodeId;
use crate::svg::builder::{curve_basis_path, SvgBuilder};
use crate::svg::geometry::{clip_circle, clip_polyline, clip_rect, clip_rhombus};
use crate::svg::theme::Theme;

use super::composite::endpoint_clip;
use super::{StateEndClip, CHOICE_H, CHOICE_W, PSEUDO_R};

/// A transition's polyline, clipped to the shape each end touches. Real
/// node→node transitions keep their routed polyline; an endpoint that is a
/// composite cluster has no layout route, so it is a straight connector
/// clipped to the cluster box. `None` for an unknown endpoint.
#[allow(clippy::too_many_arguments)]
pub(super) fn transition_line(
    tr: &StateTransition,
    id_to_u32: &HashMap<String, NodeId>,
    states: &[State],
    sizes: &[(f64, f64)],
    pos: &HashMap<NodeId, (f64, f64)>,
    boxes: &HashMap<String, (f64, f64, f64, f64)>,
    routed: &HashMap<(NodeId, NodeId), Vec<(f64, f64)>>,
) -> Option<Vec<(f64, f64)>> {
    let start = endpoint_clip(&tr.from, id_to_u32, states, sizes, pos, boxes)?;
    let end = endpoint_clip(&tr.to, id_to_u32, states, sizes, pos, boxes)?;
    let straight = vec![start.center, end.center];
    let pts = match (id_to_u32.get(&tr.from), id_to_u32.get(&tr.to)) {
        (Some(u), Some(v)) => routed.get(&(*u, *v)).unwrap_or(&straight),
        _ => &straight,
    };
    Some(clip_polyline(
        pts,
        |p| clip_end(p, &start),
        |p| clip_end(p, &end),
    ))
}

/// Draws a transition's clipped polyline.
pub(super) fn draw_transition(svg: &mut SvgBuilder, clipped: &[(f64, f64)], theme: &Theme) {
    let flow_edge_stroke = &theme.flow_edge_stroke;
    svg.path(
        &curve_basis_path(clipped),
        &format!(
            "fill=\"none\" stroke=\"{flow_edge_stroke}\" stroke-width=\"1.5\" \
             marker-end=\"url(#state-arrow)\""
        ),
    );
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
