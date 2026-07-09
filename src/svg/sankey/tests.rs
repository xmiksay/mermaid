use std::collections::BTreeSet;

use crate::parse::SankeyLink;

use super::columns::{assign_columns, Alignment};
use super::*;

fn link(s: &str, t: &str, v: f64) -> SankeyLink {
    SankeyLink {
        source: s.into(),
        target: t.into(),
        value: v,
    }
}

/// The `y` of the node rect painted with `fill`.
fn rect_y(svg: &str, fill: &str) -> f64 {
    let needle = format!("fill=\"{fill}\" stroke=\"#fff\"");
    let at = svg.find(&needle).expect("node rect present");
    let rect = &svg[..at];
    let ys = rect.rfind("y=\"").expect("y attr");
    let rest = &rect[ys + 3..];
    let end = rest.find('"').unwrap();
    rest[..end].parse().unwrap()
}

/// End-to-end: the energy sample's left column must stack Coal, Solar, Wind,
/// Gas top→bottom — matching Mermaid JS 11.16.0 — which only holds when the
/// renderer feeds d3-sankey ordering upstream's extent/padding (#317).
#[test]
fn energy_sample_column_order_matches_upstream() {
    let d = SankeyDiagram {
        links: vec![
            link("Electricity", "Industry", 250.0),
            link("Electricity", "Transport", 80.0),
            link("Electricity", "Residential", 150.0),
            link("Gas", "Heating", 120.0),
            link("Gas", "Industry", 60.0),
            link("Coal", "Electricity", 300.0),
            link("Solar", "Electricity", 90.0),
            link("Wind", "Electricity", 120.0),
        ],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    // Tableau-10 fills by first appearance: Gas=4, Coal=6, Solar=7, Wind=8.
    let (coal, solar, wind, gas) = (
        rect_y(&svg, "#af7aa1"),
        rect_y(&svg, "#ff9da7"),
        rect_y(&svg, "#9c755f"),
        rect_y(&svg, "#59a14f"),
    );
    assert!(
        coal < solar && solar < wind && wind < gas,
        "left column order"
    );
}

#[test]
fn produces_svg() {
    let d = SankeyDiagram {
        links: vec![
            SankeyLink {
                source: "A".into(),
                target: "B".into(),
                value: 5.0,
            },
            SankeyLink {
                source: "B".into(),
                target: "C".into(),
                value: 3.0,
            },
        ],
        ..Default::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.starts_with("<svg"));
    // Name and value share one line ("A 5"), matching upstream (SVG collapses
    // its `Name\nvalue` newline to whitespace), not a stacked two-line label.
    assert!(svg.contains(">A 5<"));
    assert!(svg.contains(">B 5<"));
    assert!(svg.contains(">C 3<"));
}

fn chain() -> SankeyDiagram {
    SankeyDiagram {
        links: vec![
            SankeyLink {
                source: "A".into(),
                target: "B".into(),
                value: 5.0,
            },
            SankeyLink {
                source: "B".into(),
                target: "C".into(),
                value: 3.0,
            },
        ],
        ..Default::default()
    }
}

#[test]
fn nodes_get_distinct_palette_colors() {
    let svg = render(&chain(), &Theme::default());
    // Each node rect carries its own Tableau-10 color (upstream's hardcoded
    // sankey scale), not the theme's pastel cScale palette.
    for hex in ["#4e79a7", "#f28e2c", "#e15759"] {
        assert!(svg.contains(&format!("fill=\"{hex}\" stroke=\"#fff\"")));
    }
}

#[test]
fn default_link_color_is_gradient() {
    let theme = Theme::default();
    let svg = render(&chain(), &theme);
    // Upstream default is `gradient`: each link is a per-link gradient.
    assert!(svg.contains("<linearGradient id=\"sankey-grad-0\""));
    assert!(svg.contains("stroke=\"url(#sankey-grad-0)\""));
}

#[test]
fn link_color_source_tints_from_source_node() {
    let mut d = chain();
    d.link_color = Some("source".into());
    let svg = render(&d, &Theme::default());
    // A→B tinted from A (node 0), B→C tinted from B (node 1).
    assert!(svg.contains("stroke=\"#4e79a7\" stroke-opacity"));
    assert!(svg.contains("stroke=\"#f28e2c\" stroke-opacity"));
}

#[test]
fn show_values_false_omits_value() {
    let mut d = chain();
    d.show_values = Some(false);
    let svg = render(&d, &Theme::default());
    // Node label carries only the name, no throughput second line.
    assert!(svg.contains(">A<"));
    assert!(!svg.contains(">5<"));
}

#[test]
fn prefix_and_suffix_wrap_value() {
    let mut d = chain();
    d.prefix = Some("$".into());
    d.suffix = Some(" USD".into());
    let svg = render(&d, &Theme::default());
    assert!(svg.contains(">A $5 USD<"));
}

#[test]
fn node_width_config_sets_rect_width() {
    let mut d = chain();
    d.node_width = Some(24.0);
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("width=\"24\""));
}

#[test]
fn link_color_target() {
    let mut d = chain();
    d.link_color = Some("target".into());
    let svg = render(&d, &Theme::default());
    // A→B tinted from B (node 1), B→C tinted from C (node 2).
    assert!(svg.contains("stroke=\"#f28e2c\" stroke-opacity"));
    assert!(svg.contains("stroke=\"#e15759\" stroke-opacity"));
}

#[test]
fn link_color_gradient_emits_defs() {
    let mut d = chain();
    d.link_color = Some("gradient".into());
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("<linearGradient id=\"sankey-grad-0\""));
    assert!(svg.contains("stroke=\"url(#sankey-grad-0)\""));
}

#[test]
fn link_color_hex_literal() {
    let mut d = chain();
    d.link_color = Some("#123456".into());
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("stroke=\"#123456\" stroke-opacity"));
}

#[test]
fn node_alignment_right_pushes_sources_rightward() {
    // Two independent sinks at different left-depths: `justify` and `right`
    // place the terminal nodes in the last column; `left`/`right` differ on
    // where the sources land.
    let d = SankeyDiagram {
        links: vec![
            SankeyLink {
                source: "A".into(),
                target: "X".into(),
                value: 1.0,
            },
            SankeyLink {
                source: "B".into(),
                target: "C".into(),
                value: 1.0,
            },
            SankeyLink {
                source: "C".into(),
                target: "X".into(),
                value: 1.0,
            },
        ],
        ..Default::default()
    };
    let left = assign_columns(&nodes(&d), &d.links, Alignment::Left);
    let right = assign_columns(&nodes(&d), &d.links, Alignment::Right);
    // Left: A at 0. Right: A pulled to just before X (its only path is len 1).
    assert_eq!(left["A"], 0);
    assert_eq!(right["A"], 1);
    // X is a sink; it sits in the last column under both.
    assert_eq!(left["X"], 2);
    assert_eq!(right["X"], 2);
}

#[test]
fn node_alignment_justify_pushes_sinks_to_last_column() {
    let d = SankeyDiagram {
        links: vec![
            SankeyLink {
                source: "A".into(),
                target: "B".into(),
                value: 1.0,
            },
            SankeyLink {
                source: "A".into(),
                target: "C".into(),
                value: 1.0,
            },
            SankeyLink {
                source: "C".into(),
                target: "D".into(),
                value: 1.0,
            },
        ],
        ..Default::default()
    };
    let justify = assign_columns(&nodes(&d), &d.links, Alignment::Justify);
    let left = assign_columns(&nodes(&d), &d.links, Alignment::Left);
    // B is a sink: left keeps it at depth 1, justify pushes it to the last
    // column (2, alongside D).
    assert_eq!(left["B"], 1);
    assert_eq!(justify["B"], 2);
    assert_eq!(justify["D"], 2);
}

fn nodes(d: &SankeyDiagram) -> Vec<String> {
    let mut order = Vec::new();
    let mut seen = BTreeSet::new();
    for l in &d.links {
        for n in [&l.source, &l.target] {
            if seen.insert(n.clone()) {
                order.push(n.clone());
            }
        }
    }
    order
}
