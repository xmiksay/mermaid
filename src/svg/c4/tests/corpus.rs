//! A seeded corpus of small random C4 diagrams — both layout modes, both
//! directions, nested boundaries, queues and databases, self-loops,
//! member/boundary relations and long unbreakable labels — each held to
//! every shared invariant (`check_scene`). Fixtures only cover what someone
//! thought of; this is what keeps the guarantees structural.

use std::panic::{catch_unwind, AssertUnwindSafe};

use super::invariants::{c4, check_scene};

/// Fixed linear congruential sequence (no extra dependencies).
struct Lcg(u64);

impl Lcg {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) % n as u64) as usize
    }

    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }

    fn text(&mut self) -> String {
        match self.below(6) {
            0 => String::new(),
            1 => "Calls".into(),
            2 => "Reads and writes the ledger".into(),
            3 => "sends events to the downstream consumers over the bus".into(),
            4 => "x".repeat(20 + self.below(70)),
            _ => format!("{}{}", "Token", "z".repeat(10 + self.below(30))),
        }
    }
}

struct Gen {
    rng: Lcg,
    lines: Vec<String>,
    leaves: Vec<String>,
    /// (boundary, every alias nested under it)
    boundaries: Vec<(String, Vec<String>)>,
    count: usize,
}

impl Gen {
    fn element(&mut self, depth: usize) -> Vec<String> {
        self.count += 1;
        let indent = "  ".repeat(depth);
        if depth < 2 && self.rng.chance(30) {
            let alias = format!("b{}", self.count);
            let kind = self.rng.pick(&[
                "Boundary",
                "System_Boundary",
                "Container_Boundary",
                "Deployment_Node",
            ]);
            let label = self.rng.text();
            self.lines
                .push(format!("{indent}{kind}({alias}, \"{label}\") {{"));
            let mut inside = Vec::new();
            for _ in 0..1 + self.rng.below(3) {
                inside.extend(self.element(depth + 1));
            }
            self.lines.push(format!("{indent}}}"));
            self.boundaries.push((alias.clone(), inside.clone()));
            inside.push(alias);
            return inside;
        }
        let alias = format!("e{}", self.count);
        let kind = self.rng.pick(&[
            "Person",
            "System",
            "Container",
            "ContainerDb",
            "ContainerQueue",
            "SystemQueue",
            "Component",
            "SystemDb_Ext",
        ]);
        let (label, techn, descr) = (self.rng.text(), self.rng.text(), self.rng.text());
        self.lines.push(format!(
            "{indent}{kind}({alias}, \"{label}\", \"{techn}\", \"{descr}\")"
        ));
        self.leaves.push(alias.clone());
        vec![alias]
    }
}

fn generated(seed: u64) -> String {
    let mut g = Gen {
        rng: Lcg(seed),
        lines: Vec::new(),
        leaves: Vec::new(),
        boundaries: Vec::new(),
        count: 0,
    };
    let header = g
        .rng
        .pick(&["C4Context", "C4Container", "C4Component", "C4Deployment"]);
    g.lines.push(header.into());
    match g.rng.below(3) {
        0 => g.lines.push("LAYOUT_TOP_DOWN()".into()),
        1 => g.lines.push("LAYOUT_LEFT_RIGHT()".into()),
        _ => {}
    }
    for _ in 0..2 + g.rng.below(5) {
        g.element(0);
    }
    let rels = g.rng.below(13);
    for _ in 0..rels {
        let (from, to) = if !g.boundaries.is_empty() && g.rng.chance(15) {
            let (b, inside) = g.boundaries[g.rng.below(g.boundaries.len())].clone();
            let m = inside[g.rng.below(inside.len())].clone();
            if g.rng.chance(50) {
                (m, b)
            } else {
                (b, m)
            }
        } else {
            let a = g.leaves[g.rng.below(g.leaves.len())].clone();
            let b = if g.rng.chance(10) {
                a.clone()
            } else {
                g.leaves[g.rng.below(g.leaves.len())].clone()
            };
            (a, b)
        };
        let kind = g.rng.pick(&["Rel", "Rel", "Rel", "BiRel", "Rel_Back"]);
        let (label, techn) = (g.rng.text(), g.rng.text());
        g.lines
            .push(format!("{kind}({from}, {to}, \"{label}\", \"{techn}\")"));
    }
    g.lines.join("\n") + "\n"
}

#[test]
fn generated_corpus_holds_every_invariant() {
    let mut failures = Vec::new();
    // The first 120 seeds, plus seeds the review's wider runs caught.
    let regressions = [123, 144, 364, 435, 515, 592];
    for seed in (0..120).chain(regressions) {
        let src = generated(seed);
        let d = c4(&src);
        if let Err(e) = catch_unwind(AssertUnwindSafe(|| check_scene(&d))) {
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            failures.push(format!("seed {seed}: {msg}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} failing:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
#[ignore = "prints one corpus diagram: MERMAID_SEED=<n>"]
fn print_corpus_diagram() {
    let seed = std::env::var("MERMAID_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    println!("{}", generated(seed));
}
