//! State diagram renderer. Layout via sugiyama, custom shapes for
//! start/end/choice/fork/join pseudo-states.

use std::collections::{HashMap, HashSet};

use crate::parse::{FlowDirection, State, StateDiagram, StateKind, StateTransition, Style};
use crate::sugiyama::{layout_with, Graph, LayoutConfig, NodeId};

use super::builder::{curve_basis_path, fnum, SvgBuilder};
use super::geometry::{clip_circle, clip_rect, clip_rhombus, polyline_midpoint};
use super::interact::{close_click, open_click};
use super::metrics::text_width;
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
        let mut svg = SvgBuilder::new(40.0, 40.0).theme(theme);
        define_marker(&mut svg, theme);
        return svg.finish();
    }

    let dir = d.direction;
    let sizes: Vec<(f64, f64)> = d
        .states
        .iter()
        .map(|s| state_size(s, theme.font_size))
        .collect();
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
    let mut dividers = stack_regions(d, &id_to_u32, &sizes, &mut pos);
    let node_offset: HashMap<NodeId, (f64, f64)> = pos
        .iter()
        .filter_map(|(&u, &(x, y))| {
            let (ox, oy) = orig_pos[&u];
            let off = (x - ox, y - oy);
            (off.0 != 0.0 || off.1 != 0.0).then_some((u, off))
        })
        .collect();

    let mut boxes = compute_composite_boxes(d, &id_to_u32, &pos, &sizes);

    // Canvas extent from node boundaries and cluster frames. A frame reserves
    // header room above its members for the title, so its top/left can fall
    // above/left of the topmost node — measure both corners of every box and
    // shift everything back into a positive CANVAS_PAD margin so the title band
    // is not clipped by the viewBox top edge (issue #242).
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = 0.0_f64;
    let mut max_y = 0.0_f64;
    for (&u, &(x, y)) in &pos {
        let (w, h) = sizes[u as usize];
        min_x = min_x.min(x - w / 2.0);
        min_y = min_y.min(y - h / 2.0);
        max_x = max_x.max(x + w / 2.0);
        max_y = max_y.max(y + h / 2.0);
    }
    for &(bx0, by0, bx1, by1) in boxes.values() {
        min_x = min_x.min(bx0);
        min_y = min_y.min(by0);
        max_x = max_x.max(bx1);
        max_y = max_y.max(by1);
    }
    let shift_x = (CANVAS_PAD - min_x).max(0.0);
    let shift_y = (CANVAS_PAD - min_y).max(0.0);
    if shift_x != 0.0 || shift_y != 0.0 {
        for p in pos.values_mut() {
            p.0 += shift_x;
            p.1 += shift_y;
        }
        for b in boxes.values_mut() {
            b.0 += shift_x;
            b.1 += shift_y;
            b.2 += shift_x;
            b.3 += shift_y;
        }
        for ys in dividers.values_mut() {
            for y in ys {
                *y += shift_y;
            }
        }
    }
    let width = max_x + shift_x + CANVAS_PAD;
    let height = max_y + shift_y + CANVAS_PAD;

    let mut svg = SvgBuilder::new(width, height).theme(theme);
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
                            (x + ox + shift_x, y + oy + shift_y)
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
        if let Some(action) = &state.click {
            open_click(&mut svg, action);
        }
        draw_state(&mut svg, center, sizes[i], state, &d.class_defs, theme);
        if let Some(action) = &state.click {
            close_click(&mut svg, action);
        }
    }

    // Notes attached to states.
    for note in &d.notes {
        draw_state_note(&mut svg, note, &id_to_u32, &sizes, &pos, &boxes, theme);
    }

    svg.finish()
}

const CHOICE_W: f64 = 60.0;
const CHOICE_H: f64 = 40.0;

fn state_size(s: &State, font_size: f64) -> (f64, f64) {
    match s.kind {
        StateKind::Start | StateKind::End | StateKind::History { .. } => {
            (PSEUDO_R * 2.0, PSEUDO_R * 2.0)
        }
        StateKind::Choice => (CHOICE_W, CHOICE_H),
        StateKind::Fork | StateKind::Join => (80.0, 12.0),
        StateKind::Normal => {
            let w = (text_width(&s.label, CHAR_W, font_size) + PAD_X * 2.0).max(MIN_W);
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
    let fg = rs.label_fill(&theme.fg);
    // Pseudo-state marker fill: `&theme.fg` keeps the dark dot on light themes
    // yet stays visible on the dark theme (was a hardcoded near-invisible #333).
    let pseudo = &theme.fg;
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
                &rs.shape_attrs(&theme.flow_node_fill, &theme.flow_node_stroke, "1.5"),
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
                &rs.shape_attrs(&theme.flow_node_fill, &theme.flow_node_stroke, "1.5"),
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
            let base = rs.shape_attrs(&theme.flow_node_fill, &theme.flow_node_stroke, "1.5");
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
    let flow_edge_stroke = &theme.flow_edge_stroke;
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
        crate::svg::label::draw_edge_label(svg, mid, label, theme);
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
    svg.def_arrow_marker("state-arrow", &theme.flow_edge_stroke, 10, 10);
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

    #[test]
    fn click_wraps_state_in_anchor() {
        let d = build("stateDiagram-v2\n[*] --> A\nclick A href \"https://x.test\"\n");
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("<a href=\"https://x.test\">"));
    }

    /// Bounds `(x, y, w, h)` of the composite frame rect (the solid
    /// purple-bordered `rx="5"` rect drawn before its title band).
    fn frame_rect(svg: &str) -> (f64, f64, f64, f64) {
        let key = "rx=\"5\"";
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
        // Exactly one composite frame (one bold title), and its single member
        // `a1` is the only rounded normal-state node (`rx="10"`). A second would
        // mean `A` was also emitted as a standalone node. Frames use `rx="5"`.
        assert_eq!(svg.matches("font-weight=\"bold\"").count(), 1);
        assert_eq!(svg.matches("rx=\"10\"").count(), 1);
    }

    #[test]
    fn composite_frame_not_clipped_by_top_edge() {
        // Regression for #242: a composite whose members sit at the top of the
        // layout must keep its title band inside the canvas — the frame top,
        // header included, stays at or below y=0 rather than being clipped.
        let d = build(
            "stateDiagram-v2\n[*] --> Idle\nstate Workflow {\n[*] --> Step1\nStep1 --> [*]\n}\nIdle --> Workflow\n",
        );
        let svg = render(&d, &Theme::default());
        let (_, fy, _, _) = frame_rect(&svg);
        assert!(fy >= 0.0, "composite frame top {fy} clipped above viewBox");
    }

    #[test]
    fn composite_uses_solid_border_and_title_band() {
        // Issue #242 styling: solid (not dashed) frame plus a filled title band.
        let d = build("stateDiagram-v2\n[*] --> A\nstate A {\n[*] --> a1\n}\n");
        let svg = render(&d, &Theme::default());
        assert!(!svg.contains("stroke-dasharray=\"5 3\""));
        // Purple border and lavender title band, from the theme node colors.
        assert!(svg.contains("stroke=\"#9370DB\""));
        assert!(svg.contains("fill=\"#ECECFF\""));
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
    fn opposite_transitions_render_distinctly() {
        // Regression for #241: `Idle --> Running` and `Running --> Idle` used to
        // collapse onto one segment, hiding the second label under the first.
        let d = build("stateDiagram-v2\nIdle --> Running : start\nRunning --> Idle : stop\n");
        let svg = render(&d, &Theme::default());
        // Two arrowheads, one per direction.
        assert_eq!(svg.matches("marker-end=\"url(#state-arrow)\"").count(), 2);
        // The two labels no longer share an anchor.
        let (sx, sy) = label_center(&svg, "start");
        let (tx, ty) = label_center(&svg, "stop");
        assert!(
            (sx - tx).abs() > 1.0 || (sy - ty).abs() > 1.0,
            "labels overlap: start ({sx},{sy}) vs stop ({tx},{ty})",
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
