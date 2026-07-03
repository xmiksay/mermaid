//! Composite-cluster layout for state diagrams: frame boxes around the
//! members of a `state X { … }` block, transition-endpoint clipping to those
//! boxes, and the attached state notes.

use std::collections::HashMap;

use crate::parse::{CompositeState, NotePosition, State, StateDiagram, StateKind, StateNote};
use crate::sugiyama::NodeId;

use crate::svg::builder::SvgBuilder;
use crate::svg::theme::Theme;

pub(super) const FRAME_PAD: f64 = 14.0;
const FRAME_HEADER: f64 = 18.0;

/// Clip target for one end of a transition: the shape boundary a connector
/// stops at. `kind` is `None` for a composite cluster box (clipped as a rect).
pub(super) struct StateEndClip {
    pub(super) center: (f64, f64),
    pub(super) size: (f64, f64),
    pub(super) kind: Option<StateKind>,
}

/// Resolve a transition endpoint id to its clip target — a laid-out state's
/// boundary, or the bounding box of the composite it names.
pub(super) fn endpoint_clip(
    id: &str,
    id_to_u32: &HashMap<String, NodeId>,
    states: &[State],
    sizes: &[(f64, f64)],
    pos: &HashMap<NodeId, (f64, f64)>,
    boxes: &HashMap<String, (f64, f64, f64, f64)>,
) -> Option<StateEndClip> {
    if let Some(&u) = id_to_u32.get(id) {
        if let Some(&center) = pos.get(&u) {
            return Some(StateEndClip {
                center,
                size: sizes[u as usize],
                kind: Some(states[u as usize].kind),
            });
        }
    }
    let &(x0, y0, x1, y1) = boxes.get(id)?;
    Some(StateEndClip {
        center: ((x0 + x1) / 2.0, (y0 + y1) / 2.0),
        size: (x1 - x0, y1 - y0),
        kind: None,
    })
}

/// Screen-space bounding box `(x0, y0, x1, y1)` of every composite, keyed by id.
/// A box spans all member states gathered recursively through nested composites,
/// with room above for the title.
pub(super) fn compute_composite_boxes(
    d: &StateDiagram,
    id_to_u32: &HashMap<String, NodeId>,
    pos: &HashMap<NodeId, (f64, f64)>,
    sizes: &[(f64, f64)],
) -> HashMap<String, (f64, f64, f64, f64)> {
    let by_id: HashMap<&str, &CompositeState> =
        d.composites.iter().map(|c| (c.id.as_str(), c)).collect();
    let mut boxes = HashMap::new();
    for comp in &d.composites {
        let mut members: Vec<&str> = Vec::new();
        collect_member_ids(comp, &by_id, &mut members);
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for id in &members {
            let Some(&u) = id_to_u32.get(*id) else {
                continue;
            };
            let Some(&(cx, cy)) = pos.get(&u) else {
                continue;
            };
            let (w, h) = sizes[u as usize];
            min_x = min_x.min(cx - w / 2.0);
            max_x = max_x.max(cx + w / 2.0);
            min_y = min_y.min(cy - h / 2.0);
            max_y = max_y.max(cy + h / 2.0);
        }
        if !min_x.is_finite() {
            continue;
        }
        boxes.insert(
            comp.id.clone(),
            (
                min_x - FRAME_PAD,
                min_y - FRAME_PAD - FRAME_HEADER,
                max_x + FRAME_PAD,
                max_y + FRAME_PAD,
            ),
        );
    }
    boxes
}

fn collect_member_ids<'a>(
    comp: &'a CompositeState,
    by_id: &HashMap<&str, &'a CompositeState>,
    out: &mut Vec<&'a str>,
) {
    for region in &comp.regions {
        for child in region {
            out.push(child.as_str());
            if let Some(child_comp) = by_id.get(child.as_str()) {
                collect_member_ids(child_comp, by_id, out);
            }
        }
    }
}

pub(super) fn draw_composites(
    svg: &mut SvgBuilder,
    d: &StateDiagram,
    boxes: &HashMap<String, (f64, f64, f64, f64)>,
    theme: &Theme,
) {
    let fg = theme.fg;
    for comp in &d.composites {
        let Some(&(x0, y0, x1, y1)) = boxes.get(&comp.id) else {
            continue;
        };
        svg.rect(
            x0,
            y0,
            x1 - x0,
            y1 - y0,
            "fill=\"none\" stroke=\"#999\" stroke-width=\"1\" rx=\"10\" stroke-dasharray=\"5 3\"",
        );
        let label = d
            .states
            .iter()
            .find(|s| s.id == comp.id)
            .map(|s| s.label.as_str())
            .filter(|l| !l.is_empty())
            .unwrap_or(comp.id.as_str());
        svg.text(
            x0 + 10.0,
            y0 + 14.0,
            &format!("fill=\"{fg}\" font-size=\"12\" font-weight=\"bold\""),
            label,
        );
        // Divider under the title bar, matching upstream's composite header.
        svg.line(
            x0,
            y0 + 20.0,
            x1,
            y0 + 20.0,
            "stroke=\"#999\" stroke-width=\"1\"",
        );
    }
}

pub(super) fn draw_state_note(
    svg: &mut SvgBuilder,
    note: &StateNote,
    id_to_u32: &HashMap<String, NodeId>,
    sizes: &[(f64, f64)],
    pos: &HashMap<NodeId, (f64, f64)>,
    boxes: &HashMap<String, (f64, f64, f64, f64)>,
    theme: &Theme,
) {
    let fg = theme.fg;
    let laid_out = id_to_u32
        .get(&note.target)
        .and_then(|u| pos.get(u).map(|p| (*u, *p)));
    let (cx, cy, w, h) = if let Some((u, (cx, cy))) = laid_out {
        let (w, h) = sizes[u as usize];
        (cx, cy, w, h)
    } else if let Some(&(x0, y0, x1, y1)) = boxes.get(&note.target) {
        ((x0 + x1) / 2.0, (y0 + y1) / 2.0, x1 - x0, y1 - y0)
    } else {
        return;
    };
    let chars = note.text.chars().count() as f64;
    let nw = (chars * 7.0 + 20.0).max(80.0);
    let nh = 32.0;
    let (nx, ny) = match note.position {
        NotePosition::RightOf => (cx + w / 2.0 + 14.0, cy - nh / 2.0),
        NotePosition::LeftOf => (cx - w / 2.0 - 14.0 - nw, cy - nh / 2.0),
        NotePosition::Over => (cx - nw / 2.0, cy - h / 2.0 - nh - 8.0),
    };
    svg.rect(
        nx,
        ny,
        nw,
        nh,
        "fill=\"#FFF5AD\" stroke=\"#aaaa33\" stroke-width=\"1\"",
    );
    svg.text(
        nx + nw / 2.0,
        ny + nh / 2.0 + 4.0,
        &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\""),
        &note.text,
    );
}
