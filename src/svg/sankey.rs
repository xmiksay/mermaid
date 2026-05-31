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
use super::theme::Theme;

const PAD: f64 = 30.0;
const NODE_W: f64 = 18.0;
const COL_GAP: f64 = 180.0;
const CHART_H: f64 = 380.0;
const ROW_GAP: f64 = 6.0;

pub(crate) fn render(d: &SankeyDiagram, theme: &Theme) -> String {
    let fg = theme.fg;

    if d.links.is_empty() {
        let mut svg = SvgBuilder::new(200.0, 80.0).font(theme.font_family, theme.font_size);
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

    // Compute column (depth) per node via longest path from a root.
    let depth = column_depths(&order, &d.links);
    let max_depth = *depth.values().max().unwrap_or(&0);
    let cols: usize = (max_depth + 1) as usize;

    // Group nodes by column.
    let mut by_col: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for n in &order {
        by_col.entry(depth[n]).or_default().push(n.clone());
    }

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
    let scale = (CHART_H
        - (by_col
            .values()
            .map(|v| v.len())
            .max()
            .unwrap_or(1)
            .saturating_sub(1) as f64)
            * ROW_GAP)
        / col_max;

    // Position rectangles: x by column, y stacked.
    let mut rects: BTreeMap<String, (f64, f64, f64)> = BTreeMap::new(); // id -> (x, y, h)
    for (c, ns) in &by_col {
        let x = PAD + *c as f64 * COL_GAP;
        let mut y = PAD + 10.0;
        for n in ns {
            let h = (through(n) * scale).max(2.0);
            rects.insert(n.clone(), (x, y, h));
            y += h + ROW_GAP;
        }
    }

    let width = PAD * 2.0 + (cols.saturating_sub(1) as f64) * COL_GAP + NODE_W + 120.0;
    let height = PAD * 2.0 + CHART_H + 30.0;
    let mut svg = SvgBuilder::new(width, height).font(theme.font_family, theme.font_size);

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
        let x1 = sx + NODE_W;
        let x2 = tx;
        let mx = (x1 + x2) / 2.0;
        let color = theme.pie_color(li);
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
                "fill=\"none\" stroke=\"{color}\" stroke-opacity=\"0.45\" stroke-width=\"{}\"",
                fnum(sw)
            ),
        );
    }

    // Draw node rects + labels.
    for (id, (x, y, h)) in &rects {
        svg.rect(
            *x,
            *y,
            NODE_W,
            *h,
            &format!("fill=\"{}\" stroke=\"#fff\"", theme.flow_node_stroke),
        );
        let label_x = if depth[id] == max_depth {
            *x - 6.0
        } else {
            *x + NODE_W + 6.0
        };
        let anchor = if depth[id] == max_depth {
            "end"
        } else {
            "start"
        };
        svg.text(
            label_x,
            y + h / 2.0 + 4.0,
            &format!("text-anchor=\"{anchor}\" fill=\"{fg}\" font-size=\"12\""),
            id,
        );
    }

    svg.finish()
}

fn column_depths(order: &[String], links: &[crate::parse::SankeyLink]) -> BTreeMap<String, u32> {
    let mut depth: BTreeMap<String, u32> = order.iter().map(|n| (n.clone(), 0)).collect();
    // Iteratively relax until stable.
    let mut changed = true;
    let mut iter = 0;
    while changed && iter < 200 {
        changed = false;
        iter += 1;
        for l in links {
            let s = depth[&l.source];
            let t = depth[&l.target];
            if t <= s {
                depth.insert(l.target.clone(), s + 1);
                changed = true;
            }
        }
    }
    depth
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
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">A<"));
        assert!(svg.contains(">B<"));
        assert!(svg.contains(">C<"));
    }
}
