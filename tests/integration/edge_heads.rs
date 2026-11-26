//! Flowchart arrowheads (#402): different-kind heads never overlap, and no
//! line leaves from another edge's arrowhead tip — beyond what the renderer
//! did before #402 (`baseline-heads.txt`, rewritten by
//! `EDGE_BASELINE_WRITE=1`).

use mermaid_svg::render;

use crate::edge_canvas::baseline;
use crate::edge_ends::graph_sources;
use crate::svg_read::{drawn, Pt};
use crate::svg_shapes::{frames, shape_boxes};

/// Every drawn end: its point and its marker (`None` for a bare end).
fn ends(svg: &str) -> Vec<(Pt, Option<String>)> {
    drawn(svg)
        .into_iter()
        .flat_map(|d| {
            let [s, e] = d.marks;
            [(d.raw[0], s), (d.raw[d.raw.len() - 1], e)]
        })
        .collect()
}

fn dist(a: Pt, b: Pt) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// `(different-kind heads closer than 12px, bare ends within 3px of a head
/// tip)`.
fn head_defects(svg: &str) -> (usize, usize) {
    let all = ends(svg);
    let heads: Vec<&(Pt, Option<String>)> = all.iter().filter(|e| e.1.is_some()).collect();
    let mut kinds = 0;
    for (i, a) in heads.iter().enumerate() {
        kinds += heads[i + 1..]
            .iter()
            .filter(|b| a.1 != b.1 && dist(a.0, b.0) < 12.0)
            .count();
    }
    let tips = heads
        .iter()
        .map(|h| {
            all.iter()
                .filter(|e| e.1.is_none() && dist(h.0, e.0) < 3.0)
                .count()
        })
        .sum();
    (kinds, tips)
}

#[test]
fn flowchart_heads_stay_apart_and_clear_of_lines() {
    let current: Vec<(String, String)> = graph_sources(&["flowchart"])
        .into_iter()
        .map(|(name, src)| {
            let svg = render(&src).unwrap_or_else(|e| panic!("{name}: {e}"));
            let (kinds, tips) = head_defects(&svg);
            (name, format!("{kinds} {tips}"))
        })
        .collect();
    let base = baseline("baseline-heads.txt", &current);
    let mut bad = Vec::new();
    for (name, now) in &current {
        let was = base.get(name).map_or("0 0", String::as_str);
        let n: Vec<usize> = now.split(' ').map(|t| t.parse().expect("count")).collect();
        let w: Vec<usize> = was.split(' ').map(|t| t.parse().expect("count")).collect();
        if n[0] > w[0] || n[1] > w[1] {
            bad.push(format!(
                "{name}: overlapping kinds / lines from a tip {now} (was {was})"
            ));
        }
    }
    assert!(bad.is_empty(), "arrowhead defects:\n{}", bad.join("\n"));
}

/// Every arrowhead, circle and cross tip sits on a node — within 1px of a
/// node shape's box — or on a cluster frame's outline: a head is never
/// detached from its node, however wide its marker. Zero-length edges (a
/// cluster linked to itself) have no direction and are skipped.
#[test]
fn flowchart_marker_tips_stay_on_their_node() {
    let mut bad = Vec::new();
    for (name, src) in graph_sources(&["flowchart"]) {
        let svg = render(&src).unwrap_or_else(|e| panic!("{name}: {e}"));
        let (boxes, frames) = (shape_boxes(&svg), frames(&svg));
        let near = |p: Pt, b: &(f64, f64, f64, f64)| {
            p.0 >= b.0 - 1.0 && p.0 <= b.2 + 1.0 && p.1 >= b.1 - 1.0 && p.1 <= b.3 + 1.0
        };
        for d in drawn(&svg) {
            let (s, e) = (d.raw[0], d.raw[d.raw.len() - 1]);
            if dist(s, e) < 0.01 {
                continue;
            }
            for (tip, mark) in [(s, &d.marks[0]), (e, &d.marks[1])] {
                let attached =
                    boxes.iter().any(|b| near(tip, b)) || frames.iter().any(|f| f.on_outline(tip));
                if mark.is_some() && !attached {
                    bad.push(format!("{name}: tip {tip:?} on no node"));
                }
            }
        }
    }
    assert!(bad.is_empty(), "detached heads:\n{}", bad.join("\n"));
}
