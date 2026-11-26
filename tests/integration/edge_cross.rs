//! Flowchart edges vs nodes and labels (#402): no edge runs through a node it
//! did not run through before #402 — compared per edge, so a swap still
//! fails — and no more label collisions than before (`baseline-crossings.txt`
//! and `baseline-counts.txt`, rewritten by `EDGE_BASELINE_WRITE=1`). The few
//! accepted exceptions are listed with their reason in
//! `crossing-waivers.txt`.

use mermaid_svg::render;

use crate::edge_canvas::baseline;
use crate::edge_ends::graph_sources;
use crate::svg_read::{drawn, labels, node_at, nodes, Kind, Node, Pt};

/// `edge:node` pairs (drawing order) where an edge runs through a node it
/// does not end on; `-` for none.
fn crossings(svg: &str) -> String {
    let ns = nodes(svg);
    let pairs: Vec<String> = drawn(svg)
        .iter()
        .enumerate()
        .flat_map(|(ei, d)| {
            let ends = [node_at(&ns, d.raw[0]), node_at(&ns, d.raw[d.raw.len() - 1])];
            ns.iter()
                .enumerate()
                .filter(|&(i, n)| !ends.contains(&Some(i)) && d.curve.iter().any(|&p| inside(n, p)))
                .map(|(i, _)| format!("{ei}:{i}"))
                .collect::<Vec<_>>()
        })
        .collect();
    if pairs.is_empty() {
        "-".into()
    } else {
        pairs.join(",")
    }
}

/// Strictly inside the node's outline, 1px in from it.
fn inside(n: &Node, p: Pt) -> bool {
    let (c, hw, hh) = (
        n.center(),
        (n.b.2 - n.b.0) / 2.0 - 1.0,
        (n.b.3 - n.b.1) / 2.0 - 1.0,
    );
    let (dx, dy) = ((p.0 - c.0).abs(), (p.1 - c.1).abs());
    if hw <= 0.0 || hh <= 0.0 {
        return false;
    }
    match n.kind {
        Kind::Rect => dx < hw && dy < hh,
        Kind::Circle => dx.hypot(dy) < hw,
        Kind::Rhombus => dx / hw + dy / hh < 1.0,
    }
}

#[test]
fn flowchart_edges_cross_no_new_nodes() {
    let current: Vec<(String, String)> = graph_sources(&["flowchart"])
        .into_iter()
        .map(|(name, src)| {
            let svg = render(&src).unwrap_or_else(|e| panic!("{name}: {e}"));
            (name, crossings(&svg))
        })
        .collect();
    let base = baseline("baseline-crossings.txt", &current);
    let path = format!(
        "{}/tests/fixtures/edges/crossing-waivers.txt",
        env!("CARGO_MANIFEST_DIR")
    );
    let waivers = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let waived = |name: &str, pair: &str| {
        waivers
            .lines()
            .any(|l| l.split_whitespace().take(2).eq([name, pair]))
    };
    let mut bad = Vec::new();
    for (name, now) in &current {
        let was: Vec<&str> = base.get(name).map_or(vec![], |w| w.split(',').collect());
        for pair in now.split(',').filter(|p| *p != "-") {
            if !was.contains(&pair) && !waived(name, pair) {
                bad.push(format!(
                    "{name}: edge:node {pair} (before #402: {})",
                    was.join(",")
                ));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "new edge-through-node crossings:\n{}",
        bad.join("\n")
    );
}

fn overlap(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    a.0.max(b.0) + 1.0 < a.2.min(b.2) && a.1.max(b.1) + 1.0 < a.3.min(b.3)
}

/// Label pairs that overlap each other, plus labels that overlap a node.
fn label_collisions(svg: &str) -> usize {
    let ls = labels(svg);
    let ns = nodes(svg);
    let pairs = (0..ls.len())
        .flat_map(|i| (i + 1..ls.len()).map(move |j| (i, j)))
        .filter(|&(i, j)| overlap(ls[i].0, ls[j].0))
        .count();
    let on_nodes = ls
        .iter()
        .map(|l| ns.iter().filter(|n| overlap(l.0, n.b)).count())
        .sum::<usize>();
    pairs + on_nodes
}

#[test]
fn flowchart_samples_add_no_label_collisions() {
    let current: Vec<(String, String)> = crate::SAMPLES
        .iter()
        .filter(|(_, stem, _)| stem.starts_with("flowchart/"))
        .map(|(_, stem, src)| {
            let svg = render(src).expect("renders");
            (stem.to_string(), label_collisions(&svg).to_string())
        })
        .collect();
    let base = baseline("baseline-counts.txt", &current);
    let mut bad = Vec::new();
    for (stem, now) in &current {
        let was = base
            .get(stem)
            .unwrap_or_else(|| panic!("{stem}: no baseline"));
        let (n, w): (usize, usize) = (now.parse().expect("count"), was.parse().expect("count"));
        if n > w {
            bad.push(format!("{stem}: label collisions {n} (was {w})"));
        }
    }
    assert!(bad.is_empty(), "new label collisions:\n{}", bad.join("\n"));
}
