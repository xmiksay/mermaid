use super::*;
use crate::parse::NodeShape;
use crate::svg::flowchart::edges::EndClip;

fn body(center: Pt, size: (f64, f64)) -> Body {
    Body::new(&EndClip {
        center,
        size,
        shape: Some(NodeShape::Rect),
    })
    .expect("finite")
}

/// The bodies a straight line from `a` to `b` cuts, sampled in `n` steps.
fn cut(scene: &Scene, a: Pt, b: Pt, n: usize) -> Vec<Ob> {
    let samples: Vec<Pt> = (0..=n)
        .map(|k| {
            let t = k as f64 / n as f64;
            (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
        })
        .collect();
    scene
        .hits(&samples, true, 0.0, |_, _| false)
        .iter()
        .map(|h| h.ob)
        .collect()
}

#[test]
fn the_index_finds_what_a_full_scan_finds() {
    // A dense rank of small nodes, one huge node, one far away and a
    // marker; lines across them in both directions and through the gap.
    let mut nodes: Vec<(&str, Body)> = (0..200)
        .map(|i| ("n", body((f64::from(i) * 100.0, 0.0), (60.0, 40.0))))
        .collect();
    nodes.push(("big", body((10_000.0, 500.0), (1e6, 100.0))));
    nodes.push(("far", body((1e12, 1e12), (60.0, 40.0))));
    let marks = vec![(0, ("m", body((5_000.0, 200.0), (20.0, 20.0))))];
    let scene = Scene::new(nodes, marks);
    assert_eq!(cut(&scene, (-50.0, 0.0), (19_950.0, 0.0), 500).len(), 200);
    assert!(cut(&scene, (-50.0, 100.0), (19_950.0, 100.0), 37).is_empty());
    assert_eq!(
        cut(&scene, (5_000.0, -100.0), (5_000.0, 600.0), 3),
        [(true, 50), (false, 0), (true, 200)]
    );
    assert_eq!(cut(&scene, (1e12, 0.0), (1e12, 2e12), 1), [(true, 201)]);
    // Marks are only looked at on request.
    let samples = [(4_990.0, 200.0), (5_010.0, 200.0)];
    assert!(scene.hits(&samples, false, 0.0, |_, _| false).is_empty());
    assert!(cut(&scene, (f64::NAN, 0.0), (0.0, 0.0), 4).is_empty());
}

#[test]
fn a_head_counts_near_it_but_not_for_its_own_edge() {
    let marks = vec![(3, ("a", body((0.0, 0.0), (10.0, 10.0))))];
    let scene = Scene::new(vec![("a", body((0.0, 30.0), (60.0, 40.0)))], marks);
    assert!(scene.head_at("a", (0.0, 0.0), 1));
    assert!(scene.head_at("a", (9.0, 0.0), 1), "within HEAD_ROOM");
    assert!(!scene.head_at("a", (13.0, 0.0), 1));
    assert!(!scene.head_at("a", (0.0, 0.0), 3), "its own head");
    assert!(!scene.head_at("b", (0.0, 0.0), 1), "another node's");
}
