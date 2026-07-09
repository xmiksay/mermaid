//! Sankey diagram renderer.
//!
//! Layout: nodes are assigned to columns by their distance from a source
//! (topological depth). Within a column, nodes are stacked top-to-bottom
//! sized proportionally to throughput. Links are drawn as cubic Béziers
//! whose stroke-width matches the flow value.

mod columns;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::parse::SankeyDiagram;

use super::builder::{fnum, SvgBuilder};
use super::sankey_layout;
use super::theme::Theme;
use columns::{assign_columns, Alignment};

const PAD: f64 = 30.0;
const COL_GAP: f64 = 180.0;
// Upstream defaults (config.schema.yaml SankeyDiagramConfig).
const NODE_W: f64 = 10.0;
const CHART_H: f64 = 400.0;
const ROW_GAP: f64 = 12.0;
// Extra vertical padding d3-sankey layout reserves for the value line when
// `showValues` is on (`nodePadding + 15` in the upstream renderer).
const VALUE_PAD: f64 = 15.0;

/// Per-node fill scale used by upstream's sankey renderer: it colors nodes with
/// a hardcoded `d3.scaleOrdinal(d3.schemeTableau10)` keyed by node id, i.e. the
/// Tableau-10 palette cycled in first-appearance order — independent of the
/// diagram theme's pastel `cScale` palette.
const TABLEAU10: [&str; 10] = [
    "#4e79a7", "#f28e2c", "#e15759", "#76b7b2", "#59a14f", "#edc949", "#af7aa1", "#ff9da7",
    "#9c755f", "#bab0ac",
];

pub(crate) fn render(d: &SankeyDiagram, theme: &Theme) -> String {
    let fg = &theme.fg;

    // Geometry is config-driven (`config.sankey.*`); each falls back to the
    // upstream-faithful default when unset.
    let node_w = d.node_width.unwrap_or(NODE_W);
    let row_gap = d.node_padding.unwrap_or(ROW_GAP);
    let chart_h = d.height.unwrap_or(CHART_H);
    let show_values = d.show_values.unwrap_or(true);
    let prefix = d.prefix.as_deref().unwrap_or("");
    let suffix = d.suffix.as_deref().unwrap_or("");

    if d.links.is_empty() {
        let mut svg = SvgBuilder::new(200.0, 80.0).theme(theme);
        svg.text(
            100.0,
            40.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"13\""),
            "(empty sankey)",
        );
        return svg.finish();
    }

    // Collect nodes preserving first-seen order.
    let mut order: Vec<String> = Vec::new();
    let mut seen = BTreeSet::new();
    for l in &d.links {
        for n in [&l.source, &l.target] {
            if seen.insert(n.clone()) {
                order.push(n.clone());
            }
        }
    }

    // Per-node color: upstream sankey cycles the fixed Tableau-10 scheme in
    // first-appearance order (not the theme palette); links inherit from it via
    // `linkColor`.
    let index: BTreeMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();
    let node_color = |n: &str| TABLEAU10[index[n] % TABLEAU10.len()];

    // Assign a column per node honoring `nodeAlignment`.
    let col = assign_columns(
        &order,
        &d.links,
        Alignment::parse(d.node_alignment.as_deref()),
    );
    let max_col = *col.values().max().unwrap_or(&0);
    let cols: usize = (max_col + 1) as usize;

    // `config.sankey.width` sets the total node-area span; without it fall back
    // to a fixed per-column gap.
    let col_gap = match d.width {
        Some(w) if cols > 1 => ((w - node_w) / (cols - 1) as f64).max(node_w + 20.0),
        _ => COL_GAP,
    };

    // Throughput per node.
    let mut out_sum: BTreeMap<String, f64> = BTreeMap::new();
    let mut in_sum: BTreeMap<String, f64> = BTreeMap::new();
    for l in &d.links {
        *out_sum.entry(l.source.clone()).or_default() += l.value;
        *in_sum.entry(l.target.clone()).or_default() += l.value;
    }
    let through = |n: &str| -> f64 {
        let i = *in_sum.get(n).unwrap_or(&0.0);
        let o = *out_sum.get(n).unwrap_or(&0.0);
        i.max(o)
    };

    // Group nodes by column, ordered top-to-bottom the way d3-sankey would
    // (first appearance, then barycenter relaxation minimising crossings)
    // rather than raw first-appearance order.
    let values: Vec<f64> = order.iter().map(|n| through(n)).collect();
    // Order must be computed with upstream's own d3-sankey layout inputs: the
    // vertical extent is the `height` config and the padding is
    // `nodePadding + 15` when `showValues` is on. Feeding our proportional
    // drawing gap here instead (the old bug) diverged from JS Mermaid's column
    // order — the relaxation is padding-sensitive.
    let layout_pad = row_gap + if show_values { VALUE_PAD } else { 0.0 };
    let ordered =
        sankey_layout::order_columns(&order, &col, &d.links, &values, chart_h, layout_pad);
    let mut by_col: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for (c, nodes) in ordered.iter().enumerate() {
        by_col.insert(c as u32, nodes.iter().map(|&i| order[i].clone()).collect());
    }

    // Compute height scale: max column total throughput maps to CHART_H.
    let mut col_totals: BTreeMap<u32, f64> = BTreeMap::new();
    for (c, ns) in &by_col {
        let t: f64 = ns.iter().map(|n| through(n)).sum();
        col_totals.insert(*c, t);
    }
    let col_max = col_totals
        .values()
        .cloned()
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let scale = (chart_h
        - (by_col
            .values()
            .map(|v| v.len())
            .max()
            .unwrap_or(1)
            .saturating_sub(1) as f64)
            * row_gap)
        / col_max;

    // Position rectangles: x by column, y stacked.
    let mut rects: BTreeMap<String, (f64, f64, f64)> = BTreeMap::new(); // id -> (x, y, h)
    for (c, ns) in &by_col {
        let x = PAD + *c as f64 * col_gap;
        let mut y = PAD + 10.0;
        for n in ns {
            let h = (through(n) * scale).max(2.0);
            rects.insert(n.clone(), (x, y, h));
            y += h + row_gap;
        }
    }

    let width = PAD * 2.0 + (cols.saturating_sub(1) as f64) * col_gap + node_w + 120.0;
    let height = PAD * 2.0 + chart_h + 30.0;
    let mut svg = SvgBuilder::new(width, height).theme(theme);

    let link_color = LinkColor::parse(d.link_color.as_deref());

    // Track per-node offset cursors for stacking link stubs.
    let mut out_cursor: BTreeMap<String, f64> = BTreeMap::new();
    let mut in_cursor: BTreeMap<String, f64> = BTreeMap::new();

    for (li, l) in d.links.iter().enumerate() {
        let (sx, sy, sh) = rects[&l.source];
        let (tx, ty, th) = rects[&l.target];
        let sw = (l.value * scale).max(0.5);
        let so = out_cursor.entry(l.source.clone()).or_insert(0.0);
        let to = in_cursor.entry(l.target.clone()).or_insert(0.0);
        let y1 = sy + *so * (sh / out_sum[&l.source].max(1e-9)) + sw / 2.0;
        let y2 = ty
            + *to * (th / in_sum.get(&l.target).copied().unwrap_or(l.value).max(1e-9))
            + sw / 2.0;
        *so += l.value;
        *to += l.value;
        let x1 = sx + node_w;
        let x2 = tx;
        let mx = (x1 + x2) / 2.0;
        // Derive the stroke from `linkColor`; `gradient` needs a per-link
        // `<linearGradient>` in <defs> spanning the source→target span.
        let stroke = match &link_color {
            LinkColor::Source => node_color(&l.source).to_string(),
            LinkColor::Target => node_color(&l.target).to_string(),
            LinkColor::Fixed(hex) => hex.clone(),
            LinkColor::Gradient => {
                let gid = format!("sankey-grad-{li}");
                svg.defs_raw(&format!(
                    "<linearGradient id=\"{gid}\" gradientUnits=\"userSpaceOnUse\" \
                     x1=\"{}\" x2=\"{}\"><stop offset=\"0\" stop-color=\"{}\"/>\
                     <stop offset=\"1\" stop-color=\"{}\"/></linearGradient>",
                    fnum(x1),
                    fnum(x2),
                    node_color(&l.source),
                    node_color(&l.target),
                ));
                format!("url(#{gid})")
            }
        };
        let mut path = String::new();
        let _ = write!(
            path,
            "M{} {}C{} {}, {} {}, {} {}",
            fnum(x1),
            fnum(y1),
            fnum(mx),
            fnum(y1),
            fnum(mx),
            fnum(y2),
            fnum(x2),
            fnum(y2),
        );
        svg.path(
            &path,
            &format!(
                "fill=\"none\" stroke=\"{stroke}\" stroke-opacity=\"0.45\" stroke-width=\"{}\"",
                fnum(sw)
            ),
        );
    }

    // Draw node rects + labels.
    for (id, (x, y, h)) in &rects {
        svg.rect(
            *x,
            *y,
            node_w,
            *h,
            &format!("fill=\"{}\" stroke=\"#fff\"", node_color(id)),
        );
        let label_x = if col[id] == max_col {
            *x - 6.0
        } else {
            *x + node_w + 6.0
        };
        let anchor = if col[id] == max_col { "end" } else { "start" };
        // Upstream `showValues` (on by default) shows the node throughput after
        // the name on a *single* line. Its label string is `Name\n<prefix>42
        // <suffix>`, but SVG `<text>` collapses that newline to whitespace, so
        // it renders as one line ("Coal 300"); emit a space to match rather than
        // stacking two `<tspan>`s.
        let label = if show_values {
            format!("{id} {prefix}{}{suffix}", fnum(through(id)))
        } else {
            id.clone()
        };
        svg.text(
            label_x,
            y + h / 2.0 + 5.0,
            &format!("text-anchor=\"{anchor}\" fill=\"{fg}\" font-size=\"14\""),
            &label,
        );
    }

    svg.finish()
}

/// How a link's stroke color is derived (`config.sankey.linkColor`).
enum LinkColor {
    /// The source node's palette color.
    Source,
    /// The target node's palette color.
    Target,
    /// A source→target `<linearGradient>` per link (upstream default).
    Gradient,
    /// A literal color (any hex the config supplies).
    Fixed(String),
}

impl LinkColor {
    fn parse(s: Option<&str>) -> Self {
        match s.map(str::trim) {
            Some("source") => LinkColor::Source,
            Some("target") => LinkColor::Target,
            Some("gradient") | None => LinkColor::Gradient,
            // Anything else is treated as a literal color (e.g. `#a1b2c3`).
            Some(other) => LinkColor::Fixed(other.to_string()),
        }
    }
}
