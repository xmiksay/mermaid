//! Flowchart edges vs nodes and labels: no edge runs through a node it does
//! not end on (#403), and no more label collisions than before #402
//! (`baseline-counts.txt`, rewritten by `EDGE_BASELINE_WRITE=1`).

use mermaid_svg::render;

use crate::edge_canvas::baseline;
use crate::edge_ends::graph_sources;
use crate::svg_read::{drawn, labels, node_at, nodes, Kind, Node, Pt};
use crate::svg_shapes::frames;

/// `edge:node` pairs (drawing order) where an edge runs through a node it
/// does not end on; `-` for none. An edge ending on a subgraph frame owns
/// the nodes inside that frame.
fn crossings(svg: &str) -> String {
    let (ns, fs) = (nodes(svg), frames(svg));
    let pairs: Vec<String> = drawn(svg)
        .iter()
        .enumerate()
        .flat_map(|(ei, d)| {
            let tips = [d.raw[0], d.raw[d.raw.len() - 1]];
            let ends = tips.map(|p| node_at(&ns, p));
            let owned: Vec<&Node> = fs
                .iter()
                .filter(|f| tips.iter().any(|&p| f.on_outline(p)))
                .collect();
            let owns = |n: &Node| {
                let c = n.center();
                owned
                    .iter()
                    .any(|f| f.b.0 <= c.0 && c.0 <= f.b.2 && f.b.1 <= c.1 && c.1 <= f.b.3)
            };
            ns.iter()
                .enumerate()
                .filter(|&(i, n)| !ends.contains(&Some(i)) && !owns(n))
                .filter(|(_, n)| walk(&d.curve).any(|p| inside(n, p)))
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

/// The curve's points with each straight stretch between them walked in
/// steps of at most 1px, so a long line segment is checked along its length.
fn walk(curve: &[Pt]) -> impl Iterator<Item = Pt> + '_ {
    curve.windows(2).flat_map(|w| {
        let (a, b) = (w[0], w[1]);
        let steps = ((b.0 - a.0).hypot(b.1 - a.1).ceil() as usize).clamp(1, 10_000);
        (0..=steps).map(move |k| {
            let t = k as f64 / steps as f64;
            (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
        })
    })
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
fn flowchart_edges_cross_no_nodes() {
    let bad: Vec<String> = graph_sources(&["flowchart"])
        .into_iter()
        .filter_map(|(name, src)| {
            let svg = render(&src).unwrap_or_else(|e| panic!("{name}: {e}"));
            let now = crossings(&svg);
            (now != "-").then(|| format!("{name}: edge:node {now}"))
        })
        .collect();
    assert!(
        bad.is_empty(),
        "edges through nodes they do not end on:\n{}",
        bad.join("\n")
    );
}

/// The gate above sees a crossing: a straight edge from A to C drawn right
/// through B is reported, the same edge bent around B is not.
#[test]
fn crossing_check_reports_an_edge_through_a_node() {
    let node = |x: f64| {
        format!(
            "<rect x=\"{x}\" y=\"0\" width=\"40\" height=\"20\" fill=\"#ECECFF\" stroke=\"#9370DB\"/>"
        )
    };
    let svg = |d: &str| {
        format!(
            "<svg viewBox=\"0 0 300 100\">{}{}{}<path d=\"{d}\" fill=\"none\" stroke-width=\"1.5\"/></svg>",
            node(0.0),
            node(100.0),
            node(200.0)
        )
    };
    assert_eq!(crossings(&svg("M40 10L200 10")), "0:1");
    assert_eq!(
        crossings(&svg("M40 10L70 10L70 60L170 60L170 10L200 10")),
        "-"
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
