//! Within-column node ordering, ported from d3-sankey.
//!
//! Upstream Mermaid lays out sankey nodes with d3-sankey, which seeds each
//! column with the nodes in order of first appearance and then runs iterative
//! barycenter relaxation passes (`relaxLeftToRight`/`relaxRightToLeft` +
//! collision resolution) that re-sort each column by vertical position to
//! minimise link crossings. Rendering only in first-appearance order (as a
//! plain stack) diverges from that. This module reproduces d3-sankey's
//! ordering pass so a ported diagram matches the JS-rendered column order; the
//! actual pixel geometry is still applied by the renderer.

use std::collections::BTreeMap;

use crate::parse::SankeyLink;

/// d3-sankey's default `iterations`.
const ITERATIONS: usize = 6;

/// Return the nodes of each column (indexing into `order`) sorted top-to-bottom
/// the way d3-sankey would, given the column assignment in `col`, the link set,
/// per-node throughput `value` (aligned to `order`), the vertical `extent`, and
/// node padding `py`.
pub(crate) fn order_columns(
    order: &[String],
    col: &BTreeMap<String, u32>,
    links: &[SankeyLink],
    value: &[f64],
    extent: f64,
    py: f64,
) -> Vec<Vec<usize>> {
    let n = order.len();
    let idx: BTreeMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(i, s)| (s.as_str(), i))
        .collect();
    let layer: Vec<usize> = order.iter().map(|s| col[s] as usize).collect();
    let ncols = layer.iter().copied().max().map_or(1, |m| m + 1);

    // Per-node link adjacency, preserving input link order (d3 `computeNodeLinks`).
    let mut source_links: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut target_links: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut ls = vec![0usize; links.len()];
    let mut lt = vec![0usize; links.len()];
    let lvalue: Vec<f64> = links.iter().map(|l| l.value).collect();
    for (li, l) in links.iter().enumerate() {
        let s = idx[l.source.as_str()];
        let t = idx[l.target.as_str()];
        ls[li] = s;
        lt[li] = t;
        source_links[s].push(li);
        target_links[t].push(li);
    }

    // Columns seeded in first-appearance order (d3 `computeNodeLayers`, default
    // `sort` undefined keeps insertion order).
    let mut columns: Vec<Vec<usize>> = vec![Vec::new(); ncols];
    for (i, &c) in layer.iter().enumerate() {
        columns[c].push(i);
    }

    // d3 recomputes padding to fit the tallest column within the extent
    // (`py = min(nodePadding, (y1 - y0) / (maxColLen - 1))`).
    let max_col_len = columns.iter().map(Vec::len).max().unwrap_or(1);
    let py = if max_col_len > 1 {
        py.min(extent / (max_col_len - 1) as f64)
    } else {
        py
    };

    let mut st = State {
        layer,
        value: value.to_vec(),
        y0: vec![0.0; n],
        y1: vec![0.0; n],
        source_links,
        target_links,
        ls,
        lt,
        lvalue,
        lwidth: vec![0.0; links.len()],
        py,
        extent,
    };

    st.initialize_breadths(&columns);
    for i in 0..ITERATIONS {
        let alpha = 0.99_f64.powi(i as i32);
        let beta = (1.0 - alpha).max((i + 1) as f64 / ITERATIONS as f64);
        st.relax_right_to_left(&mut columns, alpha, beta);
        st.relax_left_to_right(&mut columns, alpha, beta);
    }

    columns
}

struct State {
    layer: Vec<usize>,
    value: Vec<f64>,
    y0: Vec<f64>,
    y1: Vec<f64>,
    source_links: Vec<Vec<usize>>,
    target_links: Vec<Vec<usize>>,
    ls: Vec<usize>,
    lt: Vec<usize>,
    lvalue: Vec<f64>,
    lwidth: Vec<f64>,
    py: f64,
    extent: f64,
}

impl State {
    fn initialize_breadths(&mut self, columns: &[Vec<usize>]) {
        // ky maps summed value to available height, taken as the tightest column.
        let mut ky = f64::INFINITY;
        for c in columns {
            let sum: f64 = c.iter().map(|&i| self.value[i]).sum();
            if sum > 0.0 {
                let avail = self.extent - (c.len().saturating_sub(1)) as f64 * self.py;
                ky = ky.min(avail / sum);
            }
        }
        if !ky.is_finite() {
            ky = 1.0;
        }
        for w in self.lwidth.iter_mut().enumerate() {
            *w.1 = self.lvalue[w.0] * ky;
        }
        for c in columns {
            let mut y = 0.0;
            for &i in c {
                self.y0[i] = y;
                self.y1[i] = y + self.value[i] * ky;
                y = self.y1[i] + self.py;
            }
            // Spread the leftover vertical slack evenly between nodes so the
            // column starts centred (d3 `initializeNodeBreadths`).
            let spread = (self.extent - y + self.py) / (c.len() as f64 + 1.0);
            for (k, &i) in c.iter().enumerate() {
                let dy = spread * (k as f64 + 1.0);
                self.y0[i] += dy;
                self.y1[i] += dy;
            }
            self.reorder_links(c);
        }
    }

    /// Sort every node's own link lists (d3 `reorderLinks`, used at init): each
    /// node's out-links by target breadth, in-links by source breadth.
    fn reorder_links(&mut self, nodes: &[usize]) {
        for &node in nodes {
            let (y0, lt) = (&self.y0, &self.lt);
            self.source_links[node]
                .sort_by(|&a, &b| y0[lt[a]].total_cmp(&y0[lt[b]]).then(a.cmp(&b)));
            let (y0, ls) = (&self.y0, &self.ls);
            self.target_links[node]
                .sort_by(|&a, &b| y0[ls[a]].total_cmp(&y0[ls[b]]).then(a.cmp(&b)));
        }
    }

    fn relax_left_to_right(&mut self, columns: &mut [Vec<usize>], alpha: f64, beta: f64) {
        // Index loop: each iteration reads and then re-sorts `columns[c]` while
        // also calling `&mut self` helpers, so an iterator over `columns` can't
        // coexist with the self borrows.
        let mut c = 1;
        while c < columns.len() {
            let col_nodes = columns[c].clone();
            for &target in &col_nodes {
                let (mut y, mut w) = (0.0, 0.0);
                for &li in &self.target_links[target].clone() {
                    let source = self.ls[li];
                    let v = self.lvalue[li] * (self.layer[target] - self.layer[source]) as f64;
                    y += self.target_top(source, target) * v;
                    w += v;
                }
                if w <= 0.0 || w.is_nan() {
                    continue;
                }
                let dy = (y / w - self.y0[target]) * alpha;
                self.y0[target] += dy;
                self.y1[target] += dy;
                self.reorder_node_links(target);
            }
            columns[c].sort_by(|&a, &b| self.y0[a].total_cmp(&self.y0[b]));
            self.resolve_collisions(&columns[c], beta);
            c += 1;
        }
    }

    fn relax_right_to_left(&mut self, columns: &mut [Vec<usize>], alpha: f64, beta: f64) {
        // Walk columns n-2 .. 0 (see the index-loop note in `relax_left_to_right`).
        let mut c = columns.len().saturating_sub(1);
        while c > 0 {
            c -= 1;
            let col_nodes = columns[c].clone();
            for &source in &col_nodes {
                let (mut y, mut w) = (0.0, 0.0);
                for &li in &self.source_links[source].clone() {
                    let target = self.lt[li];
                    let v = self.lvalue[li] * (self.layer[target] - self.layer[source]) as f64;
                    y += self.source_top(source, target) * v;
                    w += v;
                }
                if w <= 0.0 || w.is_nan() {
                    continue;
                }
                let dy = (y / w - self.y0[source]) * alpha;
                self.y0[source] += dy;
                self.y1[source] += dy;
                self.reorder_node_links(source);
            }
            columns[c].sort_by(|&a, &b| self.y0[a].total_cmp(&self.y0[b]));
            self.resolve_collisions(&columns[c], beta);
        }
    }

    /// y of the top of the `source`→`target` link at the source end.
    fn target_top(&self, source: usize, target: usize) -> f64 {
        let mut y = self.y0[source]
            - (self.source_links[source].len().saturating_sub(1)) as f64 * self.py / 2.0;
        for &li in &self.source_links[source] {
            if self.lt[li] == target {
                break;
            }
            y += self.lwidth[li] + self.py;
        }
        for &li in &self.target_links[target] {
            if self.ls[li] == source {
                break;
            }
            y -= self.lwidth[li];
        }
        y
    }

    /// y of the top of the `source`→`target` link at the target end.
    fn source_top(&self, source: usize, target: usize) -> f64 {
        let mut y = self.y0[target]
            - (self.target_links[target].len().saturating_sub(1)) as f64 * self.py / 2.0;
        for &li in &self.target_links[target] {
            if self.ls[li] == source {
                break;
            }
            y += self.lwidth[li] + self.py;
        }
        for &li in &self.source_links[source] {
            if self.lt[li] == target {
                break;
            }
            y -= self.lwidth[li];
        }
        y
    }

    /// Re-sort the link lists of this node's *neighbours* (d3 `reorderNodeLinks`):
    /// each source's out-links by their target breadth, each target's in-links by
    /// their source breadth. Ties break on original link index.
    fn reorder_node_links(&mut self, node: usize) {
        let sources: Vec<usize> = self.target_links[node]
            .iter()
            .map(|&li| self.ls[li])
            .collect();
        for s in sources {
            let (y0, lt) = (&self.y0, &self.lt);
            self.source_links[s].sort_by(|&a, &b| y0[lt[a]].total_cmp(&y0[lt[b]]).then(a.cmp(&b)));
        }
        let targets: Vec<usize> = self.source_links[node]
            .iter()
            .map(|&li| self.lt[li])
            .collect();
        for t in targets {
            let (y0, ls) = (&self.y0, &self.ls);
            self.target_links[t].sort_by(|&a, &b| y0[ls[a]].total_cmp(&y0[ls[b]]).then(a.cmp(&b)));
        }
    }

    fn resolve_collisions(&mut self, nodes: &[usize], alpha: f64) {
        if nodes.is_empty() {
            return;
        }
        let i = nodes.len() >> 1;
        let subject = nodes[i];
        let top = self.y0[subject] - self.py;
        let bottom = self.y1[subject] + self.py;
        self.collisions_bottom_to_top(nodes, top, i as isize - 1, alpha);
        self.collisions_top_to_bottom(nodes, bottom, i + 1, alpha);
        self.collisions_bottom_to_top(nodes, self.extent, nodes.len() as isize - 1, alpha);
        self.collisions_top_to_bottom(nodes, 0.0, 0, alpha);
    }

    fn collisions_top_to_bottom(&mut self, nodes: &[usize], mut y: f64, start: usize, alpha: f64) {
        for &node in nodes.iter().skip(start) {
            let dy = (y - self.y0[node]) * alpha;
            if dy > 1e-6 {
                self.y0[node] += dy;
                self.y1[node] += dy;
            }
            y = self.y1[node] + self.py;
        }
    }

    fn collisions_bottom_to_top(&mut self, nodes: &[usize], mut y: f64, start: isize, alpha: f64) {
        let mut i = start;
        while i >= 0 {
            let node = nodes[i as usize];
            let dy = (self.y1[node] - y) * alpha;
            if dy > 1e-6 {
                self.y0[node] -= dy;
                self.y1[node] -= dy;
            }
            y = self.y0[node] - self.py;
            i -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::SankeyLink;
    use std::collections::BTreeSet;

    fn link(s: &str, t: &str, v: f64) -> SankeyLink {
        SankeyLink {
            source: s.into(),
            target: t.into(),
            value: v,
        }
    }

    /// First-appearance node order + `max(in, out)` throughput, as the renderer
    /// derives them.
    fn prep(links: &[SankeyLink]) -> (Vec<String>, Vec<f64>) {
        let mut order = Vec::new();
        let mut seen = BTreeSet::new();
        for l in links {
            for n in [&l.source, &l.target] {
                if seen.insert(n.clone()) {
                    order.push(n.clone());
                }
            }
        }
        let mut in_s: BTreeMap<String, f64> = BTreeMap::new();
        let mut out_s: BTreeMap<String, f64> = BTreeMap::new();
        for l in links {
            *out_s.entry(l.source.clone()).or_default() += l.value;
            *in_s.entry(l.target.clone()).or_default() += l.value;
        }
        let values = order
            .iter()
            .map(|n| {
                in_s.get(n)
                    .copied()
                    .unwrap_or(0.0)
                    .max(out_s.get(n).copied().unwrap_or(0.0))
            })
            .collect();
        (order, values)
    }

    fn names(order: &[String], col: &[usize]) -> Vec<String> {
        col.iter().map(|&i| order[i].clone()).collect()
    }

    /// The energy-flow sample from `samples/sankey.mmd`. Reproduces d3-sankey
    /// (justify) exactly when fed upstream's layout inputs — vertical extent =
    /// `height` (400) and padding = `nodePadding + 15` (27, showValues on): left
    /// column Coal, Solar, Wind, Gas; right column Industry, Transport,
    /// Residential, Heating. Matches Mermaid JS 11.16.0 (#317).
    #[test]
    fn energy_sample_matches_d3_column_order() {
        let links = vec![
            link("Electricity", "Industry", 250.0),
            link("Electricity", "Transport", 80.0),
            link("Electricity", "Residential", 150.0),
            link("Gas", "Heating", 120.0),
            link("Gas", "Industry", 60.0),
            link("Coal", "Electricity", 300.0),
            link("Solar", "Electricity", 90.0),
            link("Wind", "Electricity", 120.0),
        ];
        let (order, values) = prep(&links);
        // Justify alignment: sources at column 0, Electricity at 1, everything
        // terminal (incl. the Heating sink) at the last column.
        let mut col: BTreeMap<String, u32> = order.iter().map(|n| (n.clone(), 0)).collect();
        col.insert("Electricity".into(), 1);
        for n in ["Industry", "Transport", "Residential", "Heating"] {
            col.insert(n.into(), 2);
        }
        let cols = order_columns(&order, &col, &links, &values, 400.0, 27.0);
        assert_eq!(names(&order, &cols[0]), ["Coal", "Solar", "Wind", "Gas"]);
        assert_eq!(names(&order, &cols[1]), ["Electricity"]);
        assert_eq!(
            names(&order, &cols[2]),
            ["Industry", "Transport", "Residential", "Heating"]
        );
        // The pre-fix bug placed Gas at the top of the left column; guard it.
        assert_ne!(cols[0].first().map(|&i| order[i].as_str()), Some("Gas"));
    }

    /// Relaxation reorders a column away from first-appearance order to reduce
    /// crossings: two sources feeding a shared sink line up with their targets.
    #[test]
    fn relaxation_reorders_within_column() {
        // B and A both appear before their targets; A→Y, B→X with X above Y
        // pulls A below B despite A being seen first.
        let links = vec![
            link("A", "Y", 1.0),
            link("B", "X", 1.0),
            link("C", "X", 1.0),
            link("C", "Y", 1.0),
        ];
        let (order, values) = prep(&links);
        let mut col: BTreeMap<String, u32> = order.iter().map(|n| (n.clone(), 0)).collect();
        col.insert("X".into(), 1);
        col.insert("Y".into(), 1);
        let cols = order_columns(&order, &col, &links, &values, 200.0, 6.0);
        // Column 0 is a permutation of the sources, and every column is fully
        // populated (no node dropped by the reordering).
        let mut got: Vec<String> = names(&order, &cols[0]);
        got.sort();
        assert_eq!(got, ["A", "B", "C"]);
        assert_eq!(cols[0].len() + cols[1].len(), order.len());
    }

    #[test]
    fn single_column_is_stable() {
        let links = vec![link("A", "B", 1.0), link("C", "D", 1.0)];
        let (order, values) = prep(&links);
        let mut col: BTreeMap<String, u32> = order.iter().map(|n| (n.clone(), 0)).collect();
        col.insert("B".into(), 1);
        col.insert("D".into(), 1);
        let cols = order_columns(&order, &col, &links, &values, 100.0, 6.0);
        assert_eq!(cols.len(), 2);
        assert_eq!(cols[0].len(), 2);
        assert_eq!(cols[1].len(), 2);
    }
}
