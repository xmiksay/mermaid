//! Column assignment for the sankey layout (`config.sankey.nodeAlignment`).

use std::collections::{BTreeMap, BTreeSet};

/// Column alignment mode (`config.sankey.nodeAlignment`), mirroring d3-sankey.
pub(super) enum Alignment {
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
    pub(super) fn parse(s: Option<&str>) -> Self {
        match s.map(str::trim) {
            Some("left") => Alignment::Left,
            Some("right") => Alignment::Right,
            Some("center") => Alignment::Center,
            _ => Alignment::Justify,
        }
    }
}

/// Assign a column index to every node under the chosen [`Alignment`].
pub(super) fn assign_columns(
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
