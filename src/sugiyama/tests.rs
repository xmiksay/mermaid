use std::collections::HashMap;

use super::{layout_with, Graph, Layout, LayoutConfig, LayoutError, NodeId};

fn layout(g: &Graph) -> Result<Layout, LayoutError> {
    layout_with(g, &LayoutConfig::default())
}

fn make_graph(nodes: &[NodeId], edges: &[(NodeId, NodeId)], size: (f64, f64)) -> Graph {
    let node_size: HashMap<_, _> = nodes.iter().map(|&n| (n, size)).collect();
    Graph {
        nodes: nodes.to_vec(),
        edges: edges.to_vec(),
        node_size,
    }
}

#[test]
fn empty_graph() {
    let g = Graph::default();
    let l = layout(&g).unwrap();
    assert!(l.node_pos.is_empty());
    assert!(l.edge_points.is_empty());
}

#[test]
fn single_node() {
    let g = make_graph(&[1], &[], (40.0, 20.0));
    let l = layout(&g).unwrap();
    assert_eq!(l.node_pos.len(), 1);
    assert!(l.node_pos.contains_key(&1));
}

#[test]
fn linear_chain() {
    let g = make_graph(&[1, 2, 3], &[(1, 2), (2, 3)], (40.0, 20.0));
    let l = layout(&g).unwrap();
    let y1 = l.node_pos[&1].1;
    let y2 = l.node_pos[&2].1;
    let y3 = l.node_pos[&3].1;
    assert!(y1 < y2 && y2 < y3, "layers must increase in y");
    assert_eq!(l.edge_points.len(), 2);
    for (_, pts) in &l.edge_points {
        assert!(pts.len() >= 2);
    }
}

#[test]
fn long_edge_gets_waypoints() {
    // 1 -> 2 -> 3 -> 4 -> 5 plus a long edge 1 -> 5 (span = 4 → 3 dummies → 5 waypoints)
    let g = make_graph(
        &[1, 2, 3, 4, 5],
        &[(1, 2), (2, 3), (3, 4), (4, 5), (1, 5)],
        (40.0, 20.0),
    );
    let l = layout(&g).unwrap();
    let long_edge = l.edge_points.get(&(1, 5)).expect("long edge present");
    // endpoints + 3 dummies
    assert_eq!(long_edge.len(), 5);
    // y must monotonically increase along the polyline
    for w in long_edge.windows(2) {
        assert!(w[0].1 <= w[1].1 + 1e-6, "polyline y not monotonic");
    }
}

#[test]
fn cycle_is_broken() {
    let g = make_graph(&[1, 2, 3], &[(1, 2), (2, 3), (3, 1)], (40.0, 20.0));
    let l = layout(&g).unwrap();
    // All three nodes positioned without error.
    assert_eq!(l.node_pos.len(), 3);
    // All three edges have waypoints (in user-supplied direction).
    assert!(l.edge_points.contains_key(&(1, 2)));
    assert!(l.edge_points.contains_key(&(2, 3)));
    assert!(l.edge_points.contains_key(&(3, 1)));
    // The reversed edge (3 -> 1) must still produce a polyline from 3's
    // position to 1's position.
    let pts = &l.edge_points[&(3, 1)];
    let p3 = l.node_pos[&3];
    let p1 = l.node_pos[&1];
    assert!((pts.first().unwrap().0 - p3.0).abs() < 1e-6);
    assert!((pts.first().unwrap().1 - p3.1).abs() < 1e-6);
    assert!((pts.last().unwrap().0 - p1.0).abs() < 1e-6);
    assert!((pts.last().unwrap().1 - p1.1).abs() < 1e-6);
}

#[test]
fn self_loop_present_in_output() {
    let g = make_graph(&[1, 2], &[(1, 1), (1, 2)], (40.0, 20.0));
    let l = layout(&g).unwrap();
    assert!(l.edge_points.contains_key(&(1, 1)));
    assert!(l.edge_points.contains_key(&(1, 2)));
}

#[test]
fn diamond_no_overlap() {
    // 1 splits to 2 & 3, both merge into 4.
    let g = make_graph(&[1, 2, 3, 4], &[(1, 2), (1, 3), (2, 4), (3, 4)], (40.0, 20.0));
    let l = layout(&g).unwrap();
    // 2 and 3 are on the same layer — verify they don't overlap.
    let p2 = l.node_pos[&2];
    let p3 = l.node_pos[&3];
    assert!((p2.1 - p3.1).abs() < 1e-6, "siblings on same layer");
    assert!((p2.0 - p3.0).abs() >= 40.0, "nodes don't overlap horizontally");
}

#[test]
fn errors_on_unknown_node() {
    let mut g = make_graph(&[1, 2], &[(1, 3)], (10.0, 10.0));
    g.edges = vec![(1, 99)];
    let err = layout(&g).unwrap_err();
    match err {
        super::LayoutError::UnknownNode(99) => {}
        e => panic!("unexpected error: {e:?}"),
    }
}

#[test]
fn errors_on_missing_size() {
    let g = Graph {
        nodes: vec![1, 2],
        edges: vec![(1, 2)],
        node_size: HashMap::from([(1, (10.0, 10.0))]),
    };
    assert_eq!(
        layout(&g).unwrap_err(),
        super::LayoutError::MissingSize(2)
    );
}
