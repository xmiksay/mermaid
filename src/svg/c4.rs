//! C4 diagram renderer.
//!
//! Layout: upstream-style row-flow placement. Mermaid's `c4Renderer` does *not*
//! run a graph layout for C4 — it flows shapes into rows. We mirror that: each
//! boundary lays its members out left-to-right, wrapping after `SHAPE_IN_ROW`
//! shapes, and is then sized from that content; sibling boundaries (and any
//! unbounded elements) are themselves flowed `BOUNDARY_IN_ROW` per row. Boundary
//! boxes therefore never overlap by construction.
//!
//! Boundaries are drawn as an outline around their content: dashed `7.0,7.0` for
//! most kinds, but solid for `Deployment_Node` (matching upstream's `nodeType`
//! special-case). Stroke is `#444444`, width 1.
//!
//! Relations are `#444444` quadratic Bézier curves between the placed shapes,
//! clipped to each node's rectangle, with an arrow head on the destination side
//! (and on the source side for `BiRel`). Labels sit at the segment midpoint with
//! no background; `[techn]` renders italic below the label.

use std::collections::HashMap;

use crate::parse::{
    C4BoundaryKind, C4Diagram, C4Element, C4ElementKind, C4ElementStyle, C4Kind, C4RelStyle,
    C4Relation,
};

use super::builder::{fnum, SvgBuilder};
use super::geometry::{clip_rect, polyline_midpoint};
use super::theme::Theme;

const PAD: f64 = 32.0;
const TITLE_GAP: f64 = 44.0;

const BOX_W: f64 = 220.0;
const BOX_H: f64 = 130.0;

const BOUNDARY_HDR: f64 = 28.0;
const BOUNDARY_PAD: f64 = 20.0;
const BOUNDARY_MIN_W: f64 = 200.0;

// Row-flow knobs — upstream defaults (settable via UpdateLayoutConfig, see #14).
const SHAPE_IN_ROW: usize = 4;
const BOUNDARY_IN_ROW: usize = 2;

const H_GAP: f64 = 40.0;
const V_GAP: f64 = 40.0;

/// Boundary outlines and relation lines both use upstream's `#444444`.
const C4_LINE: &str = "#444444";

pub(crate) fn render(d: &C4Diagram, theme: &Theme) -> String {
    let fg = theme.fg;
    let fg_muted = theme.fg_muted;
    let title_h = if d.title.is_some() { TITLE_GAP } else { 0.0 };

    let origin_x = PAD;
    let origin_y = PAD + title_h;

    // Row-flow knobs are overridable via `UpdateLayoutConfig` (see #14).
    let shape_in_row = d.layout.shape_in_row.unwrap_or(SHAPE_IN_ROW).max(1);
    let boundary_in_row = d.layout.boundary_in_row.unwrap_or(BOUNDARY_IN_ROW).max(1);
    let (nodes, _cw, _ch) = flow_layout(&d.elements, shape_in_row, boundary_in_row);

    let mut pos: HashMap<String, (f64, f64, f64, f64)> = HashMap::new();
    let mut boundaries: Vec<BoundaryBox> = Vec::new();
    let mut leaves: Vec<(C4Element, f64, f64, f64, f64)> = Vec::new();
    place_absolute(
        &nodes,
        origin_x,
        origin_y,
        &mut pos,
        &mut boundaries,
        &mut leaves,
    );

    let mut max_x = origin_x;
    let mut max_y = origin_y;
    for &(x, y, w, h) in pos.values() {
        max_x = max_x.max(x + w);
        max_y = max_y.max(y + h);
    }

    let width = (max_x + PAD).max(600.0);
    let height = (max_y + PAD).max(220.0);
    let mut svg = SvgBuilder::new(width, height).font(theme.font_family, theme.font_size);

    let arrow_color = C4_LINE;
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

    let elem_styles: HashMap<&str, &C4ElementStyle> = d
        .element_styles
        .iter()
        .map(|s| (s.alias.as_str(), s))
        .collect();
    let rel_styles: HashMap<(&str, &str), &C4RelStyle> = d
        .rel_styles
        .iter()
        .map(|s| ((s.from.as_str(), s.to.as_str()), s))
        .collect();

    for (el, x, y, w, h) in &leaves {
        let ov = elem_styles.get(el.alias.as_str()).copied();
        let style = resolve_element_style(el, ov);
        draw_element(el, &style, *x, *y, *w, *h, &mut svg, theme);
    }

    for r in &d.relations {
        let ov = rel_styles.get(&(r.from.as_str(), r.to.as_str())).copied();
        draw_rel(r, ov, &pos, &mut svg, theme);
    }

    svg.finish()
}

struct BoundaryBox {
    label: String,
    kind: C4BoundaryKind,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// A placed element in the row-flow layout. Coordinates are relative to the
/// top-left origin of the level the node belongs to; a boundary's `children`
/// are relative to the boundary's own top-left.
struct LayoutNode {
    el: C4Element,
    is_boundary: bool,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    children: Vec<LayoutNode>,
}

/// Lay out `items` with the upstream row-flow: size each item (recursively for
/// boundaries), then place items left-to-right, wrapping after `SHAPE_IN_ROW`
/// shapes / `BOUNDARY_IN_ROW` boundaries. Returns the placed nodes plus the
/// total content size.
fn flow_layout(
    items: &[C4Element],
    shape_in_row: usize,
    boundary_in_row: usize,
) -> (Vec<LayoutNode>, f64, f64) {
    let mut nodes: Vec<LayoutNode> = items
        .iter()
        .map(|item| {
            if item.boundary_kind.is_some() {
                let (mut kids, cw, ch) = flow_layout(&item.members, shape_in_row, boundary_in_row);
                let dx = BOUNDARY_PAD;
                let dy = BOUNDARY_PAD + BOUNDARY_HDR;
                for k in &mut kids {
                    k.x += dx;
                    k.y += dy;
                }
                let w = (cw + 2.0 * BOUNDARY_PAD)
                    .max(BOUNDARY_MIN_W)
                    .max(header_min_w(item));
                let h = ch + 2.0 * BOUNDARY_PAD + BOUNDARY_HDR;
                LayoutNode {
                    el: item.clone(),
                    is_boundary: true,
                    x: 0.0,
                    y: 0.0,
                    w,
                    h,
                    children: kids,
                }
            } else {
                let (w, h) = shape_size(item.kind);
                LayoutNode {
                    el: item.clone(),
                    is_boundary: false,
                    x: 0.0,
                    y: 0.0,
                    w,
                    h,
                    children: Vec::new(),
                }
            }
        })
        .collect();

    let mut x = 0.0;
    let mut y = 0.0;
    let mut row_h: f64 = 0.0;
    let mut col = 0usize;
    let mut total_w: f64 = 0.0;
    for node in &mut nodes {
        let per_row = if node.is_boundary {
            boundary_in_row
        } else {
            shape_in_row
        };
        if col >= per_row {
            x = 0.0;
            y += row_h + V_GAP;
            row_h = 0.0;
            col = 0;
        }
        node.x = x;
        node.y = y;
        x += node.w + H_GAP;
        row_h = row_h.max(node.h);
        total_w = total_w.max(x - H_GAP);
        col += 1;
    }
    let total_h = y + row_h;
    (nodes, total_w, total_h)
}

/// Walk the placed tree, converting relative coords to absolute canvas coords
/// and collecting boundary frames, leaf draw entries, and the alias → rect map
/// used to route relations.
fn place_absolute(
    nodes: &[LayoutNode],
    ox: f64,
    oy: f64,
    pos: &mut HashMap<String, (f64, f64, f64, f64)>,
    boundaries: &mut Vec<BoundaryBox>,
    leaves: &mut Vec<(C4Element, f64, f64, f64, f64)>,
) {
    for n in nodes {
        let ax = ox + n.x;
        let ay = oy + n.y;
        pos.insert(n.el.alias.clone(), (ax, ay, n.w, n.h));
        if n.is_boundary {
            boundaries.push(BoundaryBox {
                label: n.el.label.clone(),
                kind: n.el.boundary_kind.unwrap_or(C4BoundaryKind::Generic),
                x: ax,
                y: ay,
                w: n.w,
                h: n.h,
            });
            place_absolute(&n.children, ax, ay, pos, boundaries, leaves);
        } else {
            leaves.push((n.el.clone(), ax, ay, n.w, n.h));
        }
    }
}

fn shape_size(_kind: C4ElementKind) -> (f64, f64) {
    (BOX_W, BOX_H)
}

/// Minimum boundary width so the header label and the `[kind]` tag (stacked
/// below it) don't get clipped.
fn header_min_w(b: &C4Element) -> f64 {
    let kind = boundary_kind_label(b.boundary_kind.unwrap_or(C4BoundaryKind::Generic));
    let label_w = b.label.chars().count() as f64 * 8.0;
    let kind_w = kind.chars().count() as f64 * 6.0;
    label_w.max(kind_w) + 28.0
}

fn boundary_kind_label(kind: C4BoundaryKind) -> &'static str {
    match kind {
        C4BoundaryKind::Enterprise => "Enterprise Boundary",
        C4BoundaryKind::System => "System Boundary",
        C4BoundaryKind::Container => "Container Boundary",
        C4BoundaryKind::Deployment => "Deployment Node",
        C4BoundaryKind::Generic => "Boundary",
    }
}

fn draw_boundary_rect(b: &BoundaryBox, svg: &mut SvgBuilder, theme: &Theme) {
    let fg = theme.fg;
    let fg_muted = theme.fg_muted;
    // Upstream draws a `Deployment_Node` (any boundary with a `nodeType`) with a
    // solid border and every other boundary kind dashed `7.0,7.0`.
    let dash = if matches!(b.kind, C4BoundaryKind::Deployment) {
        String::new()
    } else {
        " stroke-dasharray=\"7 7\"".to_string()
    };
    svg.rect(
        b.x,
        b.y,
        b.w,
        b.h,
        &format!(
            "fill=\"none\" stroke=\"{C4_LINE}\" stroke-width=\"1\" rx=\"2.5\" ry=\"2.5\"{dash}"
        ),
    );
    let kind = boundary_kind_label(b.kind);
    let label_size = theme.font_size + 2.0;
    svg.text(
        b.x + 14.0,
        b.y + 18.0,
        &format!(
            "fill=\"{fg}\" font-size=\"{}\" font-weight=\"bold\"",
            fnum(label_size)
        ),
        &b.label,
    );
    svg.text(
        b.x + 14.0,
        b.y + 18.0 + label_size,
        &format!("fill=\"{fg_muted}\" font-size=\"10\" font-style=\"italic\""),
        &format!("[{kind}]"),
    );
}

/// Effective colors for one element after applying any `UpdateElementStyle`
/// override on top of the built-in palette.
struct ElementStyle {
    fill: String,
    border: String,
    text: String,
    muted: String,
}

fn resolve_element_style(el: &C4Element, ov: Option<&C4ElementStyle>) -> ElementStyle {
    let (base_fill, base_border) = palette(el.kind, el.external);
    let fill = ov
        .and_then(|s| s.bg_color.clone())
        .unwrap_or_else(|| base_fill.to_string());
    let border = ov
        .and_then(|s| s.border_color.clone())
        .unwrap_or_else(|| base_border.to_string());
    let font = ov.and_then(|s| s.font_color.clone());
    let text = font
        .clone()
        .unwrap_or_else(|| text_color_for(&fill).to_string());
    let muted = font.unwrap_or_else(|| mute_text_color_for(&fill).to_string());
    ElementStyle {
        fill,
        border,
        text,
        muted,
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_element(
    el: &C4Element,
    style: &ElementStyle,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    svg: &mut SvgBuilder,
    theme: &Theme,
) {
    match el.kind {
        C4ElementKind::SystemDb | C4ElementKind::ContainerDb | C4ElementKind::ComponentDb => {
            draw_cylinder(el, style, x, y, w, h, svg);
        }
        C4ElementKind::SystemQueue
        | C4ElementKind::ContainerQueue
        | C4ElementKind::ComponentQueue => draw_queue(el, style, x, y, w, h, svg),
        _ => draw_box(el, style, x, y, w, h, svg),
    }
    if matches!(el.kind, C4ElementKind::Person) {
        draw_person_icon(svg, x + w - 28.0, y + 6.0, &style.fill);
    }
    let _ = theme;
}

fn draw_person_icon(svg: &mut SvgBuilder, x: f64, y: f64, fill: &str) {
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

fn draw_box(
    el: &C4Element,
    style: &ElementStyle,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    svg: &mut SvgBuilder,
) {
    let fill = &style.fill;
    let border = &style.border;
    let text_fill = style.text.as_str();
    let muted = style.muted.as_str();
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

fn draw_cylinder(
    el: &C4Element,
    style: &ElementStyle,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    svg: &mut SvgBuilder,
) {
    let fill = &style.fill;
    let border = &style.border;
    let text_fill = style.text.as_str();
    let muted = style.muted.as_str();
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

fn draw_queue(
    el: &C4Element,
    style: &ElementStyle,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    svg: &mut SvgBuilder,
) {
    let fill = &style.fill;
    let border = &style.border;
    let text_fill = style.text.as_str();
    let muted = style.muted.as_str();
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
    ov: Option<&C4RelStyle>,
    pos: &HashMap<String, (f64, f64, f64, f64)>,
    svg: &mut SvgBuilder,
    theme: &Theme,
) {
    let fg: &str = ov.and_then(|s| s.text_color.as_deref()).unwrap_or(theme.fg);
    let fg_muted = theme.fg_muted;
    let stroke: &str = ov.and_then(|s| s.line_color.as_deref()).unwrap_or(C4_LINE);

    let Some(&(ax, ay, aw, ah)) = pos.get(&r.from) else {
        return;
    };
    let Some(&(bx, by, bw, bh)) = pos.get(&r.to) else {
        return;
    };

    let (sx, sy) = (ax + aw / 2.0, ay + ah / 2.0);
    let (tx, ty) = (bx + bw / 2.0, by + bh / 2.0);

    // Point-to-point line, clipped to each node's rectangle.
    let p_first = clip_rect((tx, ty), (sx, sy), (aw, ah));
    let p_last = clip_rect((sx, sy), (tx, ty), (bw, bh));
    let clipped = vec![p_first, p_last];

    let markers = if r.bidirectional {
        "marker-start=\"url(#c4-arrow)\" marker-end=\"url(#c4-arrow)\""
    } else {
        "marker-end=\"url(#c4-arrow)\""
    };

    // Upstream draws relations as a quadratic Bézier through the routed midpoint.
    let path = quad_path(&clipped);
    svg.path(
        &path,
        &format!("fill=\"none\" stroke=\"{stroke}\" stroke-width=\"1\" {markers}"),
    );

    let label = &r.label;
    let tech = r.technology.as_deref();
    if label.is_empty() && tech.is_none() {
        return;
    }
    let (mut mx, mut my) = polyline_midpoint(&clipped);
    if let Some(s) = ov {
        mx += s.offset_x.unwrap_or(0.0);
        my += s.offset_y.unwrap_or(0.0);
    }
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
}

/// Quadratic Bézier from the first to the last point, bent through the routed
/// midpoint as its control point (matching upstream's `M … Q …` rel curves).
/// A straight two-point path collapses to a plain line.
fn quad_path(pts: &[(f64, f64)]) -> String {
    let start = pts[0];
    let end = pts[pts.len() - 1];
    let (mx, my) = polyline_midpoint(pts);
    // Lift the control point so the curve actually passes through the midpoint at t=0.5.
    let cx = 2.0 * mx - (start.0 + end.0) / 2.0;
    let cy = 2.0 * my - (start.1 + end.1) / 2.0;
    format!(
        "M{} {} Q{} {} {} {}",
        fnum(start.0),
        fnum(start.1),
        fnum(cx),
        fnum(cy),
        fnum(end.0),
        fnum(end.1),
    )
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

    fn boundary(
        alias: &str,
        label: &str,
        kind: C4BoundaryKind,
        members: Vec<C4Element>,
    ) -> C4Element {
        C4Element {
            kind: C4ElementKind::System,
            alias: alias.into(),
            label: label.into(),
            descr: None,
            technology: None,
            external: false,
            boundary_alias: None,
            boundary_label: None,
            boundary_kind: Some(kind),
            members,
        }
    }

    #[test]
    fn produces_svg() {
        let d = C4Diagram {
            kind: C4Kind::Context,
            title: Some("Sys".into()),
            elements: vec![person("u", "User")],
            relations: vec![],
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">User<"));
        assert!(svg.contains(">Sys<"));
    }

    fn container(alias: &str, label: &str, members: Vec<C4Element>) -> C4Element {
        boundary(alias, label, C4BoundaryKind::Deployment, members)
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
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());

        // Boundary rects carry the `rx="2.5"` corner (elements use rx="6"). Find
        // each and check its `y` clears the subtitle baseline.
        let subtitle_baseline = PAD + 38.0;
        let mut checked = false;
        for chunk in svg.split("<rect").skip(1) {
            if !chunk.contains("rx=\"2.5\"") {
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());
        // The connector path must not be dashed (only the boundary outline is).
        assert!(!svg.contains("stroke-dasharray=\"5 4\""));
    }

    #[test]
    fn deployment_node_boundary_is_solid() {
        let d = C4Diagram {
            kind: C4Kind::Deployment,
            title: None,
            elements: vec![boundary(
                "dn",
                "Server",
                C4BoundaryKind::Deployment,
                vec![person("a", "A"), person("b", "B")],
            )],
            relations: vec![],
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());
        // Solid border: #444444, width 1, no dasharray on the boundary rect.
        assert!(svg.contains("stroke=\"#444444\" stroke-width=\"1\" rx=\"2.5\""));
        assert!(!svg.contains("stroke-dasharray"));
        assert!(svg.contains(">[Deployment Node]<"));
    }

    #[test]
    fn generic_boundary_is_dashed_7_7() {
        let d = C4Diagram {
            kind: C4Kind::Context,
            title: None,
            elements: vec![boundary(
                "b",
                "Group",
                C4BoundaryKind::System,
                vec![person("a", "A"), person("b", "B")],
            )],
            relations: vec![],
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.contains(
            "stroke=\"#444444\" stroke-width=\"1\" rx=\"2.5\" ry=\"2.5\" stroke-dasharray=\"7 7\""
        ));
    }

    #[test]
    fn rel_is_curved_and_unbacked() {
        let d = C4Diagram {
            kind: C4Kind::Context,
            title: None,
            elements: vec![person("a", "A"), person("b", "B")],
            relations: vec![C4Relation {
                from: "a".into(),
                to: "b".into(),
                label: "uses".into(),
                technology: Some("HTTPS".into()),
                direction: C4RelDirection::Default,
                bidirectional: false,
            }],
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());
        // Quadratic Bézier, #444444, width 1.
        assert!(svg.contains(" Q"));
        assert!(svg.contains("stroke=\"#444444\" stroke-width=\"1\""));
        // No translucent label background rect.
        assert!(!svg.contains("fill-opacity=\"0.5\""));
        // techn rendered italic as [HTTPS].
        assert!(svg.contains(">[HTTPS]<"));
    }

    fn overlaps(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
        a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
    }

    #[test]
    fn sibling_boundaries_do_not_overlap() {
        // Four Deployment_Node boundaries (as in the CyberScore repro), each
        // holding a couple of shapes. None of the frames may overlap.
        let elements = vec![
            boundary(
                "app17",
                "app17",
                C4BoundaryKind::Deployment,
                vec![person("a1", "A1"), person("a2", "A2")],
            ),
            boundary(
                "app06",
                "app06",
                C4BoundaryKind::Deployment,
                vec![person("b1", "B1"), person("b2", "B2")],
            ),
            boundary(
                "app14",
                "app14",
                C4BoundaryKind::Deployment,
                vec![person("c1", "C1")],
            ),
            boundary(
                "app16",
                "app16",
                C4BoundaryKind::Deployment,
                vec![person("d1", "D1")],
            ),
        ];

        let (nodes, _, _) = flow_layout(&elements, SHAPE_IN_ROW, BOUNDARY_IN_ROW);
        let mut pos = HashMap::new();
        let mut boundaries = Vec::new();
        let mut leaves = Vec::new();
        place_absolute(&nodes, PAD, PAD, &mut pos, &mut boundaries, &mut leaves);

        assert_eq!(boundaries.len(), 4);
        for (i, a) in boundaries.iter().enumerate() {
            for b in &boundaries[i + 1..] {
                let ra = (a.x, a.y, a.w, a.h);
                let rb = (b.x, b.y, b.w, b.h);
                assert!(
                    !overlaps(ra, rb),
                    "boundary frames overlap: {ra:?} vs {rb:?}"
                );
            }
        }
    }

    #[test]
    fn boundary_contains_its_members() {
        let elements = vec![boundary(
            "app",
            "app",
            C4BoundaryKind::Deployment,
            vec![person("x", "X"), person("y", "Y")],
        )];
        let (nodes, _, _) = flow_layout(&elements, SHAPE_IN_ROW, BOUNDARY_IN_ROW);
        let mut pos = HashMap::new();
        let mut boundaries = Vec::new();
        let mut leaves = Vec::new();
        place_absolute(&nodes, PAD, PAD, &mut pos, &mut boundaries, &mut leaves);

        let b = &boundaries[0];
        for (_, x, y, w, h) in &leaves {
            assert!(
                *x >= b.x && *x + *w <= b.x + b.w,
                "member escapes boundary x"
            );
            assert!(
                *y >= b.y && *y + *h <= b.y + b.h,
                "member escapes boundary y"
            );
        }
    }

    #[test]
    fn element_style_override_applies_colors() {
        let d = C4Diagram {
            kind: C4Kind::Context,
            title: None,
            elements: vec![C4Element {
                kind: C4ElementKind::System,
                alias: "s".into(),
                label: "Sys".into(),
                descr: None,
                technology: None,
                external: false,
                boundary_alias: None,
                boundary_label: None,
                boundary_kind: None,
                members: vec![],
            }],
            relations: vec![],
            element_styles: vec![C4ElementStyle {
                alias: "s".into(),
                bg_color: Some("#ABCDEF".into()),
                font_color: None,
                border_color: Some("#123456".into()),
            }],
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("fill=\"#ABCDEF\""));
        assert!(svg.contains("stroke=\"#123456\""));
    }

    #[test]
    fn rel_style_override_colors_line_and_label() {
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
            rel_styles: vec![C4RelStyle {
                from: "a".into(),
                to: "b".into(),
                text_color: Some("#00FF00".into()),
                line_color: Some("#FF0000".into()),
                offset_x: None,
                offset_y: None,
            }],
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("stroke=\"#FF0000\""));
        assert!(svg.contains("fill=\"#00FF00\""));
    }

    #[test]
    fn layout_config_controls_shapes_per_row() {
        // With shape_in_row = 1 the two shapes stack vertically; default (4)
        // would place them on the same row. Verify the override changes layout.
        let elements = vec![person("a", "A"), person("b", "B")];
        let (nodes, _, _) = flow_layout(&elements, 1, BOUNDARY_IN_ROW);
        assert_eq!(nodes.len(), 2);
        assert!(
            nodes[1].y > nodes[0].y,
            "second shape should wrap to the next row"
        );
        assert_eq!(nodes[0].x, nodes[1].x, "wrapped shapes share the left edge");
    }
}
