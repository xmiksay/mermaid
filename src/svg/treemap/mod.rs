//! Treemap renderer. Squarified layout (Bruls/Huizing/van Wijk greedy
//! worst-aspect-ratio row packing) matching upstream d3 treemaps — rectangles
//! stay near square instead of degenerating into long thin slivers. Leaf values
//! are formatted through the `valueFormat` d3-format subset.

use std::collections::HashMap;

use crate::parse::ast::Style;
use crate::parse::{TreemapDiagram, TreemapNode};

use super::builder::SvgBuilder;
use super::theme::Theme;

mod draw;
mod format;
mod squarify;
#[cfg(test)]
mod tests;

use draw::draw_node;
use squarify::squarify;

const PAD: f64 = 24.0;
const TITLE_GAP: f64 = 32.0;
const CHART_W: f64 = 640.0;
const CHART_H: f64 = 420.0;
const HEADER_H: f64 = 22.0;

/// Shared, read-only context threaded through the recursive layout.
struct Ctx<'a> {
    theme: &'a Theme,
    class_defs: &'a HashMap<String, Style>,
    value_format: Option<&'a str>,
    show_values: bool,
}

#[derive(Clone, Copy)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

pub(crate) fn render(d: &TreemapDiagram, theme: &Theme) -> String {
    let fg = &theme.fg;
    let title_h = if d.title.is_some() { TITLE_GAP } else { 0.0 };
    let width = PAD * 2.0 + CHART_W;
    let height = PAD * 2.0 + title_h + CHART_H;
    let mut svg = SvgBuilder::new(width, height).theme(theme);

    if let Some(t) = &d.title {
        svg.text(
            width / 2.0,
            PAD + 18.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\""),
            t,
        );
    }

    let ctx = Ctx {
        theme,
        class_defs: &d.class_defs,
        value_format: d.value_format.as_deref(),
        show_values: d.show_values != Some(false),
    };
    let area = Rect {
        x: PAD,
        y: PAD + title_h,
        w: CHART_W,
        h: CHART_H,
    };
    let mut next_id = 0usize;
    let mut next_color = 0usize;
    layout(
        &d.root,
        area,
        None,
        &mut svg,
        &ctx,
        &mut next_id,
        &mut next_color,
    );

    svg.finish()
}

fn node_value(n: &TreemapNode) -> f64 {
    if let Some(v) = n.value {
        return v;
    }
    let s: f64 = n.children.iter().map(node_value).sum();
    if s == 0.0 {
        1.0
    } else {
        s
    }
}

/// Order sibling indices by value descending, ties keeping source order
/// (`sort_by` is stable). Upstream sorts every level this way.
fn order_by_value(nodes: &[TreemapNode]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..nodes.len()).collect();
    idx.sort_by(|&a, &b| node_value(&nodes[b]).total_cmp(&node_value(&nodes[a])));
    idx
}

fn layout(
    nodes: &[TreemapNode],
    area: Rect,
    parent_color: Option<&str>,
    svg: &mut SvgBuilder,
    ctx: &Ctx,
    next_id: &mut usize,
    next_color: &mut usize,
) {
    if nodes.is_empty() || area.w <= 2.0 || area.h <= 2.0 {
        return;
    }
    let order = order_by_value(nodes);
    let values: Vec<f64> = order.iter().map(|&i| node_value(&nodes[i])).collect();
    let rects = squarify(&values, area);
    for (&i, r) in order.iter().zip(rects.iter()) {
        let n = &nodes[i];
        // Every section takes the next palette hue in traversal order; its
        // leaves inherit that hue uniformly. A top-level leaf (no parent
        // section) also gets its own hue. This matches upstream, where each
        // branch is one flat color and nested sections switch hue.
        let color = match (n.children.is_empty(), parent_color) {
            (true, Some(pc)) => pc.to_string(),
            _ => {
                let c = ctx.theme.cscale_color(*next_color).to_string();
                *next_color += 1;
                c
            }
        };
        draw_node(n, *r, &color, svg, ctx, next_id);
        if !n.children.is_empty() && r.w > 30.0 && r.h > HEADER_H + 10.0 {
            let inner = Rect {
                x: r.x + 4.0,
                y: r.y + HEADER_H,
                w: r.w - 8.0,
                h: r.h - HEADER_H - 4.0,
            };
            layout(
                &n.children,
                inner,
                Some(&color),
                svg,
                ctx,
                next_id,
                next_color,
            );
        }
    }
}
