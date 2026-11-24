//! C4 diagram renderer.
//!
//! Layout is hybrid. Small diagrams keep upstream's row-flow grid
//! (`grid.rs`, #327/#370); diagrams with more than [`GRID_MAX_RELATIONS`]
//! relations, or with an explicit `LAYOUT_TOP_DOWN()` / `LAYOUT_LEFT_RIGHT()`,
//! use the layered layout (`graph/`), with boundaries as clusters and
//! relations routed as splines. Both modes size every shape from its wrapped
//! text and share relation drawing (`rel.rs`): ends clipped to the drawn
//! outline, wrapped labels placed clear of shapes and of each other.
//!
//! Boundaries are drawn as an outline around their content: dashed `7.0,7.0` for
//! most kinds, but solid for `Deployment_Node` (matching upstream's `nodeType`
//! special-case). Stroke is `#444444`, width 1.

use std::collections::HashMap;

use crate::parse::{C4BoundaryKind, C4Diagram, C4Element, C4ElementStyle, C4RelStyle};

use super::builder::{fnum, SvgBuilder};
use super::metrics::text_width;
use super::theme::Theme;

mod arc_path;
mod graph;
mod grid;
mod grid_route;
mod outline;
mod rel;
mod rel_annotate;
mod rel_label;
mod rel_loop;
mod rel_place;
mod rel_route;
mod rel_straight;
mod rel_text;
mod shapes;
#[cfg(test)]
mod tests;
mod text;
mod wrap;

use outline::{bounds, Rect, Span};
use rel::{draw_rels, min_label_gaps, plan_rels, RelPlan, ARROW_ID, ARROW_REF_X, ARROW_SIZE};
use shapes::{draw_element, resolve_element_style};

const PAD: f64 = 32.0;
const TITLE_GAP: f64 = 56.0;

const BOUNDARY_HDR: f64 = 28.0;
const BOUNDARY_PAD: f64 = 20.0;
const BOUNDARY_MIN_W: f64 = 200.0;

/// Diagrams with at most this many relations keep upstream's grid; beyond it
/// a grid's straight centre-to-centre connectors cross shapes and each other
/// (#396), so the layered layout takes over.
const GRID_MAX_RELATIONS: usize = 8;

/// Boundary outlines and relation lines both use upstream's `#444444`.
pub(super) const C4_LINE: &str = "#444444";

/// Where everything sits, independent of layout mode.
#[derive(Default)]
struct Placement {
    leaves: Vec<(C4Element, Rect)>,
    boundaries: Vec<BoundaryBox>,
    /// Per relation, a route from the source centre to the target centre;
    /// `None` draws a straight connector.
    routes: Vec<Option<Vec<(f64, f64)>>>,
    /// Routes are drawn as splines (graph mode) rather than straight lines.
    curved: bool,
    /// Relation labels wrap narrower (left-right graph mode).
    narrow_labels: bool,
}

impl Placement {
    fn shift(&mut self, dx: f64, dy: f64) {
        for (_, r) in &mut self.leaves {
            r.0 += dx;
            r.1 += dy;
        }
        for b in &mut self.boundaries {
            b.rect.0 += dx;
            b.rect.1 += dy;
        }
    }
}

struct BoundaryBox {
    alias: String,
    label: String,
    kind: C4BoundaryKind,
    /// Explicit type argument (overrides the fixed per-kind `[label]` tag).
    type_text: Option<String>,
    rect: Rect,
    /// Width of the header text (title / `[kind]` line), from `header_min_w`.
    header_w: f64,
}

/// Room a route keeps past the end of a boundary's header text.
const HEADER_TEXT_CLEARANCE: f64 = 6.0;

impl BoundaryBox {
    /// The header text block — title and `[kind]` line, whichever is wider,
    /// plus `HEADER_TEXT_CLEARANCE` — which routes must not run through.
    fn header_text(&self) -> Rect {
        let (x, y, ..) = self.rect;
        (
            x + 14.0,
            y + 4.0,
            self.header_w + HEADER_TEXT_CLEARANCE,
            36.0,
        )
    }
}

fn uses_graph_layout(d: &C4Diagram) -> bool {
    d.layout_direction.is_some() || drawn_relations(d) > GRID_MAX_RELATIONS
}

/// Relations that become a routed line: both ends declared (a relation to
/// an undefined alias is dropped) and distinct (a self-relation is a loop
/// on its shape, which no layout needs to route).
fn drawn_relations(d: &C4Diagram) -> usize {
    let mut declared: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut stack: Vec<&C4Element> = d.elements.iter().collect();
    while let Some(el) = stack.pop() {
        declared.insert(el.alias.as_str());
        stack.extend(el.members.iter());
    }
    d.relations
        .iter()
        .filter(|r| r.from != r.to)
        .filter(|r| declared.contains(r.from.as_str()) && declared.contains(r.to.as_str()))
        .count()
}

/// Fully resolved geometry, before anything is drawn.
struct Scene<'a> {
    placement: Placement,
    rels: Vec<RelPlan<'a>>,
    width: f64,
    height: f64,
}

fn layout_scene<'a>(
    d: &'a C4Diagram,
    rel_styles: &HashMap<(&str, &str), &C4RelStyle>,
    theme: &Theme,
) -> Scene<'a> {
    let title_h = if d.title.is_some() { TITLE_GAP } else { 0.0 };
    let grid =
        || grid::grid_placement(d, (0.0, 0.0), theme.font_size, min_label_gaps(&d.relations));
    // The graph construction never feeds sugiyama an invalid graph; should
    // it ever, the grid still renders every element.
    let mut placement = if uses_graph_layout(d) {
        graph::graph_placement(d, theme.font_size).unwrap_or_else(|_| grid())
    } else {
        grid()
    };
    let mut rels = plan_rels(&d.relations, &d.elements, &placement, rel_styles);

    // Measure everything (shapes, frames, lines, labels), then shift so the
    // content's top-left lands at the padding.
    let Span { x0, y0, x1, y1 } = bounds(
        placement
            .leaves
            .iter()
            .map(|(_, r)| *r)
            .chain(placement.boundaries.iter().map(|b| b.rect))
            .chain(rels.iter().filter_map(RelPlan::label_rect))
            .chain(
                rels.iter()
                    .flat_map(|p| p.line.iter().map(|&(x, y)| (x, y, 0.0, 0.0))),
            ),
    )
    .unwrap_or(Span::new(0.0, 0.0, 0.0, 0.0));
    let (dx, dy) = (PAD - x0, PAD + title_h - y0);
    placement.shift(dx, dy);
    for p in &mut rels {
        p.shift(dx, dy);
    }
    let title_w = d
        .title
        .as_deref()
        .map_or(0.0, |t| text_width(t, 10.0, 18.0) + 2.0 * PAD);
    Scene {
        placement,
        rels,
        width: (x1 + dx + PAD).max(600.0).max(title_w),
        height: (y1 + dy + PAD).max(220.0),
    }
}

pub(crate) fn render(d: &C4Diagram, theme: &Theme) -> String {
    let fg = &theme.fg;
    let rel_styles: HashMap<(&str, &str), &C4RelStyle> = d
        .rel_styles
        .iter()
        .map(|s| ((s.from.as_str(), s.to.as_str()), s))
        .collect();
    let scene = layout_scene(d, &rel_styles, theme);
    let (width, height) = (scene.width, scene.height);
    let mut svg = SvgBuilder::new(width, height).theme(theme);

    svg.def_arrow_marker(ARROW_ID, C4_LINE, ARROW_REF_X, ARROW_SIZE);

    if let Some(t) = &d.title {
        svg.text(
            width / 2.0,
            PAD + 22.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\""),
            t,
        );
    }

    let boundary_styles: HashMap<&str, &C4ElementStyle> = d
        .boundary_styles
        .iter()
        .map(|s| (s.alias.as_str(), s))
        .collect();
    for b in &scene.placement.boundaries {
        let ov = boundary_styles.get(b.alias.as_str()).copied();
        draw_boundary_rect(b, ov, &mut svg, theme);
    }

    let elem_styles: HashMap<&str, &C4ElementStyle> = d
        .element_styles
        .iter()
        .map(|s| (s.alias.as_str(), s))
        .collect();
    for (el, (x, y, w, h)) in &scene.placement.leaves {
        let ov = elem_styles.get(el.alias.as_str()).copied();
        let style = resolve_element_style(el, ov);
        draw_element(el, &style, *x, *y, *w, *h, &mut svg, theme);
    }

    draw_rels(&scene.rels, &rel_styles, &mut svg, theme);

    svg.finish()
}

/// Minimum boundary width so the header label and the `[kind]` tag (stacked
/// below it) don't get clipped.
fn header_min_w(b: &C4Element, font_size: f64) -> f64 {
    let kind = boundary_tag_text(
        b.boundary_kind.unwrap_or(C4BoundaryKind::Generic),
        &b.boundary_type,
    );
    // Measured at the sizes `draw_boundary_rect` draws them: the bold title
    // two points up, the italic tag at 10.
    let label_w = super::metrics::proportional_width(&b.label, font_size + 2.0, true);
    let kind_w = super::metrics::proportional_width(&format!("[{kind}]"), 10.0, false);
    label_w.max(kind_w) + 28.0
}

/// The `[…]` header tag: an explicit type argument overrides the fixed per-kind
/// label (e.g. `Deployment_Node(n, "Web Server", "Ubuntu 16.04 LTS")` shows the
/// OS text).
fn boundary_tag_text(kind: C4BoundaryKind, type_text: &Option<String>) -> String {
    match type_text {
        Some(t) if !t.is_empty() => t.clone(),
        _ => boundary_kind_label(kind).to_string(),
    }
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

fn draw_boundary_rect(
    b: &BoundaryBox,
    style: Option<&C4ElementStyle>,
    svg: &mut SvgBuilder,
    theme: &Theme,
) {
    // `UpdateBoundaryStyle` overrides the outline/fill/label colors.
    let fill = style
        .and_then(|s| s.bg_color.clone())
        .unwrap_or_else(|| "none".to_string());
    let stroke = style
        .and_then(|s| s.border_color.clone())
        .unwrap_or_else(|| C4_LINE.to_string());
    let fg = style
        .and_then(|s| s.font_color.clone())
        .unwrap_or_else(|| theme.fg.to_string());
    let fg_muted = &theme.fg_muted;
    // Upstream draws a `Deployment_Node` (any boundary with a `nodeType`) with a
    // solid border and every other boundary kind dashed `7.0,7.0`.
    let dash = if matches!(b.kind, C4BoundaryKind::Deployment) {
        String::new()
    } else {
        " stroke-dasharray=\"7 7\"".to_string()
    };
    let (bx, by, bw, bh) = b.rect;
    svg.rect(
        bx,
        by,
        bw,
        bh,
        &format!(
            "fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1\" rx=\"2.5\" ry=\"2.5\"{dash}"
        ),
    );
    let kind = boundary_tag_text(b.kind, &b.type_text);
    let label_size = theme.font_size + 2.0;
    svg.text(
        bx + 14.0,
        by + 18.0,
        &format!(
            "fill=\"{fg}\" font-size=\"{}\" font-weight=\"bold\"",
            fnum(label_size)
        ),
        &b.label,
    );
    svg.text(
        bx + 14.0,
        by + 18.0 + label_size,
        &format!("fill=\"{fg_muted}\" font-size=\"10\" font-style=\"italic\""),
        &format!("[{kind}]"),
    );
}
