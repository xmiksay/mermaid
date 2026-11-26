//! Edge canvas sweeps (#402): no edge leaves the viewBox, the non-flowchart
//! drawing is unchanged apart from that shift, flowchart edges add no node
//! crossings or label collisions, and output is deterministic.
//!
//! The baselines under `tests/fixtures/edges/` were captured from the
//! renderer before #402; `EDGE_BASELINE_WRITE=1` rewrites them from the
//! current output.

use std::collections::HashMap;

use mermaid_svg::render;

use crate::edge_ends::graph_sources;
use crate::svg_read::{drawn, labels, node_at, nodes, view_box, Node, Pt};

const GRAPH: [&str; 5] = ["flowchart", "state", "class", "er", "requirement"];

fn baseline_path(file: &str) -> String {
    format!("{}/tests/fixtures/edges/{file}", env!("CARGO_MANIFEST_DIR"))
}

/// Compares `current` (`name<TAB>value` lines) with the stored baseline, or
/// rewrites the baseline when `EDGE_BASELINE_WRITE` is set.
pub(crate) fn baseline(file: &str, current: &[(String, String)]) -> HashMap<String, String> {
    let path = baseline_path(file);
    if std::env::var_os("EDGE_BASELINE_WRITE").is_some() {
        let text: String = current.iter().map(|(k, v)| format!("{k}\t{v}\n")).collect();
        std::fs::write(&path, text).unwrap_or_else(|e| panic!("{path}: {e}"));
    }
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{path}: {e}"))
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn no_edge_leaves_the_view_box() {
    let mut bad = Vec::new();
    for (name, src) in graph_sources(&GRAPH) {
        let svg = render(&src).unwrap_or_else(|e| panic!("{name}: {e}"));
        let (w, h) = view_box(&svg);
        let out = drawn(&svg)
            .iter()
            .flat_map(|d| d.curve.iter())
            .map(|&(x, y)| (-x).max(-y).max(x - w).max(y - h))
            .fold(0.0, f64::max);
        if out > 0.5 {
            bad.push(format!("{name}: {out:.1}px past the viewBox"));
        }
    }
    assert!(bad.is_empty(), "edges off the canvas:\n{}", bad.join("\n"));
}

/// Every edge path's points, as one line of `x,y` pairs per path.
fn edge_points(svg: &str) -> String {
    drawn(svg)
        .iter()
        .map(|d| {
            let pts: Vec<String> = d.raw.iter().map(|p| format!("{},{}", p.0, p.1)).collect();
            pts.join(" ")
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn parse_points(s: &str) -> Vec<Vec<Pt>> {
    s.split(" | ")
        .map(|path| {
            path.split(' ')
                .filter_map(|p| {
                    let (x, y) = p.split_once(',')?;
                    Some((x.parse().ok()?, y.parse().ok()?))
                })
                .collect()
        })
        .collect()
}

/// Class, ER, requirement and state samples draw the same edges as before
/// #402, up to one translation of the whole drawing (the viewBox shift).
#[test]
fn non_flowchart_samples_match_baseline_up_to_shift() {
    let current: Vec<(String, String)> = crate::SAMPLES
        .iter()
        .filter(|(_, stem, _)| GRAPH[1..].contains(&stem.split('/').next().unwrap_or("")))
        .map(|(_, stem, src)| {
            (
                stem.to_string(),
                edge_points(&render(src).expect("renders")),
            )
        })
        .collect();
    let base = baseline("baseline-paths.txt", &current);
    let mut bad = Vec::new();
    for (stem, now) in &current {
        let was = base
            .get(stem)
            .unwrap_or_else(|| panic!("{stem}: no baseline"));
        let (now, was) = (parse_points(now), parse_points(was));
        let shift = match (
            now.first().and_then(|p| p.first()),
            was.first().and_then(|p| p.first()),
        ) {
            (Some(a), Some(b)) => (a.0 - b.0, a.1 - b.1),
            _ => (0.0, 0.0),
        };
        let same = now.len() == was.len()
            && now.iter().zip(&was).all(|(a, b)| {
                a.len() == b.len()
                    && a.iter().zip(b).all(|(p, q)| {
                        (p.0 - q.0 - shift.0).abs() < 0.01 && (p.1 - q.1 - shift.1).abs() < 0.01
                    })
            });
        if !same {
            bad.push(stem.clone());
        }
    }
    assert!(
        bad.is_empty(),
        "edge geometry changed beyond a shift: {bad:?}"
    );
}

/// `(edge, node)` pairs where an edge runs through a node it does not end on.
fn crossings(svg: &str) -> usize {
    let ns = nodes(svg);
    drawn(svg)
        .iter()
        .map(|d| {
            let ends = [node_at(&ns, d.raw[0]), node_at(&ns, d.raw[d.raw.len() - 1])];
            ns.iter()
                .enumerate()
                .filter(|(i, n)| !ends.contains(&Some(*i)) && d.curve.iter().any(|&p| inside(n, p)))
                .count()
        })
        .sum()
}

fn inside(n: &Node, p: Pt) -> bool {
    p.0 > n.b.0 + 1.0 && p.0 < n.b.2 - 1.0 && p.1 > n.b.1 + 1.0 && p.1 < n.b.3 - 1.0
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

/// Flowchart samples: no more edge-through-node crossings and no more label
/// collisions than before #402.
#[test]
fn flowchart_samples_add_no_crossings_or_label_collisions() {
    let current: Vec<(String, String)> = crate::SAMPLES
        .iter()
        .filter(|(_, stem, _)| stem.starts_with("flowchart/"))
        .map(|(_, stem, src)| {
            let svg = render(src).expect("renders");
            (
                stem.to_string(),
                format!("{} {}", crossings(&svg), label_collisions(&svg)),
            )
        })
        .collect();
    let base = baseline("baseline-counts.txt", &current);
    let mut bad = Vec::new();
    for (stem, now) in &current {
        let was = base
            .get(stem)
            .unwrap_or_else(|| panic!("{stem}: no baseline"));
        let n: Vec<usize> = now.split(' ').map(|t| t.parse().expect("count")).collect();
        let w: Vec<usize> = was.split(' ').map(|t| t.parse().expect("count")).collect();
        if n[0] > w[0] || n[1] > w[1] {
            bad.push(format!("{stem}: crossings/collisions {now} (was {was})"));
        }
    }
    assert!(
        bad.is_empty(),
        "new crossings or collisions:\n{}",
        bad.join("\n")
    );
}

#[test]
fn graph_renders_are_deterministic() {
    for (name, src) in graph_sources(&GRAPH) {
        let a = render(&src).unwrap_or_else(|e| panic!("{name}: {e}"));
        let b = render(&src).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(a, b, "{name}: two renders differ");
    }
}
