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
/// Height of the filled title band drawn at the top of a composite frame.
const TITLE_BAND: f64 = 24.0;
/// Vertical gap between two stacked parallel regions inside a composite.
const REGION_GAP: f64 = 24.0;

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

/// Screen-space bounding box `(x0, y0, x1, y1)` of every composite, keyed by
/// id. Computed bottom-up (nested composites first): a composite's box comes
/// from its own IMMEDIATE region members, and a member that is itself a
/// composite contributes its already-computed frame box wholesale rather than
/// its descendant states directly. That way each nesting level's own title
/// band reserves its own headroom instead of being swallowed by an ancestor
/// measuring straight through to the deepest leaf states (#see nested title
/// band overlap).
pub(super) fn compute_composite_boxes(
    d: &StateDiagram,
    id_to_u32: &HashMap<String, NodeId>,
    pos: &HashMap<NodeId, (f64, f64)>,
    sizes: &[(f64, f64)],
) -> HashMap<String, (f64, f64, f64, f64)> {
    let by_id: HashMap<&str, &CompositeState> =
        d.composites.iter().map(|c| (c.id.as_str(), c)).collect();
    let mut boxes: HashMap<String, (f64, f64, f64, f64)> = HashMap::new();
    for comp in &d.composites {
        composite_box(comp, &by_id, id_to_u32, pos, sizes, &mut boxes);
    }
    boxes
}

/// Compute (and memoize into `boxes`) one composite's frame box, recursing
/// into a nested composite child so its box is available before it is folded
/// into the parent's extent.
fn composite_box(
    comp: &CompositeState,
    by_id: &HashMap<&str, &CompositeState>,
    id_to_u32: &HashMap<String, NodeId>,
    pos: &HashMap<NodeId, (f64, f64)>,
    sizes: &[(f64, f64)],
    boxes: &mut HashMap<String, (f64, f64, f64, f64)>,
) -> Option<(f64, f64, f64, f64)> {
    if let Some(&b) = boxes.get(&comp.id) {
        return Some(b);
    }
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for region in &comp.regions {
        for child in region {
            if let Some(&child_comp) = by_id.get(child.as_str()) {
                let Some((x0, y0, x1, y1)) =
                    composite_box(child_comp, by_id, id_to_u32, pos, sizes, boxes)
                else {
                    continue;
                };
                min_x = min_x.min(x0);
                min_y = min_y.min(y0);
                max_x = max_x.max(x1);
                max_y = max_y.max(y1);
            } else if let Some(&u) = id_to_u32.get(child.as_str()) {
                if let Some(&(cx, cy)) = pos.get(&u) {
                    let (w, h) = sizes[u as usize];
                    min_x = min_x.min(cx - w / 2.0);
                    max_x = max_x.max(cx + w / 2.0);
                    min_y = min_y.min(cy - h / 2.0);
                    max_y = max_y.max(cy + h / 2.0);
                }
            }
        }
    }
    if !min_x.is_finite() {
        return None;
    }
    let b = (
        min_x - FRAME_PAD,
        min_y - FRAME_PAD - FRAME_HEADER,
        max_x + FRAME_PAD,
        max_y + FRAME_PAD,
    );
    boxes.insert(comp.id.clone(), b);
    Some(b)
}

/// Ids of every state reachable from one region, descending into nested
/// composites so a whole region moves as a unit when regions are stacked.
fn collect_region_member_ids<'a>(
    region: &'a [String],
    by_id: &HashMap<&str, &'a CompositeState>,
    out: &mut Vec<&'a str>,
) {
    for child in region {
        out.push(child.as_str());
        if let Some(child_comp) = by_id.get(child.as_str()) {
            for r in &child_comp.regions {
                collect_region_member_ids(r, by_id, out);
            }
        }
    }
}

/// Stack the parallel regions of every multi-region composite into disjoint
/// vertical bands (they otherwise share layers and render interleaved). Each
/// region is left-aligned to a common x and translated down below the previous
/// one; `pos` is mutated in place. Returns the y of each dashed divider drawn
/// between adjacent regions, keyed by composite id.
pub(super) fn stack_regions(
    d: &StateDiagram,
    id_to_u32: &HashMap<String, NodeId>,
    sizes: &[(f64, f64)],
    pos: &mut HashMap<NodeId, (f64, f64)>,
) -> HashMap<String, Vec<f64>> {
    let by_id: HashMap<&str, &CompositeState> =
        d.composites.iter().map(|c| (c.id.as_str(), c)).collect();
    let mut dividers: HashMap<String, Vec<f64>> = HashMap::new();

    for comp in &d.composites {
        if comp.regions.len() < 2 {
            continue;
        }

        // Resolve each region to its laid-out member node ids.
        let regions: Vec<Vec<NodeId>> = comp
            .regions
            .iter()
            .map(|region| {
                let mut ids: Vec<&str> = Vec::new();
                collect_region_member_ids(region, &by_id, &mut ids);
                ids.iter()
                    .filter_map(|id| id_to_u32.get(*id).copied())
                    .filter(|u| pos.contains_key(u))
                    .collect()
            })
            .collect();

        // Left edge shared by every band, so the stacked regions line up.
        let overall_min_x = regions
            .iter()
            .filter_map(|nodes| region_bbox(nodes, sizes, pos).map(|b| b.0))
            .fold(f64::INFINITY, f64::min);
        if !overall_min_x.is_finite() {
            continue;
        }

        let mut divs = Vec::new();
        let mut prev_bottom: Option<f64> = None;
        for nodes in &regions {
            let Some((min_x, min_y, _max_x, max_y)) = region_bbox(nodes, sizes, pos) else {
                continue;
            };
            let dx = overall_min_x - min_x;
            let target_top = match prev_bottom {
                Some(bottom) => bottom + REGION_GAP,
                None => min_y,
            };
            let dy = target_top - min_y;
            for &u in nodes {
                if let Some(p) = pos.get_mut(&u) {
                    p.0 += dx;
                    p.1 += dy;
                }
            }
            if let Some(bottom) = prev_bottom {
                divs.push((bottom + target_top) / 2.0);
            }
            prev_bottom = Some(max_y + dy);
        }
        dividers.insert(comp.id.clone(), divs);
    }
    dividers
}

/// Bounding box `(min_x, min_y, max_x, max_y)` of a set of laid-out nodes.
fn region_bbox(
    nodes: &[NodeId],
    sizes: &[(f64, f64)],
    pos: &HashMap<NodeId, (f64, f64)>,
) -> Option<(f64, f64, f64, f64)> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for &u in nodes {
        let (cx, cy) = *pos.get(&u)?;
        let (w, h) = sizes[u as usize];
        min_x = min_x.min(cx - w / 2.0);
        max_x = max_x.max(cx + w / 2.0);
        min_y = min_y.min(cy - h / 2.0);
        max_y = max_y.max(cy + h / 2.0);
    }
    min_x.is_finite().then_some((min_x, min_y, max_x, max_y))
}

pub(super) fn draw_composites(
    svg: &mut SvgBuilder,
    d: &StateDiagram,
    boxes: &HashMap<String, (f64, f64, f64, f64)>,
    dividers: &HashMap<String, Vec<f64>>,
    theme: &Theme,
) {
    let fg = &theme.fg;
    let border = &theme.flow_node_stroke;
    let band = &theme.flow_node_fill;
    for comp in &d.composites {
        let Some(&(x0, y0, x1, y1)) = boxes.get(&comp.id) else {
            continue;
        };
        // Solid purple-bordered rounded frame with a filled lavender title band
        // and a divider under the title, matching upstream's composite styling.
        svg.rect(
            x0,
            y0,
            x1 - x0,
            y1 - y0,
            &format!("fill=\"none\" stroke=\"{border}\" stroke-width=\"1\" rx=\"5\""),
        );
        svg.rect(
            x0,
            y0,
            x1 - x0,
            TITLE_BAND,
            &format!("fill=\"{band}\" stroke=\"{border}\" stroke-width=\"1\" rx=\"5\""),
        );
        let label = d
            .states
            .iter()
            .find(|s| s.id == comp.id)
            .map(|s| s.label.as_str())
            .filter(|l| !l.is_empty())
            .unwrap_or(comp.id.as_str());
        svg.text(
            (x0 + x1) / 2.0,
            y0 + 16.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\" font-weight=\"bold\""),
            label,
        );
        svg.line(
            x0,
            y0 + TITLE_BAND,
            x1,
            y0 + TITLE_BAND,
            &format!("stroke=\"{border}\" stroke-width=\"1\""),
        );
        // Dashed dividers between stacked parallel regions.
        if let Some(ys) = dividers.get(&comp.id) {
            for &y in ys {
                svg.line(
                    x0,
                    y,
                    x1,
                    y,
                    "stroke=\"#999\" stroke-width=\"1\" stroke-dasharray=\"3 3\"",
                );
            }
        }
    }
}

/// Screen-space bounding box `(x0, y0, x1, y1)` a state note would occupy,
/// computed independently of drawing so canvas sizing can see the real
/// (text-dependent) extent before the `SvgBuilder` exists — the same
/// measure-then-size two-pass pattern used for node/composite bounds.
pub(super) fn note_box(
    note: &StateNote,
    id_to_u32: &HashMap<String, NodeId>,
    sizes: &[(f64, f64)],
    pos: &HashMap<NodeId, (f64, f64)>,
    boxes: &HashMap<String, (f64, f64, f64, f64)>,
    theme: &Theme,
) -> Option<(f64, f64, f64, f64)> {
    let laid_out = id_to_u32
        .get(&note.target)
        .and_then(|u| pos.get(u).map(|p| (*u, *p)));
    let (cx, cy, w, h) = if let Some((u, (cx, cy))) = laid_out {
        let (w, h) = sizes[u as usize];
        (cx, cy, w, h)
    } else if let Some(&(x0, y0, x1, y1)) = boxes.get(&note.target) {
        ((x0 + x1) / 2.0, (y0 + y1) / 2.0, x1 - x0, y1 - y0)
    } else {
        return None;
    };
    let nw = (crate::svg::metrics::text_width(&note.text, 7.0, theme.font_size) + 20.0).max(80.0);
    let nh = 32.0;
    let (nx, ny) = match note.position {
        NotePosition::RightOf => (cx + w / 2.0 + 14.0, cy - nh / 2.0),
        NotePosition::LeftOf => (cx - w / 2.0 - 14.0 - nw, cy - nh / 2.0),
        NotePosition::Over => (cx - nw / 2.0, cy - h / 2.0 - nh - 8.0),
    };
    Some((nx, ny, nx + nw, ny + nh))
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
    let Some((nx, ny, nx1, ny1)) = note_box(note, id_to_u32, sizes, pos, boxes, theme) else {
        return;
    };
    let fg = &theme.fg;
    let (nw, nh) = (nx1 - nx, ny1 - ny);
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
