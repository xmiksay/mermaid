//! State diagram renderer. Layout via sugiyama, custom shapes for
//! start/end/choice/fork/join pseudo-states.

use std::collections::{HashMap, HashSet};

use crate::parse::{FlowDirection, State, StateDiagram, StateKind, StateTransition, Style};
use crate::sugiyama::{layout_with, Graph, LayoutConfig, NodeId};

use super::builder::{curve_basis_path, fnum, SvgBuilder};
use super::geometry::{clip_circle, clip_rect, clip_rhombus, polyline_midpoint};
use super::style::resolve_style;
use super::theme::Theme;

mod composite;
use composite::*;

const CHAR_W: f64 = 7.5;
const LINE_H: f64 = 20.0;
const PAD_X: f64 = 18.0;
const PAD_Y: f64 = 12.0;
const MIN_W: f64 = 70.0;
const MIN_H: f64 = 40.0;
const PSEUDO_R: f64 = 10.0; // start/end radius
const CANVAS_PAD: f64 = 24.0;

pub(crate) fn render(d: &StateDiagram, theme: &Theme) -> String {
    if d.states.is_empty() {
        let mut svg = SvgBuilder::new(40.0, 40.0).font(theme.font_family, theme.font_size);
        define_marker(&mut svg, theme);
        return svg.finish();
    }

    let dir = d.direction;
    let sizes: Vec<(f64, f64)> = d.states.iter().map(state_size).collect();
    let id_to_u32: HashMap<String, NodeId> = d
        .states
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.clone(), i as NodeId))
        .collect();

    // Composite states are drawn as cluster frames around their members, not as
    // standalone nodes; external transitions clip to the frame the way flowchart
    // subgraphs do. So they are excluded from the layout graph.
    let composite_ids: HashSet<&str> = d.composites.iter().map(|c| c.id.as_str()).collect();

    let nodes: Vec<NodeId> = (0..d.states.len() as NodeId)
        .filter(|&u| !composite_ids.contains(d.states[u as usize].id.as_str()))
        .collect();
    let edges: Vec<(NodeId, NodeId)> = d
        .transitions
        .iter()
        .filter_map(|t| {
            if composite_ids.contains(t.from.as_str()) || composite_ids.contains(t.to.as_str()) {
                return None;
            }
            Some((*id_to_u32.get(&t.from)?, *id_to_u32.get(&t.to)?))
        })
        .collect();
    let node_size_su: HashMap<NodeId, (f64, f64)> = nodes
        .iter()
        .map(|&u| {
            let (w, h) = sizes[u as usize];
            let s = match dir {
                FlowDirection::LeftRight | FlowDirection::RightLeft => (h, w),
                _ => (w, h),
            };
            (u, s)
        })
        .collect();

    let g = Graph {
        nodes,
        edges,
        node_size: node_size_su,
    };
    let layout = layout_with(&g, &LayoutConfig::default()).unwrap_or_default();
    let raw_h = layout.height;

    let transform = move |(sx, sy): (f64, f64)| -> (f64, f64) {
        let (tx, ty) = match dir {
            FlowDirection::TopDown => (sx, sy),
            FlowDirection::BottomTop => (sx, raw_h - sy),
            FlowDirection::LeftRight => (sy, sx),
            FlowDirection::RightLeft => (raw_h - sy, sx),
        };
        (tx + CANVAS_PAD, ty + CANVAS_PAD)
    };

    // Screen-space positions for laid-out (non-composite) states.
    let mut pos: HashMap<NodeId, (f64, f64)> = layout
        .node_pos
        .iter()
        .map(|(&u, &p)| (u, transform(p)))
        .collect();

    // Stack the parallel regions of multi-region composites into disjoint
    // vertical bands; record how far each moved so routed edges follow.
    let orig_pos = pos.clone();
    let dividers = stack_regions(d, &id_to_u32, &sizes, &mut pos);
    let node_offset: HashMap<NodeId, (f64, f64)> = pos
        .iter()
        .filter_map(|(&u, &(x, y))| {
            let (ox, oy) = orig_pos[&u];
            let off = (x - ox, y - oy);
            (off.0 != 0.0 || off.1 != 0.0).then_some((u, off))
        })
        .collect();

    let boxes = compute_composite_boxes(d, &id_to_u32, &pos, &sizes);

    // Canvas extent from node boundaries and cluster frames.
    let mut max_x = 0.0_f64;
    let mut max_y = 0.0_f64;
    for (&u, &(x, y)) in &pos {
        let (w, h) = sizes[u as usize];
        max_x = max_x.max(x + w / 2.0);
        max_y = max_y.max(y + h / 2.0);
    }
    for &(_, _, bx1, by1) in boxes.values() {
        max_x = max_x.max(bx1 + FRAME_PAD);
        max_y = max_y.max(by1 + FRAME_PAD);
    }
    let width = max_x + CANVAS_PAD;
    let height = max_y + CANVAS_PAD;

    let mut svg = SvgBuilder::new(width, height).font(theme.font_family, theme.font_size);
    define_marker(&mut svg, theme);

    // Cluster frames first (under nodes/edges) so labels stay legible.
    draw_composites(&mut svg, d, &boxes, &dividers, theme);

    for tr in &d.transitions {
        let (Some(start), Some(end)) = (
            endpoint_clip(&tr.from, &id_to_u32, &d.states, &sizes, &pos, &boxes),
            endpoint_clip(&tr.to, &id_to_u32, &d.states, &sizes, &pos, &boxes),
        ) else {
            continue;
        };
        // Real node→node transitions keep their routed polyline; an endpoint
        // that is a composite cluster has no layout route, so draw a straight
        // connector clipped to the cluster box.
        let pts: Vec<(f64, f64)> = match (id_to_u32.get(&tr.from), id_to_u32.get(&tr.to)) {
            (Some(&u), Some(&v)) => match layout.edge_points.get(&(u, v)) {
                // Both endpoints share a region, hence the same stacking offset;
                // shift the whole routed polyline so it tracks the moved nodes.
                Some(p) if p.len() >= 2 => {
                    let (ox, oy) = node_offset.get(&u).copied().unwrap_or((0.0, 0.0));
                    p.iter()
                        .map(|&q| {
                            let (x, y) = transform(q);
                            (x + ox, y + oy)
                        })
                        .collect()
                }
                _ => vec![start.center, end.center],
            },
            _ => vec![start.center, end.center],
        };
        draw_transition(&mut svg, &pts, tr, &start, &end, theme);
    }

    for (i, state) in d.states.iter().enumerate() {
        let Some(&center) = pos.get(&(i as NodeId)) else {
            continue;
        };
        draw_state(&mut svg, center, sizes[i], state, &d.class_defs, theme);
    }

    // Notes attached to states.
    for note in &d.notes {
        draw_state_note(&mut svg, note, &id_to_u32, &sizes, &pos, &boxes, theme);
    }

    svg.finish()
}

const CHOICE_W: f64 = 60.0;
const CHOICE_H: f64 = 40.0;

fn state_size(s: &State) -> (f64, f64) {
    match s.kind {
        StateKind::Start | StateKind::End | StateKind::History { .. } => {
            (PSEUDO_R * 2.0, PSEUDO_R * 2.0)
        }
        StateKind::Choice => (CHOICE_W, CHOICE_H),
        StateKind::Fork | StateKind::Join => (80.0, 12.0),
        StateKind::Normal => {
            let n = s.label.chars().count() as f64;
            let w = (n * CHAR_W + PAD_X * 2.0).max(MIN_W);
            let h = (LINE_H + PAD_Y * 2.0).max(MIN_H);
            (w, h)
        }
    }
}

fn draw_state(
    svg: &mut SvgBuilder,
    (cx, cy): (f64, f64),
    (w, h): (f64, f64),
    s: &State,
    class_defs: &HashMap<String, Style>,
    theme: &Theme,
) {
    let rs = resolve_style(class_defs, &s.classes, &s.style);
    let fg = rs.label_fill(theme.fg);
    // Pseudo-state marker fill: `theme.fg` keeps the dark dot on light themes
    // yet stays visible on the dark theme (was a hardcoded near-invisible #333).
    let pseudo = theme.fg;
    match s.kind {
        StateKind::Start => {
            svg.circle(
                cx,
                cy,
                PSEUDO_R,
                &format!("fill=\"{pseudo}\" stroke=\"none\""),
            );
        }
        StateKind::End => {
            svg.circle(
                cx,
                cy,
                PSEUDO_R,
                &format!("fill=\"none\" stroke=\"{pseudo}\" stroke-width=\"1.5\""),
            );
            svg.circle(
                cx,
                cy,
                PSEUDO_R - 4.0,
                &format!("fill=\"{pseudo}\" stroke=\"none\""),
            );
        }
        StateKind::Choice => {
            let hw = CHOICE_W / 2.0;
            let hh = CHOICE_H / 2.0;
            let d = format!(
                "M{cx} {top}L{right} {cy}L{cx} {bot}L{left} {cy}Z",
                cx = fnum(cx),
                top = fnum(cy - hh),
                right = fnum(cx + hw),
                bot = fnum(cy + hh),
                left = fnum(cx - hw)
            );
            svg.path(
                &d,
                &rs.shape_attrs(theme.flow_node_fill, theme.flow_node_stroke, "1.5"),
            );
        }
        StateKind::Fork | StateKind::Join => {
            svg.rect(
                cx - w / 2.0,
                cy - h / 2.0,
                w,
                h,
                &format!("fill=\"{pseudo}\" stroke=\"none\""),
            );
        }
        StateKind::History { deep } => {
            svg.circle(
                cx,
                cy,
                PSEUDO_R,
                &rs.shape_attrs(theme.flow_node_fill, theme.flow_node_stroke, "1.5"),
            );
            let label = if deep { "H*" } else { "H" };
            svg.text(
                cx,
                cy + 4.0,
                &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"11\""),
                label,
            );
        }
        StateKind::Normal => {
            let base = rs.shape_attrs(theme.flow_node_fill, theme.flow_node_stroke, "1.5");
            svg.rect(
                cx - w / 2.0,
                cy - h / 2.0,
                w,
                h,
                &format!("{base} rx=\"10\""),
            );
            svg.text(
                cx,
                cy + 5.0,
                &format!("text-anchor=\"middle\" fill=\"{fg}\""),
                &s.label,
            );
        }
    }
}

fn draw_transition(
    svg: &mut SvgBuilder,
    pts: &[(f64, f64)],
    tr: &StateTransition,
    start: &StateEndClip,
    end: &StateEndClip,
    theme: &Theme,
) {
    let fg = theme.fg;
    let flow_edge_stroke = theme.flow_edge_stroke;
    let flow_label_bg = theme.flow_label_bg;
    let n = pts.len();
    if n < 2 {
        return;
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
    if let Some(label) = &tr.label {
        let mid = polyline_midpoint(&clipped);
        let chars = label.chars().count() as f64;
        let w = chars * 7.0 + 8.0;
        let h = 18.0;
        svg.rect(
            mid.0 - w / 2.0,
            mid.1 - h / 2.0,
            w,
            h,
            &format!("fill=\"{flow_label_bg}\" stroke=\"none\""),
        );
        svg.text(
            mid.0,
            mid.1 + 4.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\""),
            label,
        );
    }
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

fn define_marker(svg: &mut SvgBuilder, theme: &Theme) {
    let flow_edge_stroke = theme.flow_edge_stroke;
    let m = format!(
        "<marker id=\"state-arrow\" viewBox=\"0 0 10 10\" refX=\"10\" refY=\"5\" \
         markerWidth=\"10\" markerHeight=\"10\" orient=\"auto-start-reverse\">\
         <path d=\"M0 0 L10 5 L0 10 z\" fill=\"{flow_edge_stroke}\"/></marker>"
    );
    svg.defs_raw(&m);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn build(s: &str) -> StateDiagram {
        match parse(s).unwrap() {
            crate::parse::Diagram::State(s) => s,
            _ => panic!("not state"),
        }
    }

    #[test]
    fn renders_full_lifecycle() {
        let d = build("stateDiagram-v2\n[*] --> Idle\nIdle --> Running: go\nRunning --> [*]\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">Idle<"));
        assert!(svg.contains(">Running<"));
        assert!(svg.contains(">go<"));
    }

    #[test]
    fn start_and_end_drawn() {
        let d = build("stateDiagram-v2\n[*] --> A\nA --> [*]\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("<circle"));
    }

    #[test]
    fn style_applies_to_normal_state() {
        let d = build("stateDiagram-v2\n[*] --> A\nstyle A fill:#abc\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("fill=\"#abc\""));
    }

    #[test]
    fn history_states_rendered() {
        let d = build("stateDiagram-v2\nstate A {\n[*] --> B\nB --> [H]\n[H*] --> C\n}\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains(">H<"));
        assert!(svg.contains(">H*<"));
    }

    #[test]
    fn classdef_applies_to_state() {
        let d = build("stateDiagram-v2\n[*] --> A\nclassDef foo fill:#abc\nclass A foo\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("fill=\"#abc\""));
    }

    /// Bounds `(x, y, w, h)` of the dashed composite frame rect.
    fn frame_rect(svg: &str) -> (f64, f64, f64, f64) {
        let key = "stroke-dasharray=\"5 3\"";
        let kpos = svg.find(key).expect("no composite frame");
        let open = svg[..kpos].rfind("<rect ").unwrap();
        let tag = &svg[open..kpos];
        let grab = |attr: &str| {
            let s = tag.find(attr).unwrap() + attr.len();
            let e = s + tag[s..].find('"').unwrap();
            tag[s..e].parse::<f64>().unwrap()
        };
        (
            grab("x=\""),
            grab("y=\""),
            grab("width=\""),
            grab("height=\""),
        )
    }

    /// Centre `(x, y)` of the `text-anchor=middle` label for `id`.
    fn label_center(svg: &str, id: &str) -> (f64, f64) {
        let needle = format!(">{id}</text>");
        let end = svg.find(&needle).unwrap_or_else(|| panic!("no label {id}"));
        let open = svg[..end].rfind("<text ").unwrap();
        let tag = &svg[open..end];
        let grab = |attr: &str| {
            let s = tag.find(attr).unwrap() + attr.len();
            let e = s + tag[s..].find('"').unwrap();
            tag[s..e].parse::<f64>().unwrap()
        };
        (grab("x=\""), grab("y=\""))
    }

    #[test]
    fn composite_frame_contains_its_members() {
        // Regression for #63: the composite's children must be laid out *inside*
        // its frame, not on a detached part of the canvas.
        let d = build("stateDiagram-v2\n[*] --> A\nstate A {\n[*] --> a1\n}\n");
        let svg = render(&d, &Theme::default());
        let (fx, fy, fw, fh) = frame_rect(&svg);
        let (ax, ay) = label_center(&svg, "a1");
        assert!(
            ax > fx && ax < fx + fw && ay > fy && ay < fy + fh,
            "member a1 ({ax},{ay}) must sit inside the frame ({fx},{fy},{fw},{fh})",
        );
        // The external `[*] --> A` transition still draws an arrow to the frame.
        assert!(svg.contains("marker-end=\"url(#state-arrow)\""));
    }

    #[test]
    fn composite_not_drawn_as_standalone_node() {
        // The composite id `A` must not also be drawn as a small normal-state
        // rounded rect (the detached artifact the issue describes).
        let d = build("stateDiagram-v2\n[*] --> A\nstate A {\n[*] --> a1\n}\n");
        let svg = render(&d, &Theme::default());
        // Exactly one dashed frame (for `A`) and, in total, two `rx="10"` rects:
        // the frame plus the single member node `a1`. A third would mean `A` was
        // also emitted as a standalone node.
        assert_eq!(svg.matches("rx=\"10\" stroke-dasharray").count(), 1);
        assert_eq!(svg.matches("rx=\"10\"").count(), 2);
    }

    #[test]
    fn parallel_regions_stacked_with_divider() {
        // Two concurrent regions must render in disjoint vertical bands with a
        // dashed divider between them, not interleaved into one blob.
        let d = build(
            "stateDiagram-v2\nstate Active {\n[*] --> NumLockOff\n--\n[*] --> CapsLockOff\n}\n",
        );
        let svg = render(&d, &Theme::default());
        // One dashed region divider inside the frame.
        assert_eq!(svg.matches("stroke-dasharray=\"3 3\"").count(), 1);
        // The two regions' states sit in separate vertical bands.
        let (_, up) = label_center(&svg, "NumLockOff");
        let (_, down) = label_center(&svg, "CapsLockOff");
        assert!(
            (up - down).abs() > 20.0,
            "regions overlap vertically: {up} vs {down}",
        );
    }

    #[test]
    fn pseudo_states_visible_on_dark_theme() {
        // Start/end dots used a hardcoded #333, near-invisible on the dark bg.
        let d = build("stateDiagram-v2\n[*] --> A\nA --> [*]\n");
        let svg = render(&d, &Theme::dark());
        assert!(svg.contains("fill=\"#E0E0E0\""));
        assert!(!svg.contains("#333"));
    }
}
