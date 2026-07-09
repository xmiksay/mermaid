//! Mindmap renderer. Deterministic radial tree fanning out from a central root
//! circle, matching upstream Mermaid's radial silhouette: branch-colored filled
//! rounded nodes and thick edges from the categorical theme scale.

use std::f64::consts::{FRAC_PI_2, PI};

use crate::parse::{MindmapDiagram, MindmapNode};

use crate::svg::builder::SvgBuilder;
use crate::svg::theme::Theme;

mod draw;
mod layout;

#[cfg(test)]
mod tests;

const NODE_PAD_X: f64 = 14.0;
const NODE_H: f64 = 32.0;
/// Radius added per depth level; first ring sits this far from the centre.
const RING_GAP: f64 = 130.0;
const TEXT_PX: f64 = 7.0;
const ICON_SIZE: f64 = 16.0;
/// Gap between an in-node icon glyph and its label text.
const ICON_GAP: f64 = 6.0;
/// Fraction of a non-root parent's angular sector its children occupy, centred
/// on the parent's radial line. Below 1 it pulls each subtree into a compact
/// cone around its branch node instead of fanning it across the full inherited
/// sector — the fix for the sprawling long diagonal edges.
const CHILD_SPREAD: f64 = 0.72;

/// Drop-shadow filter for the borderless nodes (upstream renders shadowed,
/// borderless nodes rather than uniformly bordered rects).
const SHADOW_FILTER: &str = "<filter id=\"mm-shadow\" x=\"-30%\" y=\"-30%\" width=\"160%\" height=\"160%\">\
     <feDropShadow dx=\"0\" dy=\"1.5\" stdDeviation=\"2\" flood-color=\"#000000\" flood-opacity=\"0.2\"/></filter>";

#[derive(Clone)]
struct Laid {
    node: MindmapNode,
    /// Node centre in layout space.
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    /// Radius of the root circle (only meaningful at `depth == 0`).
    r: f64,
    /// Label font size (px) for this node, scaled down by depth.
    font: f64,
    depth: usize,
    /// Index of the first-level branch this node belongs to (`-1` for the root),
    /// used to pick a branch color from the theme scale.
    section: i32,
    children: Vec<Laid>,
}

pub(crate) fn render(d: &MindmapDiagram, theme: &Theme) -> String {
    let Some(root) = d.root.clone() else {
        let mut svg = SvgBuilder::new(200.0, 80.0).theme(theme);
        svg.text(
            100.0,
            40.0,
            &format!(
                "text-anchor=\"middle\" fill=\"{}\" font-size=\"13\"",
                &theme.fg_muted
            ),
            "(empty mindmap)",
        );
        return svg.finish();
    };

    let font_size = theme.font_size;
    // Root sits at the origin; its children are dealt around the full circle by
    // angular sector (proportional to each subtree's leaf count), and every
    // descendant is fanned outward within its parent's sector.
    let mut laid = layout::build(&root, 0, -1, -FRAC_PI_2, -FRAC_PI_2 + 2.0 * PI, font_size);

    // Frame the whole radial layout and shift into positive space.
    let margin = 24.0;
    let (min_x, min_y, max_x, max_y) = layout::bounds(&laid);
    layout::shift(&mut laid, margin - min_x, margin - min_y);
    let width = (max_x - min_x) + margin * 2.0;
    let height = (max_y - min_y) + margin * 2.0;

    let mut svg = SvgBuilder::new(width, height).theme(theme);
    svg.defs_raw(SHADOW_FILTER);

    draw::draw_edges(&laid, &mut svg, theme);
    draw::draw_nodes(&laid, &mut svg, theme, &d.class_defs);

    svg.finish()
}
