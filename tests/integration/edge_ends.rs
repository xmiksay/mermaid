//! Flowchart edge ends (#402): rank-relation faces and a straight run under
//! every marker.

use mermaid_svg::render;

use crate::svg_read::{attr, drawn, fixture, fixtures, markers, node_at, nodes, Drawn, Node, Pt};

/// Every edge fixture plus every gallery sample of the graph-shaped kinds,
/// as `(name, source)`.
pub(crate) fn graph_sources(kinds: &[&str]) -> Vec<(String, String)> {
    let samples = crate::SAMPLES
        .iter()
        .filter(|(_, stem, _)| kinds.contains(&stem.split('/').next().unwrap_or("")))
        .map(|(_, stem, src)| (stem.to_string(), src.to_string()));
    let header = |src: &str| src.split_whitespace().next().unwrap_or("").to_string();
    let kind_of = |src: &str| match header(src).as_str() {
        "flowchart" | "graph" => "flowchart",
        "stateDiagram-v2" | "stateDiagram" => "state",
        "classDiagram" => "class",
        "erDiagram" => "er",
        "requirementDiagram" => "requirement",
        _ => "",
    };
    fixtures()
        .into_iter()
        .filter(|(_, src)| kinds.contains(&kind_of(src)))
        .chain(samples)
        .collect()
}

/// Each edge's end nodes, `None` where an end is on no single node outline.
fn end_nodes(edges: &[Drawn], ns: &[Node]) -> Vec<[Option<usize>; 2]> {
    edges
        .iter()
        .map(|d| [node_at(ns, d.raw[0]), node_at(ns, d.raw[d.raw.len() - 1])])
        .collect()
}

/// Ends that keep master's clip-first route: self-loops, opposite pairs and
/// cluster ends.
fn plain(ends: &[[Option<usize>; 2]], i: usize) -> bool {
    match ends[i] {
        [Some(a), Some(b)] => a == b || ends.contains(&[Some(b), Some(a)]),
        _ => true,
    }
}

/// The node drawn with `label`: the one holding the label's text anchor.
fn node_of(svg: &str, ns: &[Node], label: &str) -> usize {
    let at = svg
        .find(&format!(">{label}</text>"))
        .unwrap_or_else(|| panic!("no label {label}"));
    let open = svg[..at].rfind("<text").expect("text tag");
    let tag = &svg[open..at];
    let p = (attr(tag, "x").expect("x"), attr(tag, "y").expect("y"));
    ns.iter()
        .position(|n| p.0 > n.b.0 && p.0 < n.b.2 && p.1 > n.b.1 && p.1 < n.b.3)
        .unwrap_or_else(|| panic!("no node under {label}"))
}

/// The drawn edge `from → to` (by node label): its terminal segment and tip.
fn edge_end(svg: &str, from: &str, to: &str) -> (Pt, Pt, Node) {
    let ns = nodes(svg);
    let (a, b) = (node_of(svg, &ns, from), node_of(svg, &ns, to));
    let edges = drawn(svg);
    let ends = end_nodes(&edges, &ns);
    let i = ends
        .iter()
        .position(|e| *e == [Some(a), Some(b)])
        .unwrap_or_else(|| panic!("no edge {from} -> {to}"));
    let raw = &edges[i].raw;
    (raw[raw.len() - 2], raw[raw.len() - 1], ns[b])
}

/// Asserts `from → to` ends on the target's top (`bottom == false`) or bottom
/// face, with a terminal segment within 10° of vertical.
fn assert_vertical_entry(svg: &str, from: &str, to: &str, bottom: bool) {
    let (before, tip, node) = edge_end(svg, from, to);
    let face_y = if bottom { node.b.3 } else { node.b.1 };
    assert!(
        (tip.1 - face_y).abs() < 1.0 && tip.0 > node.b.0 && tip.0 < node.b.2,
        "{from} -> {to}: tip {tip:?} is not on the {} face of {:?}",
        if bottom { "bottom" } else { "top" },
        node.b
    );
    let (dx, dy) = (tip.0 - before.0, tip.1 - before.1);
    let tilt = dx.atan2(dy.abs()).abs().to_degrees();
    assert!(
        tilt <= 10.0,
        "{from} -> {to}: enters {tilt:.1}° off vertical"
    );
}

#[test]
fn k8s_inter_rank_edges_enter_top_faces() {
    let svg = render(&fixture("k8s")).expect("renders");
    let cp = "k8s-cp-1: control plane, etcd, Traefik";
    let (w1, w2) = ("k8s-worker-1: workloady", "k8s-worker-2: workloady");
    for (from, to) in [
        ("Internet", cp),
        (cp, w1),
        (cp, w2),
        (w2, "postgres VM"),
        (w2, "redpanda VM"),
    ] {
        assert_vertical_entry(&svg, from, to, false);
    }
}

#[test]
fn td_fan_outer_edges_enter_top_faces() {
    let svg = render(&fixture("fan")).expect("renders");
    for to in ["One", "Five"] {
        assert_vertical_entry(&svg, "Start node", to, false);
    }
}

#[test]
fn td_back_edges_enter_bottom_faces() {
    let svg = render(&fixture("td-back")).expect("renders");
    for (from, to) in [("End", "Start"), ("Done", "Middle")] {
        let (_, tip, node) = edge_end(&svg, from, to);
        assert!(
            (tip.1 - node.b.3).abs() < 1.0 && tip.0 > node.b.0 && tip.0 < node.b.2,
            "{from} -> {to}: tip {tip:?} is not on the bottom face of {:?}",
            node.b
        );
    }
}

fn dist_to_line(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    ((p.0 - a.0) * dy - (p.1 - a.1) * dx).abs() / dx.hypot(dy)
}

/// Within a marker's reach from the tip, the drawn curve stays within 1.5px
/// of the arrow's axis (the path's final tangent), so the line joins the
/// marker at its base instead of bending inside it.
#[test]
fn flowchart_lines_run_straight_under_markers() {
    let mut bad = Vec::new();
    for (name, src) in graph_sources(&["flowchart"]) {
        let svg = render(&src).unwrap_or_else(|e| panic!("{name}: {e}"));
        let reach = markers(&svg);
        let ns = nodes(&svg);
        let edges = drawn(&svg);
        let ends = end_nodes(&edges, &ns);
        for (i, d) in edges.iter().enumerate() {
            if plain(&ends, i) {
                continue;
            }
            for which in 0..2 {
                let Some((r, _)) = d.marks[which].as_ref().and_then(|m| reach.get(m)) else {
                    continue;
                };
                let len = r * d.stroke;
                let (mut raw, mut curve) = (d.raw.clone(), d.curve.clone());
                if which == 0 {
                    raw.reverse();
                    curve.reverse();
                }
                let (tip, axis) = (raw[raw.len() - 1], raw[raw.len() - 2]);
                let worst = curve
                    .iter()
                    .filter(|p| (p.0 - tip.0).hypot(p.1 - tip.1) <= len)
                    .map(|&p| dist_to_line(p, axis, tip))
                    .fold(0.0, f64::max);
                if worst > 1.5 {
                    bad.push(format!("{name} edge {i} end {which}: {worst:.2}px"));
                }
            }
        }
    }
    assert!(bad.is_empty(), "bent under the marker:\n{}", bad.join("\n"));
}
