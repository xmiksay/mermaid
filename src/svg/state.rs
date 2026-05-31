//! State diagram renderer. Layout via sugiyama, custom shapes for
//! start/end/choice/fork/join pseudo-states.

use std::collections::HashMap;
use std::fmt::Write as _;

use crate::parse::{
    FlowDirection, NotePosition, State, StateDiagram, StateKind, StateNote, StateTransition,
};
use crate::sugiyama::{layout_with, Graph, LayoutConfig, NodeId};

use super::builder::{fnum, SvgBuilder};
use super::theme::Theme;

const CHAR_W: f64 = 7.5;
const LINE_H: f64 = 20.0;
const PAD_X: f64 = 18.0;
const PAD_Y: f64 = 12.0;
const MIN_W: f64 = 70.0;
const MIN_H: f64 = 40.0;
const PSEUDO_R: f64 = 10.0; // start/end radius
const CANVAS_PAD: f64 = 24.0;

pub(crate) fn render(d: &StateDiagram, theme: &Theme) -> String {
    let fg = theme.fg;
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

    let nodes: Vec<NodeId> = (0..d.states.len() as NodeId).collect();
    let edges: Vec<(NodeId, NodeId)> = d
        .transitions
        .iter()
        .filter_map(|t| Some((*id_to_u32.get(&t.from)?, *id_to_u32.get(&t.to)?)))
        .collect();
    let node_size_su: HashMap<NodeId, (f64, f64)> = d
        .states
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let (w, h) = sizes[i];
            let s = match dir {
                FlowDirection::LeftRight | FlowDirection::RightLeft => (h, w),
                _ => (w, h),
            };
            (i as NodeId, s)
        })
        .collect();

    let g = Graph {
        nodes,
        edges,
        node_size: node_size_su,
    };
    let layout = layout_with(&g, &LayoutConfig::default()).unwrap_or_default();
    let (raw_w, raw_h) = (layout.width, layout.height);
    let (canvas_w, canvas_h) = match dir {
        FlowDirection::TopDown | FlowDirection::BottomTop => (raw_w, raw_h),
        FlowDirection::LeftRight | FlowDirection::RightLeft => (raw_h, raw_w),
    };
    let width = canvas_w + CANVAS_PAD * 2.0;
    let height = canvas_h + CANVAS_PAD * 2.0;

    let transform = move |(sx, sy): (f64, f64)| -> (f64, f64) {
        let (tx, ty) = match dir {
            FlowDirection::TopDown => (sx, sy),
            FlowDirection::BottomTop => (sx, raw_h - sy),
            FlowDirection::LeftRight => (sy, sx),
            FlowDirection::RightLeft => (raw_h - sy, sx),
        };
        (tx + CANVAS_PAD, ty + CANVAS_PAD)
    };

    let mut svg = SvgBuilder::new(width, height).font(theme.font_family, theme.font_size);
    define_marker(&mut svg, theme);

    for tr in &d.transitions {
        let (Some(&u), Some(&v)) = (id_to_u32.get(&tr.from), id_to_u32.get(&tr.to)) else {
            continue;
        };
        let Some(raw_pts) = layout.edge_points.get(&(u, v)) else {
            continue;
        };
        if raw_pts.len() < 2 {
            continue;
        }
        let pts: Vec<(f64, f64)> = raw_pts.iter().map(|&p| transform(p)).collect();
        draw_transition(&mut svg, &pts, tr, &d.states, &id_to_u32, &sizes, theme);
    }

    for (i, state) in d.states.iter().enumerate() {
        let center = transform(layout.node_pos[&(i as NodeId)]);
        draw_state(&mut svg, center, sizes[i], state, theme);
    }

    // Composite outlines: bounding box of all child state positions, drawn
    // under or above nodes. We draw on top with no fill so labels remain
    // visible.
    for comp in &d.composites {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        let mut count = 0;
        for region in &comp.regions {
            for child_id in region {
                if let Some(&u) = id_to_u32.get(child_id) {
                    let (cx, cy) = transform(layout.node_pos[&u]);
                    let (w, h) = sizes[u as usize];
                    min_x = min_x.min(cx - w / 2.0);
                    max_x = max_x.max(cx + w / 2.0);
                    min_y = min_y.min(cy - h / 2.0);
                    max_y = max_y.max(cy + h / 2.0);
                    count += 1;
                }
            }
        }
        if count == 0 || !min_x.is_finite() {
            continue;
        }
        let pad_inner = 14.0;
        let header_h = 18.0;
        let x = min_x - pad_inner;
        let y = min_y - pad_inner - header_h;
        let w = (max_x - min_x) + pad_inner * 2.0;
        let h = (max_y - min_y) + pad_inner * 2.0 + header_h;
        svg.rect(
            x,
            y,
            w,
            h,
            "fill=\"none\" stroke=\"#999\" stroke-width=\"1\" rx=\"10\" stroke-dasharray=\"5 3\"",
        );
        svg.text(
            x + 10.0,
            y + 14.0,
            &format!("fill=\"{fg}\" font-size=\"12\" font-weight=\"bold\""),
            &comp.id,
        );
    }

    // Notes attached to states.
    for note in &d.notes {
        draw_state_note(
            &mut svg, note, &id_to_u32, &sizes, &layout, &transform, theme,
        );
    }

    svg.finish()
}

fn draw_state_note(
    svg: &mut SvgBuilder,
    note: &StateNote,
    id_to_u32: &HashMap<String, NodeId>,
    sizes: &[(f64, f64)],
    layout: &crate::sugiyama::Layout,
    transform: &impl Fn((f64, f64)) -> (f64, f64),
    theme: &Theme,
) {
    let fg = theme.fg;
    let Some(&u) = id_to_u32.get(&note.target) else {
        return;
    };
    let (cx, cy) = transform(layout.node_pos[&u]);
    let (w, h) = sizes[u as usize];
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

const CHOICE_W: f64 = 60.0;
const CHOICE_H: f64 = 40.0;

fn state_size(s: &State) -> (f64, f64) {
    match s.kind {
        StateKind::Start | StateKind::End => (PSEUDO_R * 2.0, PSEUDO_R * 2.0),
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
    theme: &Theme,
) {
    let fg = theme.fg;
    let flow_node_fill = theme.flow_node_fill;
    let flow_node_stroke = theme.flow_node_stroke;
    match s.kind {
        StateKind::Start => {
            svg.circle(cx, cy, PSEUDO_R, "fill=\"#333\" stroke=\"none\"");
        }
        StateKind::End => {
            svg.circle(
                cx,
                cy,
                PSEUDO_R,
                "fill=\"none\" stroke=\"#333\" stroke-width=\"1.5\"",
            );
            svg.circle(cx, cy, PSEUDO_R - 4.0, "fill=\"#333\" stroke=\"none\"");
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
                &format!(
                    "fill=\"{}\" stroke=\"{}\" stroke-width=\"1.5\"",
                    theme.flow_node_fill, theme.flow_node_stroke
                ),
            );
        }
        StateKind::Fork | StateKind::Join => {
            svg.rect(
                cx - w / 2.0,
                cy - h / 2.0,
                w,
                h,
                "fill=\"#333\" stroke=\"none\"",
            );
        }
        StateKind::Normal => {
            svg.rect(
                cx - w / 2.0,
                cy - h / 2.0,
                w,
                h,
                &format!(
                    "fill=\"{flow_node_fill}\" stroke=\"{flow_node_stroke}\" stroke-width=\"1.5\" rx=\"10\""
                ),
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
    states: &[State],
    id_to_u32: &HashMap<String, NodeId>,
    sizes: &[(f64, f64)],
    theme: &Theme,
) {
    let fg = theme.fg;
    let flow_edge_stroke = theme.flow_edge_stroke;
    let flow_label_bg = theme.flow_label_bg;
    let src_idx = id_to_u32[&tr.from] as usize;
    let dst_idx = id_to_u32[&tr.to] as usize;
    let n = pts.len();

    let first = clip_to_state(pts[1], pts[0], sizes[src_idx], states[src_idx].kind);
    let last = clip_to_state(pts[n - 2], pts[n - 1], sizes[dst_idx], states[dst_idx].kind);

    let mut clipped = Vec::with_capacity(n);
    clipped.push(first);
    for p in &pts[1..n - 1] {
        clipped.push(*p);
    }
    clipped.push(last);

    let d = polyline_path(&clipped);
    svg.path(
        &d,
        &format!(
            "fill=\"none\" stroke=\"{flow_edge_stroke}\" stroke-width=\"1.5\" \
             marker-end=\"url(#state-arrow)\""
        ),
    );
    if let Some(label) = &tr.label {
        let mid = midpoint(&clipped);
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

fn clip_to_state(
    from: (f64, f64),
    center: (f64, f64),
    size: (f64, f64),
    kind: StateKind,
) -> (f64, f64) {
    match kind {
        StateKind::Start | StateKind::End => clip_circle(from, center, PSEUDO_R),
        StateKind::Choice => clip_rhombus(from, center, (CHOICE_W, CHOICE_H)),
        _ => clip_rect(from, center, size),
    }
}

fn clip_rect(from: (f64, f64), c: (f64, f64), (w, h): (f64, f64)) -> (f64, f64) {
    let dx = from.0 - c.0;
    let dy = from.1 - c.1;
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return c;
    }
    let hw = w / 2.0;
    let hh = h / 2.0;
    let tx = if dx.abs() > 1e-9 {
        hw / dx.abs()
    } else {
        f64::INFINITY
    };
    let ty = if dy.abs() > 1e-9 {
        hh / dy.abs()
    } else {
        f64::INFINITY
    };
    let t = tx.min(ty);
    (c.0 + dx * t, c.1 + dy * t)
}

fn clip_circle(from: (f64, f64), c: (f64, f64), r: f64) -> (f64, f64) {
    let dx = from.0 - c.0;
    let dy = from.1 - c.1;
    let d = (dx * dx + dy * dy).sqrt().max(1e-9);
    (c.0 + dx * r / d, c.1 + dy * r / d)
}

fn clip_rhombus(from: (f64, f64), c: (f64, f64), (w, h): (f64, f64)) -> (f64, f64) {
    let dx = from.0 - c.0;
    let dy = from.1 - c.1;
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return c;
    }
    let hw = w / 2.0;
    let hh = h / 2.0;
    let t = 1.0 / (dx.abs() / hw + dy.abs() / hh).max(1e-9);
    (c.0 + dx * t, c.1 + dy * t)
}

fn polyline_path(pts: &[(f64, f64)]) -> String {
    let mut s = String::new();
    for (i, (x, y)) in pts.iter().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        let _ = write!(s, "{cmd}{} {}", fnum(*x), fnum(*y));
    }
    s
}

fn midpoint(pts: &[(f64, f64)]) -> (f64, f64) {
    if pts.len() < 2 {
        return pts[0];
    }
    let mut segs = Vec::with_capacity(pts.len() - 1);
    let mut total = 0.0;
    for w in pts.windows(2) {
        let dx = w[1].0 - w[0].0;
        let dy = w[1].1 - w[0].1;
        let l = (dx * dx + dy * dy).sqrt();
        segs.push(l);
        total += l;
    }
    let half = total / 2.0;
    let mut walked = 0.0;
    for (i, w) in pts.windows(2).enumerate() {
        if walked + segs[i] >= half {
            let t = (half - walked) / segs[i].max(1e-9);
            return (
                w[0].0 + t * (w[1].0 - w[0].0),
                w[0].1 + t * (w[1].1 - w[0].1),
            );
        }
        walked += segs[i];
    }
    pts[pts.len() / 2]
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
}
