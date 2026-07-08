//! Sankey diagram renderer.
//!
//! Layout: nodes are assigned to columns by their distance from a source
//! (topological depth). Within a column, nodes are stacked top-to-bottom
//! sized proportionally to throughput. Links are drawn as cubic Béziers
//! whose stroke-width matches the flow value.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use crate::parse::SankeyDiagram;

use super::builder::{fnum, SvgBuilder};
use super::sankey_layout;
use super::theme::Theme;

const PAD: f64 = 30.0;
const COL_GAP: f64 = 180.0;
// Upstream defaults (config.schema.yaml SankeyDiagramConfig).
const NODE_W: f64 = 10.0;
const CHART_H: f64 = 380.0;
const ROW_GAP: f64 = 6.0;

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

    // Per-node palette color (upstream cycles a scheme per node); links inherit
    // from it via `linkColor`.
    let index: BTreeMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();
    let node_color = |n: &str| theme.cscale_color(index[n]);

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
    let ordered = sankey_layout::order_columns(&order, &col, &d.links, &values, chart_h, row_gap);
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
        // the name (`Name\n<prefix>42<suffix>`); `false` shows only the name.
        let label = if show_values {
            format!("{id}\n{prefix}{}{suffix}", fnum(through(id)))
        } else {
            id.clone()
        };
        svg.text(
            label_x,
            y + h / 2.0 + 4.0,
            &format!("text-anchor=\"{anchor}\" fill=\"{fg}\" font-size=\"12\""),
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

/// Column alignment mode (`config.sankey.nodeAlignment`), mirroring d3-sankey.
enum Alignment {
    /// Sink nodes pushed to the last column, others by depth (upstream default).
    Justify,
    /// Source-less nodes nudged toward their earliest target.
    Center,
    /// Every node at its longest-path depth from a source.
    Left,
    /// Every node at its longest-path distance from a sink.
    Right,
}

impl Alignment {
    fn parse(s: Option<&str>) -> Self {
        match s.map(str::trim) {
            Some("left") => Alignment::Left,
            Some("right") => Alignment::Right,
            Some("center") => Alignment::Center,
            _ => Alignment::Justify,
        }
    }
}

/// Assign a column index to every node under the chosen [`Alignment`].
fn assign_columns(
    order: &[String],
    links: &[crate::parse::SankeyLink],
    align: Alignment,
) -> BTreeMap<String, u32> {
    let depth = column_depths(order, links);
    let height = column_heights(order, links);
    let ncols = depth.values().max().map_or(1, |m| m + 1);
    let has_out: BTreeSet<&str> = links.iter().map(|l| l.source.as_str()).collect();
    let has_in: BTreeSet<&str> = links.iter().map(|l| l.target.as_str()).collect();

    order
        .iter()
        .map(|n| {
            let c = match align {
                Alignment::Left => depth[n],
                Alignment::Right => (ncols - 1).saturating_sub(height[n]),
                Alignment::Justify => {
                    if has_out.contains(n.as_str()) {
                        depth[n]
                    } else {
                        ncols - 1
                    }
                }
                Alignment::Center => {
                    if has_in.contains(n.as_str()) {
                        depth[n]
                    } else if has_out.contains(n.as_str()) {
                        links
                            .iter()
                            .filter(|l| &l.source == n)
                            .map(|l| depth[&l.target])
                            .min()
                            .unwrap_or(0)
                            .saturating_sub(1)
                    } else {
                        0
                    }
                }
            };
            (n.clone(), c)
        })
        .collect()
}

/// Longest-path depth from a source node (column when left-aligned).
fn column_depths(order: &[String], links: &[crate::parse::SankeyLink]) -> BTreeMap<String, u32> {
    let mut depth: BTreeMap<String, u32> = order.iter().map(|n| (n.clone(), 0)).collect();
    // Iteratively relax until stable.
    let mut changed = true;
    let mut iter = 0;
    while changed && iter < 200 {
        changed = false;
        iter += 1;
        for l in links {
            // `order` is expected to cover every endpoint, but a link naming an
            // undeclared node would otherwise panic on the map index — treat a
            // missing endpoint as column 0 rather than crashing.
            let s = depth.get(&l.source).copied().unwrap_or(0);
            let t = depth.get(&l.target).copied().unwrap_or(0);
            if t <= s {
                depth.insert(l.target.clone(), s + 1);
                changed = true;
            }
        }
    }
    depth
}

/// Longest-path distance to a sink node (used by right/center alignment).
fn column_heights(order: &[String], links: &[crate::parse::SankeyLink]) -> BTreeMap<String, u32> {
    let mut height: BTreeMap<String, u32> = order.iter().map(|n| (n.clone(), 0)).collect();
    let mut changed = true;
    let mut iter = 0;
    while changed && iter < 200 {
        changed = false;
        iter += 1;
        for l in links {
            let s = height.get(&l.source).copied().unwrap_or(0);
            let t = height.get(&l.target).copied().unwrap_or(0);
            if s <= t {
                height.insert(l.source.clone(), t + 1);
                changed = true;
            }
        }
    }
    height
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::SankeyLink;

    #[test]
    fn produces_svg() {
        let d = SankeyDiagram {
            links: vec![
                SankeyLink {
                    source: "A".into(),
                    target: "B".into(),
                    value: 5.0,
                },
                SankeyLink {
                    source: "B".into(),
                    target: "C".into(),
                    value: 3.0,
                },
            ],
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">A<"));
        assert!(svg.contains(">B<"));
        assert!(svg.contains(">C<"));
    }

    fn chain() -> SankeyDiagram {
        SankeyDiagram {
            links: vec![
                SankeyLink {
                    source: "A".into(),
                    target: "B".into(),
                    value: 5.0,
                },
                SankeyLink {
                    source: "B".into(),
                    target: "C".into(),
                    value: 3.0,
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn nodes_get_distinct_palette_colors() {
        let theme = Theme::default();
        let svg = render(&chain(), &theme);
        // Each node rect carries its own palette color, not one flat fill.
        for i in 0..3 {
            assert!(svg.contains(&format!(
                "fill=\"{}\" stroke=\"#fff\"",
                theme.cscale_color(i)
            )));
        }
    }

    #[test]
    fn default_link_color_is_gradient() {
        let theme = Theme::default();
        let svg = render(&chain(), &theme);
        // Upstream default is `gradient`: each link is a per-link gradient.
        assert!(svg.contains("<linearGradient id=\"sankey-grad-0\""));
        assert!(svg.contains("stroke=\"url(#sankey-grad-0)\""));
    }

    #[test]
    fn link_color_source_tints_from_source_node() {
        let mut d = chain();
        d.link_color = Some("source".into());
        let theme = Theme::default();
        let svg = render(&d, &theme);
        // A→B tinted from A (node 0), B→C tinted from B (node 1).
        assert!(svg.contains(&format!(
            "stroke=\"{}\" stroke-opacity",
            theme.cscale_color(0)
        )));
        assert!(svg.contains(&format!(
            "stroke=\"{}\" stroke-opacity",
            theme.cscale_color(1)
        )));
    }

    #[test]
    fn show_values_false_omits_value() {
        let mut d = chain();
        d.show_values = Some(false);
        let svg = render(&d, &Theme::default());
        // Node label carries only the name, no throughput second line.
        assert!(svg.contains(">A<"));
        assert!(!svg.contains(">5<"));
    }

    #[test]
    fn prefix_and_suffix_wrap_value() {
        let mut d = chain();
        d.prefix = Some("$".into());
        d.suffix = Some(" USD".into());
        let svg = render(&d, &Theme::default());
        assert!(svg.contains(">$5 USD<"));
    }

    #[test]
    fn node_width_config_sets_rect_width() {
        let mut d = chain();
        d.node_width = Some(24.0);
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("width=\"24\""));
    }

    #[test]
    fn link_color_target() {
        let mut d = chain();
        d.link_color = Some("target".into());
        let theme = Theme::default();
        let svg = render(&d, &theme);
        // A→B tinted from B (node 1), B→C tinted from C (node 2).
        assert!(svg.contains(&format!(
            "stroke=\"{}\" stroke-opacity",
            theme.cscale_color(1)
        )));
        assert!(svg.contains(&format!(
            "stroke=\"{}\" stroke-opacity",
            theme.cscale_color(2)
        )));
    }

    #[test]
    fn link_color_gradient_emits_defs() {
        let mut d = chain();
        d.link_color = Some("gradient".into());
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("<linearGradient id=\"sankey-grad-0\""));
        assert!(svg.contains("stroke=\"url(#sankey-grad-0)\""));
    }

    #[test]
    fn link_color_hex_literal() {
        let mut d = chain();
        d.link_color = Some("#123456".into());
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("stroke=\"#123456\" stroke-opacity"));
    }

    #[test]
    fn node_alignment_right_pushes_sources_rightward() {
        // Two independent sinks at different left-depths: `justify` and `right`
        // place the terminal nodes in the last column; `left`/`right` differ on
        // where the sources land.
        let d = SankeyDiagram {
            links: vec![
                SankeyLink {
                    source: "A".into(),
                    target: "X".into(),
                    value: 1.0,
                },
                SankeyLink {
                    source: "B".into(),
                    target: "C".into(),
                    value: 1.0,
                },
                SankeyLink {
                    source: "C".into(),
                    target: "X".into(),
                    value: 1.0,
                },
            ],
            ..Default::default()
        };
        let left = assign_columns(&nodes(&d), &d.links, Alignment::Left);
        let right = assign_columns(&nodes(&d), &d.links, Alignment::Right);
        // Left: A at 0. Right: A pulled to just before X (its only path is len 1).
        assert_eq!(left["A"], 0);
        assert_eq!(right["A"], 1);
        // X is a sink; it sits in the last column under both.
        assert_eq!(left["X"], 2);
        assert_eq!(right["X"], 2);
    }

    #[test]
    fn node_alignment_justify_pushes_sinks_to_last_column() {
        let d = SankeyDiagram {
            links: vec![
                SankeyLink {
                    source: "A".into(),
                    target: "B".into(),
                    value: 1.0,
                },
                SankeyLink {
                    source: "A".into(),
                    target: "C".into(),
                    value: 1.0,
                },
                SankeyLink {
                    source: "C".into(),
                    target: "D".into(),
                    value: 1.0,
                },
            ],
            ..Default::default()
        };
        let justify = assign_columns(&nodes(&d), &d.links, Alignment::Justify);
        let left = assign_columns(&nodes(&d), &d.links, Alignment::Left);
        // B is a sink: left keeps it at depth 1, justify pushes it to the last
        // column (2, alongside D).
        assert_eq!(left["B"], 1);
        assert_eq!(justify["B"], 2);
        assert_eq!(justify["D"], 2);
    }

    fn nodes(d: &SankeyDiagram) -> Vec<String> {
        let mut order = Vec::new();
        let mut seen = BTreeSet::new();
        for l in &d.links {
            for n in [&l.source, &l.target] {
                if seen.insert(n.clone()) {
                    order.push(n.clone());
                }
            }
        }
        order
    }
}
