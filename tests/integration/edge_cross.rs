//! Flowchart edges vs nodes and labels: no edge runs through a node it does
//! not end on (#403), and no more label collisions than before #402
//! (`baseline-counts.txt`, rewritten by `EDGE_BASELINE_WRITE=1`).

use mermaid_svg::render;

use crate::edge_canvas::baseline;
use crate::edge_ends::graph_sources;
use crate::flow_nodes::{flow_nodes, node_of_tip, FlowNode};
use crate::svg_read::{drawn, labels, nodes, Node, Pt};
use crate::svg_shapes::frames;

/// `(edge, node, depth)` (drawing order) for each node an edge comes within
/// `reach` px of without ending on it — `depth` the closest it gets, signed
/// distance to the outline, negative inside. An edge ending on a subgraph
/// frame owns the nodes inside that frame only when its other end lies
/// inside it too (a member, or a nested frame); an edge owns the nodes a
/// broken layout stacks on its end nodes.
pub(crate) fn touching(svg: &str, reach: f64) -> Vec<(usize, usize, f64)> {
    let (ns, fs) = (flow_nodes(svg), frames(svg));
    let within =
        |b: (f64, f64, f64, f64), c: Pt| b.0 <= c.0 && c.0 <= b.2 && b.1 <= c.1 && c.1 <= b.3;
    let mut out = Vec::new();
    for (ei, d) in drawn(svg).iter().enumerate() {
        let tips = [d.raw[0], d.raw[d.raw.len() - 1]];
        let ends = tips.map(|p| node_of_tip(&ns, p));
        let owned: Vec<&Node> = fs
            .iter()
            .filter(|f| {
                (f.on_outline(tips[0]) && within(f.b, tips[1]))
                    || (f.on_outline(tips[1]) && within(f.b, tips[0]))
            })
            .collect();
        // A node stacked on an end node (overlapping it by more than 1px)
        // leaves the edge no way past it.
        let stacked = |n: &FlowNode| {
            ends.iter().flatten().filter_map(|&e| ns.get(e)).any(|e| {
                n.b.0.max(e.b.0) + 1.0 < n.b.2.min(e.b.2)
                    && n.b.1.max(e.b.1) + 1.0 < n.b.3.min(e.b.3)
            })
        };
        let owns = |n: &FlowNode| owned.iter().any(|f| within(f.b, n.center())) || stacked(n);
        let pts: Vec<Pt> = walk(&d.curve).collect();
        for (i, n) in ns.iter().enumerate() {
            if ends.contains(&Some(i)) || owns(n) {
                continue;
            }
            let near = |p: &&Pt| n.near_box(**p, reach + 1.0);
            let depth = pts
                .iter()
                .filter(near)
                .map(|&p| n.sdist(p))
                .fold(f64::INFINITY, f64::min);
            if depth < reach {
                out.push((ei, i, depth));
            }
        }
    }
    out
}

/// `edge:node` pairs where an edge runs through a node (more than 1px in)
/// it does not end on; `-` for none.
pub(crate) fn crossings(svg: &str) -> String {
    let pairs: Vec<String> = touching(svg, -1.0)
        .iter()
        .map(|(e, n, _)| format!("{e}:{n}"))
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

/// Closer than this to a node it does not end on, an edge grazes it.
const GRAZE: f64 = 3.0;

/// `edge:node@depth` for each node an edge comes within `GRAZE` px of (or
/// through) without ending on it, `depth` the closest it gets; `-` for none.
fn grazes(svg: &str) -> String {
    let pairs: Vec<String> = touching(svg, GRAZE)
        .iter()
        .map(|(e, n, d)| format!("{e}:{n}@{d:.2}"))
        .collect();
    if pairs.is_empty() {
        "-".into()
    } else {
        pairs.join(",")
    }
}

/// Room for rounding when comparing depths with the baseline.
const SLACK: f64 = 0.05;

/// Detours (#403) bring no edge closer to a node than it came before
/// detours existed: every edge passing within `GRAZE` px of a node it does
/// not end on did so then, the same edge past the same node, at least as
/// closely (`baseline-grazes.txt`, written with detours off; the edges a
/// detour leaves alone keep what they had).
#[test]
fn flowchart_detours_graze_no_nodes() {
    let current: Vec<(String, String)> = graph_sources(&["flowchart"])
        .into_iter()
        .map(|(name, src)| {
            let svg = render(&src).unwrap_or_else(|e| panic!("{name}: {e}"));
            (name, grazes(&svg))
        })
        .collect();
    let base = baseline("baseline-grazes.txt", &current);
    let split = |s: &str| -> Option<(String, f64)> {
        let (pair, d) = s.split_once('@')?;
        Some((pair.to_string(), d.parse().ok()?))
    };
    let mut bad = Vec::new();
    for (name, now) in &current {
        let was: Vec<(String, f64)> = base
            .get(name)
            .map_or(vec![], |w| w.split(',').filter_map(split).collect());
        for (pair, d) in now.split(',').filter_map(split) {
            match was.iter().find(|(p, _)| *p == pair) {
                Some(&(_, before)) if before <= d + SLACK => {}
                Some(&(_, before)) => bad.push(format!(
                    "{name}: edge:node {pair} at {d:.2}px (was {before:.2})"
                )),
                None => bad.push(format!("{name}: edge:node {pair} at {d:.2}px (was clear)")),
            }
        }
    }
    assert!(bad.is_empty(), "new grazes:\n{}", bad.join("\n"));
}

/// The gate above sees a crossing whatever the node's colours, through a
/// straight-sided outline by its real shape: a line through B is reported,
/// the same line bent around B is not, and one through the empty corner of
/// a parallelogram's box is not either.
#[test]
fn crossing_check_reports_an_edge_through_a_node() {
    let rect = |x: f64| {
        format!(
            "<rect x=\"{x}\" y=\"0\" width=\"40\" height=\"20\" fill=\"#bbf\" stroke=\"#33f\" stroke-width=\"2\"/><text>n</text>"
        )
    };
    let lean =
        "<path d=\"M110 0 L140 0 L130 20 L100 20 Z\" fill=\"#fdd\" stroke=\"#900\" stroke-width=\"2\"/><text>p</text>";
    let svg = |d: &str, mid: &str| {
        format!(
            "<svg viewBox=\"0 0 300 100\"><defs></defs><path d=\"{d}\" fill=\"none\" stroke-width=\"1.5\"/>{}{mid}{}</svg>",
            rect(0.0),
            rect(200.0)
        )
    };
    assert_eq!(crossings(&svg("M40 10L200 10", &rect(100.0))), "0:1");
    let around = "M40 10L70 10L70 60L170 60L170 10L200 10";
    assert_eq!(crossings(&svg(around, &rect(100.0))), "-");
    assert_eq!(crossings(&svg("M40 10L200 10", lean)), "0:1");
    let corner = "M40 2L106 2L95 30L95 60L190 60L190 10L200 10";
    assert_eq!(crossings(&svg(corner, lean)), "-");
    // A `style … fill:none` node still counts.
    let hollow = "<rect x=\"100\" y=\"0\" width=\"40\" height=\"20\" fill=\"none\" stroke=\"#9370DB\" stroke-width=\"1.5\"/><text>h</text>";
    assert_eq!(crossings(&svg("M40 10L200 10", hollow)), "0:1");
    // 2px past a node grazes it, 5px past does not.
    let past = |y: f64| format!("M40 10L70 10L70 {y}L170 {y}L170 10L200 10");
    assert!(grazes(&svg(&past(22.0), &rect(100.0))).starts_with("0:1@"));
    assert_eq!(grazes(&svg(&past(25.0), &rect(100.0))), "-");
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
