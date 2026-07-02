//! C4 diagram renderer.
//!
//! Layout: sugiyama layered placement driven by relations (matches upstream
//! mermaid's dagre-based behaviour). Boundaries are drawn as a dashed outline
//! around the bounding box of their members after layout.
//!
//! Relations are solid polylines following the routed waypoints from sugiyama,
//! with an arrow head on the destination side (and on the source side for
//! `BiRel`). Labels sit at the polyline midpoint over a translucent background.

use std::collections::{BTreeMap, HashMap};

use crate::parse::{C4BoundaryKind, C4Diagram, C4Element, C4ElementKind, C4Kind, C4Relation};
use crate::sugiyama::{layout_with, Graph, LayoutConfig, NodeId};

use super::builder::{fnum, SvgBuilder};
use super::theme::Theme;

const PAD: f64 = 32.0;
const TITLE_GAP: f64 = 44.0;

const BOX_W: f64 = 220.0;
const BOX_H: f64 = 130.0;

const BOUNDARY_HDR: f64 = 28.0;
const BOUNDARY_PAD: f64 = 20.0;

pub(crate) fn render(d: &C4Diagram, theme: &Theme) -> String {
    let fg = theme.fg;
    let fg_muted = theme.fg_muted;
    let title_h = if d.title.is_some() { TITLE_GAP } else { 0.0 };

    // Boundary headers extend BOUNDARY_HDR + BOUNDARY_PAD above their topmost
    // member. Reserve that overhang above the content origin so the topmost
    // boundary clears the title (and the canvas top when there is no title).
    let boundary_overhang = if has_any_boundary(&d.elements) {
        BOUNDARY_HDR + BOUNDARY_PAD
    } else {
        0.0
    };

    let flat = flatten(&d.elements, None);
    let alias_to_id: HashMap<String, NodeId> = flat
        .iter()
        .enumerate()
        .map(|(i, f)| (f.el.alias.clone(), i as NodeId))
        .collect();

    let mut g = Graph::default();
    for (i, _) in flat.iter().enumerate() {
        g.nodes.push(i as NodeId);
    }
    for (i, f) in flat.iter().enumerate() {
        let (w, h) = shape_size(f.el.kind);
        g.node_size.insert(i as NodeId, (w, h));
        let _ = f;
        let _ = i;
    }
    for r in &d.relations {
        if let (Some(&u), Some(&v)) = (alias_to_id.get(&r.from), alias_to_id.get(&r.to)) {
            g.edges.push((u, v));
        }
    }

    let cfg = LayoutConfig {
        layer_gap: 90.0,
        node_gap: 50.0,
        ..LayoutConfig::default()
    };
    let layout = layout_with(&g, &cfg).unwrap_or_default();

    let origin_x = PAD;
    let origin_y = PAD + title_h + boundary_overhang;

    let mut pos: HashMap<String, (f64, f64, f64, f64)> = HashMap::new();
    for (i, f) in flat.iter().enumerate() {
        let (w, h) = shape_size(f.el.kind);
        let id = i as NodeId;
        let (cx, cy) = layout
            .node_pos
            .get(&id)
            .copied()
            .unwrap_or((BOX_W / 2.0, BOX_H / 2.0));
        let x = origin_x + cx - w / 2.0;
        let y = origin_y + cy - h / 2.0;
        pos.insert(f.el.alias.clone(), (x, y, w, h));
    }

    let boundaries = collect_boundaries(&d.elements, &pos);
    for b in &boundaries {
        pos.insert(b.alias.clone(), (b.x, b.y, b.w, b.h));
    }

    let mut max_x = origin_x + layout.width;
    let mut max_y = origin_y + layout.height;
    for (_, &(x, y, w, h)) in pos.iter() {
        if x + w > max_x {
            max_x = x + w;
        }
        if y + h > max_y {
            max_y = y + h;
        }
    }

    let width = (max_x + PAD).max(600.0);
    let height = (max_y + PAD).max(220.0);
    let mut svg = SvgBuilder::new(width, height).font(theme.font_family, theme.font_size);

    let arrow_color = theme.flow_edge_stroke;
    svg.defs_raw(&format!(
        "<marker id=\"c4-arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" \
         markerWidth=\"9\" markerHeight=\"9\" orient=\"auto-start-reverse\">\
         <path d=\"M0,0 L10,5 L0,10 z\" fill=\"{arrow_color}\"/></marker>"
    ));

    if let Some(t) = &d.title {
        svg.text(
            width / 2.0,
            PAD + 22.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\" font-weight=\"bold\""),
            t,
        );
        let sub = match d.kind {
            C4Kind::Context => "System Context",
            C4Kind::Container => "Container Diagram",
            C4Kind::Component => "Component Diagram",
            C4Kind::Dynamic => "Dynamic Diagram",
            C4Kind::Deployment => "Deployment Diagram",
        };
        svg.text(
            width / 2.0,
            PAD + 38.0,
            &format!(
                "text-anchor=\"middle\" fill=\"{fg_muted}\" font-size=\"11\" font-style=\"italic\""
            ),
            sub,
        );
    }

    for b in &boundaries {
        draw_boundary_rect(b, &mut svg, theme);
    }

    for f in &flat {
        if let Some(&(x, y, w, h)) = pos.get(&f.el.alias) {
            draw_element(&f.el, x, y, w, h, &mut svg, theme);
        }
    }

    for r in &d.relations {
        draw_rel(r, &pos, &layout, &alias_to_id, &mut svg, theme);
    }

    svg.finish()
}

struct FlatElement {
    el: C4Element,
}

fn has_any_boundary(elements: &[C4Element]) -> bool {
    elements
        .iter()
        .any(|el| el.boundary_kind.is_some() || has_any_boundary(&el.members))
}

fn flatten(elements: &[C4Element], _parent: Option<String>) -> Vec<FlatElement> {
    let mut out = Vec::new();
    for el in elements {
        if el.boundary_kind.is_some() {
            let mut nested = flatten(&el.members, Some(el.alias.clone()));
            out.append(&mut nested);
        } else {
            out.push(FlatElement { el: el.clone() });
        }
    }
    out
}

struct BoundaryBox {
    alias: String,
    label: String,
    kind: C4BoundaryKind,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

fn collect_boundaries(
    elements: &[C4Element],
    pos: &HashMap<String, (f64, f64, f64, f64)>,
) -> Vec<BoundaryBox> {
    let mut out = Vec::new();
    walk_boundaries(elements, pos, &mut out);
    out
}

fn walk_boundaries(
    elements: &[C4Element],
    pos: &HashMap<String, (f64, f64, f64, f64)>,
    out: &mut Vec<BoundaryBox>,
) {
    for el in elements {
        if el.boundary_kind.is_some() {
            let mut min_x = f64::INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            collect_member_bounds(
                &el.members,
                pos,
                &mut min_x,
                &mut min_y,
                &mut max_x,
                &mut max_y,
            );
            if min_x.is_finite() {
                let pad = BOUNDARY_PAD;
                out.push(BoundaryBox {
                    alias: el.alias.clone(),
                    label: el.label.clone(),
                    kind: el.boundary_kind.unwrap_or(C4BoundaryKind::Generic),
                    x: min_x - pad,
                    y: min_y - pad - BOUNDARY_HDR,
                    w: (max_x - min_x) + pad * 2.0,
                    h: (max_y - min_y) + pad * 2.0 + BOUNDARY_HDR,
                });
            }
            walk_boundaries(&el.members, pos, out);
        }
    }
}

fn collect_member_bounds(
    members: &[C4Element],
    pos: &HashMap<String, (f64, f64, f64, f64)>,
    min_x: &mut f64,
    min_y: &mut f64,
    max_x: &mut f64,
    max_y: &mut f64,
) {
    for m in members {
        if m.boundary_kind.is_some() {
            collect_member_bounds(&m.members, pos, min_x, min_y, max_x, max_y);
        } else if let Some(&(x, y, w, h)) = pos.get(&m.alias) {
            if x < *min_x {
                *min_x = x;
            }
            if y < *min_y {
                *min_y = y;
            }
            if x + w > *max_x {
                *max_x = x + w;
            }
            if y + h > *max_y {
                *max_y = y + h;
            }
        }
    }
}

fn shape_size(_kind: C4ElementKind) -> (f64, f64) {
    (BOX_W, BOX_H)
}

fn draw_boundary_rect(b: &BoundaryBox, svg: &mut SvgBuilder, theme: &Theme) {
    let fg = theme.fg;
    let fg_muted = theme.fg_muted;
    let stroke = theme.flow_node_stroke;
    svg.rect(
        b.x,
        b.y,
        b.w,
        b.h,
        &format!(
            "fill=\"none\" stroke=\"{stroke}\" stroke-width=\"1.5\" \
             stroke-dasharray=\"6 4\" rx=\"6\""
        ),
    );
    let kind = match b.kind {
        C4BoundaryKind::Enterprise => "Enterprise Boundary",
        C4BoundaryKind::System => "System Boundary",
        C4BoundaryKind::Container => "Container Boundary",
        C4BoundaryKind::Deployment => "Deployment Node",
        C4BoundaryKind::Generic => "Boundary",
    };
    svg.text(
        b.x + 14.0,
        b.y + 18.0,
        &format!("fill=\"{fg}\" font-size=\"12\" font-weight=\"bold\""),
        &b.label,
    );
    svg.text(
        b.x + b.w - 14.0,
        b.y + 18.0,
        &format!("text-anchor=\"end\" fill=\"{fg_muted}\" font-size=\"10\" font-style=\"italic\""),
        &format!("[{kind}]"),
    );
}

fn draw_element(
    el: &C4Element,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    svg: &mut SvgBuilder,
    theme: &Theme,
) {
    match el.kind {
        C4ElementKind::SystemDb | C4ElementKind::ContainerDb | C4ElementKind::ComponentDb => {
            draw_cylinder(el, x, y, w, h, svg);
        }
        C4ElementKind::SystemQueue
        | C4ElementKind::ContainerQueue
        | C4ElementKind::ComponentQueue => draw_queue(el, x, y, w, h, svg),
        _ => draw_box(el, x, y, w, h, svg),
    }
    if matches!(el.kind, C4ElementKind::Person) {
        draw_person_icon(svg, x + w - 28.0, y + 6.0, el);
    }
    let _ = theme;
}

fn draw_person_icon(svg: &mut SvgBuilder, x: f64, y: f64, el: &C4Element) {
    let (fill, _) = palette(el.kind, el.external);
    let stroke = if is_dark_fill(fill) {
        "#FFFFFF"
    } else {
        "#0B2B4A"
    };
    use std::fmt::Write as _;
    let _ = write!(
        svg.body,
        "<g transform=\"translate({x} {y})\" fill=\"none\" stroke=\"{stroke}\" stroke-width=\"1.5\" stroke-linecap=\"round\" stroke-linejoin=\"round\">\
         <circle cx=\"11\" cy=\"6\" r=\"4\" fill=\"{stroke}\"/>\
         <path d=\"M2 22 C2 14 20 14 20 22\" fill=\"{stroke}\"/>\
         </g>",
        x = fnum(x),
        y = fnum(y),
    );
}

fn draw_box(el: &C4Element, x: f64, y: f64, w: f64, h: f64, svg: &mut SvgBuilder) {
    let (fill, border) = palette(el.kind, el.external);
    let text_fill = text_color_for(fill);
    let muted = mute_text_color_for(fill);
    svg.rect(
        x,
        y,
        w,
        h,
        &format!(
            "fill=\"{fill}\" stroke=\"{border}\" stroke-width=\"1.5\" rx=\"6\"{dash}",
            dash = if el.external {
                " stroke-dasharray=\"5 3\""
            } else {
                ""
            }
        ),
    );
    write_label_block(svg, el, x, y, w, h, text_fill, muted);
}

fn draw_cylinder(el: &C4Element, x: f64, y: f64, w: f64, h: f64, svg: &mut SvgBuilder) {
    let (fill, border) = palette(el.kind, el.external);
    let text_fill = text_color_for(fill);
    let muted = mute_text_color_for(fill);
    let rx = w / 2.0;
    let ry = 10.0;
    let top_y = y + ry;
    let bot_y = y + h - ry;
    let dash = if el.external {
        " stroke-dasharray=\"5 3\""
    } else {
        ""
    };
    svg.path(
        &format!(
            "M {lx} {top_y} L {lx} {bot_y} A {rx} {ry} 0 0 0 {rx_end} {bot_y} L {rx_end} {top_y}",
            lx = fnum(x),
            top_y = fnum(top_y),
            bot_y = fnum(bot_y),
            rx = fnum(rx),
            ry = fnum(ry),
            rx_end = fnum(x + w),
        ),
        &format!("fill=\"{fill}\" stroke=\"{border}\" stroke-width=\"1.5\"{dash}"),
    );
    svg.path(
        &format!(
            "M {lx} {top_y} A {rx} {ry} 0 0 1 {rx_end} {top_y} A {rx} {ry} 0 0 1 {lx} {top_y} Z",
            lx = fnum(x),
            top_y = fnum(top_y),
            rx = fnum(rx),
            ry = fnum(ry),
            rx_end = fnum(x + w),
        ),
        &format!("fill=\"{fill}\" stroke=\"{border}\" stroke-width=\"1.5\"{dash}"),
    );
    svg.path(
        &format!(
            "M {lx} {bot_y} A {rx} {ry} 0 0 0 {rx_end} {bot_y}",
            lx = fnum(x),
            bot_y = fnum(bot_y),
            rx = fnum(rx),
            ry = fnum(ry),
            rx_end = fnum(x + w),
        ),
        &format!("fill=\"none\" stroke=\"{border}\" stroke-width=\"1.5\"{dash}"),
    );
    write_label_block(svg, el, x, y + ry, w, h - 2.0 * ry, text_fill, muted);
}

fn draw_queue(el: &C4Element, x: f64, y: f64, w: f64, h: f64, svg: &mut SvgBuilder) {
    let (fill, border) = palette(el.kind, el.external);
    let text_fill = text_color_for(fill);
    let muted = mute_text_color_for(fill);
    let rx = h / 2.0;
    let dash = if el.external {
        " stroke-dasharray=\"5 3\""
    } else {
        ""
    };
    svg.rect(
        x,
        y,
        w,
        h,
        &format!(
            "fill=\"{fill}\" stroke=\"{border}\" stroke-width=\"1.5\" rx=\"{rx}\" ry=\"{rx}\"{dash}"
        ),
    );
    write_label_block(svg, el, x, y, w, h, text_fill, muted);
}

#[allow(clippy::too_many_arguments)]
fn write_label_block(
    svg: &mut SvgBuilder,
    el: &C4Element,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    fg: &str,
    muted: &str,
) {
    let cx = x + w / 2.0;
    let kind_label = kind_text(el.kind, el.external);
    let top = y + 6.0;
    svg.text(
        cx,
        top + 12.0,
        &format!("text-anchor=\"middle\" fill=\"{muted}\" font-size=\"10\" font-style=\"italic\""),
        kind_label,
    );
    let title_y = top + 32.0;
    svg.text(
        cx,
        title_y,
        &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"13\" font-weight=\"bold\""),
        &el.label,
    );
    let mut next_y = title_y + 16.0;
    if let Some(t) = &el.technology {
        svg.text(
            cx,
            next_y,
            &format!(
                "text-anchor=\"middle\" fill=\"{muted}\" font-size=\"10\" font-style=\"italic\""
            ),
            &format!("[{}]", t),
        );
        next_y += 14.0;
    }
    if let Some(d) = &el.descr {
        let max_chars = ((w - 16.0) / 6.2).max(8.0) as usize;
        let max_lines = (((y + h) - next_y - 4.0) / 12.0).max(1.0) as usize;
        let lines = wrap_text(d, max_chars, max_lines);
        for (i, line) in lines.iter().enumerate() {
            svg.text(
                cx,
                next_y + (i as f64) * 12.0,
                &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"10\""),
                line,
            );
        }
    }
}

fn kind_text(kind: C4ElementKind, external: bool) -> &'static str {
    match (kind, external) {
        (C4ElementKind::Person, false) => "<<person>>",
        (C4ElementKind::Person, true) => "<<external_person>>",
        (C4ElementKind::System, false) => "<<system>>",
        (C4ElementKind::System, true) => "<<external_system>>",
        (C4ElementKind::SystemDb, false) => "<<system_db>>",
        (C4ElementKind::SystemDb, true) => "<<external_system_db>>",
        (C4ElementKind::SystemQueue, false) => "<<system_queue>>",
        (C4ElementKind::SystemQueue, true) => "<<external_system_queue>>",
        (C4ElementKind::Container, false) => "<<container>>",
        (C4ElementKind::Container, true) => "<<external_container>>",
        (C4ElementKind::ContainerDb, false) => "<<container_db>>",
        (C4ElementKind::ContainerDb, true) => "<<external_container_db>>",
        (C4ElementKind::ContainerQueue, false) => "<<container_queue>>",
        (C4ElementKind::ContainerQueue, true) => "<<external_container_queue>>",
        (C4ElementKind::Component, false) => "<<component>>",
        (C4ElementKind::Component, true) => "<<external_component>>",
        (C4ElementKind::ComponentDb, false) => "<<component_db>>",
        (C4ElementKind::ComponentDb, true) => "<<external_component_db>>",
        (C4ElementKind::ComponentQueue, false) => "<<component_queue>>",
        (C4ElementKind::ComponentQueue, true) => "<<external_component_queue>>",
        (C4ElementKind::Node, _) => "<<node>>",
    }
}

fn palette(kind: C4ElementKind, external: bool) -> (&'static str, &'static str) {
    if external {
        return ("#999999", "#6B6B6B");
    }
    match kind {
        C4ElementKind::Person => ("#08427B", "#073B6F"),
        C4ElementKind::System | C4ElementKind::SystemDb | C4ElementKind::SystemQueue => {
            ("#1168BD", "#0D5BA8")
        }
        C4ElementKind::Container | C4ElementKind::ContainerDb | C4ElementKind::ContainerQueue => {
            ("#438DD5", "#3A7DBE")
        }
        C4ElementKind::Component | C4ElementKind::ComponentDb | C4ElementKind::ComponentQueue => {
            ("#85BBF0", "#6FA8DC")
        }
        C4ElementKind::Node => ("#444444", "#2E2E2E"),
    }
}

fn text_color_for(fill: &str) -> &'static str {
    if is_dark_fill(fill) {
        "#FFFFFF"
    } else {
        "#0B2B4A"
    }
}

fn mute_text_color_for(fill: &str) -> &'static str {
    if is_dark_fill(fill) {
        "#D9E5F2"
    } else {
        "#3A5A7A"
    }
}

fn is_dark_fill(fill: &str) -> bool {
    if let Some(hex) = fill.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f64;
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f64;
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f64;
            return (0.299 * r + 0.587 * g + 0.114 * b) < 140.0;
        }
    }
    false
}

fn draw_rel(
    r: &C4Relation,
    pos: &HashMap<String, (f64, f64, f64, f64)>,
    layout: &crate::sugiyama::Layout,
    alias_to_id: &HashMap<String, NodeId>,
    svg: &mut SvgBuilder,
    theme: &Theme,
) {
    let fg = theme.fg;
    let fg_muted = theme.fg_muted;
    let stroke = theme.flow_edge_stroke;
    let label_bg = theme.flow_label_bg;

    let Some(&(ax, ay, aw, ah)) = pos.get(&r.from) else {
        return;
    };
    let Some(&(bx, by, bw, bh)) = pos.get(&r.to) else {
        return;
    };

    // Prefer sugiyama's routed waypoints if available; fall back to a straight line.
    let pts: Vec<(f64, f64)> = match (alias_to_id.get(&r.from), alias_to_id.get(&r.to)) {
        (Some(&u), Some(&v)) => layout.edge_points.get(&(u, v)).cloned().unwrap_or_default(),
        _ => Vec::new(),
    };

    let (sx, sy) = (ax + aw / 2.0, ay + ah / 2.0);
    let (tx, ty) = (bx + bw / 2.0, by + bh / 2.0);

    let routed: Vec<(f64, f64)> = if pts.len() >= 2 {
        // Sugiyama returns waypoints in local layout coordinates; translate back to canvas space.
        // Since we centred each node on its layout position, sugiyama waypoints are *already*
        // in the same coordinate space as our pos values once the canvas origin offset is applied.
        // The first/last waypoints are the source/dest centres in layout coords -> we replace them
        // with our actual canvas centres before clipping.
        let mut v: Vec<(f64, f64)> = Vec::with_capacity(pts.len());
        v.push((sx, sy));
        let mid = &pts[1..pts.len() - 1];
        for (mx, my) in mid {
            v.push(origin_translated(*mx, *my, sx, sy, tx, ty, &pts));
        }
        v.push((tx, ty));
        v
    } else {
        vec![(sx, sy), (tx, ty)]
    };

    let p_first = clip_rect_to_edge(routed[1], (sx, sy), aw, ah);
    let p_last = clip_rect_to_edge(routed[routed.len() - 2], (tx, ty), bw, bh);
    let mut clipped = Vec::with_capacity(routed.len());
    clipped.push(p_first);
    for p in &routed[1..routed.len() - 1] {
        clipped.push(*p);
    }
    clipped.push(p_last);

    let markers = if r.bidirectional {
        "marker-start=\"url(#c4-arrow)\" marker-end=\"url(#c4-arrow)\""
    } else {
        "marker-end=\"url(#c4-arrow)\""
    };

    let path = polyline_path(&clipped);
    svg.path(
        &path,
        &format!("fill=\"none\" stroke=\"{stroke}\" stroke-width=\"1.4\" {markers}"),
    );

    let label = &r.label;
    let tech = r.technology.as_deref();
    if label.is_empty() && tech.is_none() {
        return;
    }
    let lw = label_width(label, tech).min(220.0);
    let lh = if tech.is_some() { 30.0 } else { 18.0 };
    let (mx, my) = polyline_midpoint(&clipped);
    svg.rect(
        mx - lw / 2.0,
        my - lh / 2.0,
        lw,
        lh,
        &format!("fill=\"{label_bg}\" fill-opacity=\"0.5\" stroke=\"none\" rx=\"3\""),
    );
    if let Some(t) = tech {
        svg.text(
            mx,
            my - 1.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"10\""),
            &truncate(label, 36),
        );
        svg.text(
            mx,
            my + 12.0,
            &format!(
                "text-anchor=\"middle\" fill=\"{fg_muted}\" font-size=\"9\" font-style=\"italic\""
            ),
            &format!("[{}]", truncate(t, 30)),
        );
    } else {
        svg.text(
            mx,
            my + 4.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"10\""),
            &truncate(label, 36),
        );
    }
    let _ = BTreeMap::<String, ()>::new();
}

#[allow(clippy::too_many_arguments)]
fn origin_translated(
    mx: f64,
    my: f64,
    sx: f64,
    sy: f64,
    tx: f64,
    ty: f64,
    pts: &[(f64, f64)],
) -> (f64, f64) {
    // Sugiyama produces waypoints in its own coordinate frame; the first and last
    // waypoints are the source and destination centres in that frame. Our actual
    // canvas centres for the same nodes are (sx,sy) and (tx,ty). For interior
    // waypoints we apply an affine transform mapping (pts[0], pts[last]) -> ((sx,sy), (tx,ty))
    // when the layout offsets differ from our placement (e.g. boundary padding).
    let s_layout = pts[0];
    let t_layout = pts[pts.len() - 1];
    let lx = t_layout.0 - s_layout.0;
    let ly = t_layout.1 - s_layout.1;
    let cx = tx - sx;
    let cy = ty - sy;
    let denom_x = lx.abs() + 1e-9;
    let denom_y = ly.abs() + 1e-9;
    if lx.abs() < 1e-6 && ly.abs() < 1e-6 {
        return (mx, my);
    }
    let scale_x = if lx.abs() > 1e-6 { cx / lx } else { 1.0 };
    let scale_y = if ly.abs() > 1e-6 { cy / ly } else { 1.0 };
    let rx = sx + (mx - s_layout.0) * scale_x;
    let ry = sy + (my - s_layout.1) * scale_y;
    let _ = (denom_x, denom_y);
    (rx, ry)
}

fn polyline_path(pts: &[(f64, f64)]) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    for (i, (x, y)) in pts.iter().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        let _ = write!(s, "{cmd}{} {}", fnum(*x), fnum(*y));
    }
    s
}

fn polyline_midpoint(pts: &[(f64, f64)]) -> (f64, f64) {
    if pts.len() < 2 {
        return pts.first().copied().unwrap_or((0.0, 0.0));
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

fn clip_rect_to_edge(from: (f64, f64), center: (f64, f64), w: f64, h: f64) -> (f64, f64) {
    let dx = from.0 - center.0;
    let dy = from.1 - center.1;
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return center;
    }
    let hw = w / 2.0;
    let hh = h / 2.0;
    let tx_lim = if dx.abs() < 1e-9 {
        f64::INFINITY
    } else {
        hw / dx.abs()
    };
    let ty_lim = if dy.abs() < 1e-9 {
        f64::INFINITY
    } else {
        hh / dy.abs()
    };
    let t = tx_lim.min(ty_lim);
    (center.0 + dx * t, center.1 + dy * t)
}

fn label_width(label: &str, tech: Option<&str>) -> f64 {
    let len_label = label.chars().count();
    let len_tech = tech.map(|t| t.chars().count() + 2).unwrap_or(0);
    let max_chars = len_label.max(len_tech) as f64;
    (max_chars * 5.8 + 16.0).max(60.0)
}

fn wrap_text(s: &str, max_chars: usize, max_lines: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        if cur.is_empty() {
            cur.push_str(word);
            continue;
        }
        if cur.chars().count() + 1 + word.chars().count() <= max_chars {
            cur.push(' ');
            cur.push_str(word);
        } else {
            lines.push(std::mem::take(&mut cur));
            cur.push_str(word);
            if lines.len() >= max_lines {
                break;
            }
        }
    }
    if !cur.is_empty() && lines.len() < max_lines {
        lines.push(cur);
    }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
    }
    if let Some(last) = lines.last_mut() {
        if last.chars().count() > max_chars {
            let mut t: String = last.chars().take(max_chars.saturating_sub(1)).collect();
            t.push('…');
            *last = t;
        }
    }
    lines
}

fn truncate(s: &str, n: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= n {
        s.to_string()
    } else {
        let mut out: String = chars[..n.saturating_sub(1)].iter().collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{C4Kind, C4RelDirection};

    fn person(alias: &str, label: &str) -> C4Element {
        C4Element {
            kind: C4ElementKind::Person,
            alias: alias.into(),
            label: label.into(),
            descr: None,
            technology: None,
            external: false,
            boundary_alias: None,
            boundary_label: None,
            boundary_kind: None,
            members: vec![],
        }
    }

    #[test]
    fn produces_svg() {
        let d = C4Diagram {
            kind: C4Kind::Context,
            title: Some("Sys".into()),
            elements: vec![person("u", "User")],
            relations: vec![],
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">User<"));
        assert!(svg.contains(">Sys<"));
    }

    fn container(alias: &str, label: &str, members: Vec<C4Element>) -> C4Element {
        C4Element {
            kind: C4ElementKind::Node,
            alias: alias.into(),
            label: label.into(),
            descr: None,
            technology: None,
            external: false,
            boundary_alias: None,
            boundary_label: None,
            boundary_kind: Some(C4BoundaryKind::Deployment),
            members,
        }
    }

    /// Regression for #5: with a title present, the topmost boundary header must
    /// not overlap the title/subtitle text. The subtitle baseline is at PAD+38;
    /// the boundary rect top must sit below it.
    #[test]
    fn boundary_clears_title() {
        let d = C4Diagram {
            kind: C4Kind::Deployment,
            title: Some("Deployment".into()),
            elements: vec![container(
                "app06",
                "app06",
                vec![person("uportal", "portal")],
            )],
            relations: vec![],
        };
        let svg = render(&d, &Theme::default());

        // Every boundary rect uses the dashed stroke; find its `y` and check it
        // clears the subtitle baseline plus a small margin.
        let subtitle_baseline = PAD + 38.0;
        let mut checked = false;
        for chunk in svg.split("<rect").skip(1) {
            if !chunk.contains("stroke-dasharray=\"6 4\"") {
                continue;
            }
            let y = extract_attr(chunk, "y=\"").expect("boundary rect has y");
            assert!(
                y > subtitle_baseline,
                "boundary top {y} overlaps title (subtitle baseline {subtitle_baseline})"
            );
            checked = true;
        }
        assert!(checked, "expected at least one boundary rect");
    }

    fn extract_attr(s: &str, key: &str) -> Option<f64> {
        let start = s.find(key)? + key.len();
        let rest = &s[start..];
        let end = rest.find('"')?;
        rest[..end].parse().ok()
    }

    #[test]
    fn arrow_marker_present() {
        let d = C4Diagram {
            kind: C4Kind::Context,
            title: None,
            elements: vec![person("a", "A"), person("b", "B")],
            relations: vec![C4Relation {
                from: "a".into(),
                to: "b".into(),
                label: "uses".into(),
                technology: None,
                direction: C4RelDirection::Default,
                bidirectional: false,
            }],
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("c4-arrow"));
        assert!(svg.contains("marker-end=\"url(#c4-arrow)\""));
        assert!(!svg.contains("marker-start=\"url(#c4-arrow)\""));
    }

    #[test]
    fn bidirectional_has_both_markers() {
        let d = C4Diagram {
            kind: C4Kind::Container,
            title: None,
            elements: vec![person("a", "A"), person("b", "B")],
            relations: vec![C4Relation {
                from: "a".into(),
                to: "b".into(),
                label: "syncs".into(),
                technology: None,
                direction: C4RelDirection::Default,
                bidirectional: true,
            }],
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("marker-start=\"url(#c4-arrow)\""));
        assert!(svg.contains("marker-end=\"url(#c4-arrow)\""));
    }

    #[test]
    fn relations_are_solid() {
        let d = C4Diagram {
            kind: C4Kind::Context,
            title: None,
            elements: vec![person("a", "A"), person("b", "B")],
            relations: vec![C4Relation {
                from: "a".into(),
                to: "b".into(),
                label: "uses".into(),
                technology: None,
                direction: C4RelDirection::Default,
                bidirectional: false,
            }],
        };
        let svg = render(&d, &Theme::default());
        // The connector path must not be dashed (only the boundary outline is).
        assert!(!svg.contains("stroke-dasharray=\"5 4\""));
    }
}
