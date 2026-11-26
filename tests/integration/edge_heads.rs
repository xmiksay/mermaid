//! Flowchart arrowheads (#402): different-kind heads never overlap, and no
//! line leaves from another edge's arrowhead tip — beyond what the renderer
//! did before #402 (`baseline-heads.txt`, rewritten by
//! `EDGE_BASELINE_WRITE=1`).

use mermaid_svg::render;

use crate::edge_canvas::baseline;
use crate::edge_ends::graph_sources;
use crate::svg_read::{drawn, markers, nodes, Kind, Pt};
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

/// Every arrowhead, circle and cross tip sits on a node — on a circle's or
/// rhombus's actual outline (within 1px), else within 1px of a node shape's
/// box — or on a cluster frame's outline: a head is never detached from its
/// node, however wide its marker. Zero-length edges (a cluster linked to
/// itself) have no direction and are skipped.
#[test]
fn flowchart_marker_tips_stay_on_their_node() {
    let mut bad = Vec::new();
    for (name, src) in graph_sources(&["flowchart"]) {
        let svg = render(&src).unwrap_or_else(|e| panic!("{name}: {e}"));
        let (boxes, frames) = (shape_boxes(&svg), frames(&svg));
        let round: Vec<_> = nodes(&svg)
            .into_iter()
            .filter(|n| n.kind != Kind::Rect)
            .collect();
        let near = |p: Pt, b: &(f64, f64, f64, f64)| {
            p.0 >= b.0 - 1.0 && p.0 <= b.2 + 1.0 && p.1 >= b.1 - 1.0 && p.1 <= b.3 + 1.0
        };
        for d in drawn(&svg) {
            let (s, e) = (d.raw[0], d.raw[d.raw.len() - 1]);
            if dist(s, e) < 0.01 {
                continue;
            }
            for (tip, mark) in [(s, &d.marks[0]), (e, &d.marks[1])] {
                // Inside a circle's or rhombus's box, only its outline counts.
                let inside = |b: &(f64, f64, f64, f64)| {
                    tip.0 > b.0 + 1.0 && tip.0 < b.2 - 1.0 && tip.1 > b.1 + 1.0 && tip.1 < b.3 - 1.0
                };
                let attached = match round.iter().find(|n| inside(&n.b)) {
                    Some(n) => n.on_outline(tip),
                    None => boxes.iter().any(|b| near(tip, b)),
                } || frames.iter().any(|f| f.on_outline(tip));
                if mark.is_some() && !attached {
                    bad.push(format!("{name}: tip {tip:?} on no node"));
                }
            }
        }
    }
    assert!(bad.is_empty(), "detached heads:\n{}", bad.join("\n"));
}

/// The sharpest turn of the drawn curve within `reach` (arc length) of its
/// tip at `tip_first` (true: the path's start), in degrees: the angle
/// between the directions over 3px either side of each 1px-resampled point.
fn sharpest_turn_near_tip(curve: &[Pt], tip_first: bool, reach: f64) -> f64 {
    let mut pts: Vec<Pt> = curve.to_vec();
    if !tip_first {
        pts.reverse();
    }
    let mut walk = vec![pts[0]];
    for &p in &pts[1..] {
        if dist(*walk.last().expect("non-empty"), p) >= 1.0 {
            walk.push(p);
        }
    }
    let mut worst = 0.0_f64;
    for i in 3..walk.len().saturating_sub(3) {
        if i as f64 > reach {
            break;
        }
        let (a, b, c) = (walk[i - 3], walk[i], walk[i + 3]);
        let turn = ((c.1 - b.1).atan2(c.0 - b.0) - (b.1 - a.1).atan2(b.0 - a.0)).to_degrees();
        worst = worst.max(((turn + 180.0).rem_euclid(360.0) - 180.0).abs());
    }
    worst
}

/// No edge hooks into its head: within one marker length (plus the lead-in
/// margin and the next few px) of an arrowhead the curve never turns more
/// than 60° — on round targets met from the side or far side included.
#[test]
fn edges_do_not_hook_into_their_heads() {
    let mut bad = Vec::new();
    for name in ["circ5", "dense", "s4-flow"] {
        let svg = render(&crate::svg_read::fixture(name)).expect("renders");
        let reach = markers(&svg);
        for (i, d) in drawn(&svg).iter().enumerate() {
            for (which, mark) in d.marks.iter().enumerate() {
                let Some((r, _)) = mark.as_ref().and_then(|m| reach.get(m)) else {
                    continue;
                };
                let turn = sharpest_turn_near_tip(&d.curve, which == 0, r * d.stroke + 10.0);
                if turn > 60.0 {
                    bad.push(format!("{name} edge {i} end {which}: turns {turn:.0}°"));
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "edges hooking into their heads:\n{}",
        bad.join("\n")
    );
}
