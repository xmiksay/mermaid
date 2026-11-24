//! A generated ~100-shape diagram (the reviewer's perf100 class): every
//! relation's line stays out of every shape. Kept to that one check so it
//! stays cheap in a debug build.

use super::invariants::{c4, inside_length, scene};
use super::*;

/// 100 containers in 10 boundaries and 200 relations between them, from a
/// fixed linear congruential sequence.
fn generated(seed: u64) -> String {
    let mut state = seed;
    let mut next = |n: u64| {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) % n
    };
    let mut src = String::from("C4Container\n");
    for b in 0..10 {
        src += &format!("Container_Boundary(b{b}, \"Boundary {b}\") {{\n");
        for k in 0..10 {
            let n = b * 10 + k;
            src += &format!(
                "  Container(n{n}, \"Node {n}\", \"Tech\", \"Description of node {n}\")\n"
            );
        }
        src += "}\n";
    }
    for r in 0..200 {
        let (a, b) = (next(100), next(100));
        if a != b {
            src += &format!("Rel(n{a}, n{b}, \"rel {r} label\")\n");
        }
    }
    src
}

#[test]
fn no_line_runs_through_a_shape_in_a_large_diagram() {
    for seed in [1, 7] {
        let d = c4(&generated(seed));
        let s = scene(&d);
        let p = &s.placement;
        for plan in &s.rels {
            let (x0, y0, x1, y1) = plan
                .line
                .iter()
                .fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |b, q| {
                    (b.0.min(q.0), b.1.min(q.1), b.2.max(q.0), b.3.max(q.1))
                });
            for (el, r) in &p.leaves {
                if r.0 > x1 || r.0 + r.2 < x0 || r.1 > y1 || r.1 + r.3 < y0 {
                    continue;
                }
                let o = outline::outline_for(el.kind);
                let through = inside_length(&plan.line, o, *r);
                assert!(
                    through <= 1.0,
                    "seed {seed}: {} -> {} runs {through:.0}px through {}",
                    plan.rel.from,
                    plan.rel.to,
                    el.alias
                );
            }
        }
    }
}

#[test]
fn a_relation_to_a_boundary_leaves_its_band_before_bending() {
    // Review fuzz f38: a relation ending on a boundary bent inside the layer
    // band a tall neighbouring boundary made, and cut through a shape in it.
    let src = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/c4/f38_long_title.mmd"
    ));
    let d = c4(src);
    let s = scene(&d);
    for plan in &s.rels {
        for (el, r) in &s.placement.leaves {
            let through = inside_length(&plan.line, outline::outline_for(el.kind), *r);
            assert!(
                through <= 1.0,
                "{} -> {} runs {through:.0}px through {}",
                plan.rel.from,
                plan.rel.to,
                el.alias
            );
        }
    }
}
