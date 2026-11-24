//! The outline a relation is clipped against is the outline that is drawn:
//! read the queue's and the database's path back out of the SVG and check
//! every point of their caps lies on the clip outline.

use super::super::outline::{contains, outline_for, Outline, Rect};
use super::*;

/// The numbers of the first `<path d="…">` starting with `prefix`.
fn path_numbers(svg: &str, prefix: &str) -> Vec<f64> {
    let start = svg
        .find(&format!("d=\"{prefix}"))
        .expect("the shape's path is drawn");
    let d = &svg[start + 3..];
    let d = &d[..d.find('"').expect("closed attribute")];
    d.split(|c: char| c.is_ascii_alphabetic() || c.is_whitespace())
        .filter(|t| !t.is_empty())
        .filter_map(|t| t.parse().ok())
        .collect()
}

fn on_outline(o: Outline, r: Rect, p: (f64, f64)) -> bool {
    let probes = [(0.3, 0.0), (-0.3, 0.0), (0.0, 0.3), (0.0, -0.3)];
    let n = probes
        .iter()
        .filter(|(dx, dy)| contains(o, r, (p.0 + dx, p.1 + dy)))
        .count();
    n > 0 && n < 4
}

fn render(src: &str) -> String {
    crate::render(src).expect("renders")
}

#[test]
fn a_queue_is_clipped_against_the_caps_it_draws() {
    // A long name and description make the queue much taller than its caps
    // are wide: the caps must stay narrow ellipses, in both layout modes.
    let body = "ContainerQueue(q, \"Payment events queue with a long name\", \"Kafka\", \
        \"Carries every domain event from all of the services to the analytics pipeline and the audit log consumers and more\")\n\
        Person(u, \"User\")\nRel(u, q, \"Publishes\")\n";
    for layout in ["", "LAYOUT_LEFT_RIGHT()\n"] {
        let svg = render(&format!("C4Container\n{layout}{body}"));
        // M l top L r top A rx ry 0 0 1 r bot L l bot A rx ry 0 0 1 l top Z
        let n = path_numbers(&svg, "M ");
        let (l, top, r, rx, ry, bot) = (n[0], n[1], n[2], n[4], n[5], n[10]);
        assert!(
            (rx - super::super::text::queue_rx(bot - top)).abs() < 0.01,
            "{rx}"
        );
        assert!(
            (ry - (bot - top) / 2.0).abs() < 0.01,
            "caps span the height"
        );
        assert!(bot - top > 2.0 * rx, "a tall queue: {}", bot - top);
        let rect = (l - rx, top, r - l + 2.0 * rx, bot - top);
        let o = outline_for(C4ElementKind::ContainerQueue);
        for k in 0..=24 {
            let t = std::f64::consts::PI * (k as f64 / 24.0 - 0.5);
            let cy = top + ry;
            let front = (r + rx * t.cos(), cy + ry * t.sin());
            let back = (l - rx * t.cos(), cy + ry * t.sin());
            assert!(on_outline(o, rect, front), "front cap {front:?}");
            assert!(on_outline(o, rect, back), "back cap {back:?}");
        }
    }
}

#[test]
fn a_database_is_clipped_against_the_caps_it_draws() {
    let svg = render(
        "C4Container\nContainerDb(d, \"Ledger database with a long name\", \"Postgres\", \
         \"Every posting, balance and audit record of every account and more\")\n",
    );
    // M x top L x bot A rx ry 0 0 0 x2 bot L x2 top
    let n = path_numbers(&svg, "M ");
    let (x, top, bot, rx, ry, x2) = (n[0], n[1], n[3], n[4], n[5], n[9]);
    assert_eq!(ry, super::super::text::CYL_RY);
    assert!((rx - (x2 - x) / 2.0).abs() < 0.01);
    let rect = (x, top - ry, x2 - x, bot - top + 2.0 * ry);
    let o = outline_for(C4ElementKind::ContainerDb);
    for k in 0..=24 {
        let t = std::f64::consts::PI * k as f64 / 24.0;
        let p = (x + rx + rx * t.cos(), bot + ry * t.sin());
        let q = (x + rx + rx * t.cos(), top - ry * t.sin());
        assert!(on_outline(o, rect, p), "bottom cap {p:?}");
        assert!(on_outline(o, rect, q), "top cap {q:?}");
    }
}
