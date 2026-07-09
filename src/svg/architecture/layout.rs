//! Port-hint grid placement, `align` directives, and orthogonal edge routing.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write as _;

use crate::parse::{ArchAlign, ArchAlignAxis, ArchEdge, ArchSide};
use crate::sugiyama::NodeId;

use crate::svg::builder::fnum;

/// Assigns integer grid coordinates `(col, row)` to each node from the edge port
/// hints. Following an edge `from:S₁ -- S₂:to`, the neighbour sits one cell away
/// in the direction of the anchored node's named side (`L`→left, `R`→right,
/// `T`→up, `B`→down), so an `L`/`R` pair shares a row and a `T`/`B` pair shares a
/// column. Each connected component is grown breadth-first from its
/// source-order seed; separate components (and edge-less nodes) start in fresh
/// columns so they don't overlap.
/// Node → its neighbours and the grid direction each neighbour sits in.
type Adjacency<'a> = HashMap<&'a str, Vec<(&'a str, (i32, i32))>>;

pub(super) fn grid_place(ids: &[String], edges: &[ArchEdge]) -> HashMap<String, (i32, i32)> {
    let in_group: HashSet<&str> = ids.iter().map(String::as_str).collect();

    // Adjacency: from each node, its neighbour and the direction that neighbour
    // sits relative to it.
    let mut adj: Adjacency = HashMap::new();
    for e in edges {
        if in_group.contains(e.from.as_str()) && in_group.contains(e.to.as_str()) {
            adj.entry(&e.from)
                .or_default()
                .push((&e.to, side_delta(e.from_side)));
            adj.entry(&e.to)
                .or_default()
                .push((&e.from, side_delta(e.to_side)));
        }
    }

    let mut pos: HashMap<String, (i32, i32)> = HashMap::new();
    let mut occupied: HashSet<(i32, i32)> = HashSet::new();
    let mut next_base_col = 0;

    for start in ids {
        if pos.contains_key(start) {
            continue;
        }
        let mut base = (next_base_col, 0);
        while occupied.contains(&base) {
            base.0 += 1;
        }
        pos.insert(start.clone(), base);
        occupied.insert(base);

        let mut queue: VecDeque<&str> = VecDeque::new();
        queue.push_back(start);
        while let Some(cur) = queue.pop_front() {
            let cpos = pos[cur];
            let Some(neighbours) = adj.get(cur) else {
                continue;
            };
            for (nb, d) in neighbours.clone() {
                if pos.contains_key(nb) {
                    continue;
                }
                let mut np = (cpos.0 + d.0, cpos.1 + d.1);
                // On a collision, keep stepping in the same direction until the
                // cell is free (a straight axis move can't loop).
                while occupied.contains(&np) {
                    np = (np.0 + d.0, np.1 + d.1);
                }
                pos.insert(nb.to_string(), np);
                occupied.insert(np);
                queue.push_back(nb);
            }
        }
        // Start the next component to the right of everything placed so far.
        next_base_col = occupied.iter().map(|&(c, _)| c).max().unwrap_or(0) + 2;
    }
    pos
}

/// Unit grid step for a port side: `L`→left, `R`→right, `T`→up, `B`→down.
fn side_delta(side: ArchSide) -> (i32, i32) {
    match side {
        ArchSide::Left => (-1, 0),
        ArchSide::Right => (1, 0),
        ArchSide::Top => (0, -1),
        ArchSide::Bottom => (0, 1),
    }
}

/// Repositions each `align` directive's members into a shared row (common y,
/// boxes laid left→right) or column (common x, boxes laid top→bottom), anchored
/// at the members' current top-left corner so the arrangement replaces the
/// grid default in place. Directives naming fewer than two members present in
/// this group are ignored.
pub(super) fn apply_aligns(
    positions: &mut HashMap<NodeId, (f64, f64)>,
    id_to_node: &HashMap<String, NodeId>,
    node_size: &HashMap<NodeId, (f64, f64)>,
    aligns: &[ArchAlign],
) {
    const GAP: f64 = 36.0;
    for a in aligns {
        let members: Vec<NodeId> = a
            .ids
            .iter()
            .filter_map(|id| id_to_node.get(id).copied())
            .collect();
        if members.len() < 2 {
            continue;
        }
        let anchor_x = members
            .iter()
            .map(|&n| positions[&n].0)
            .fold(f64::INFINITY, f64::min);
        let anchor_y = members
            .iter()
            .map(|&n| positions[&n].1)
            .fold(f64::INFINITY, f64::min);
        match a.axis {
            ArchAlignAxis::Row => {
                let mut left = anchor_x - node_size[&members[0]].0 / 2.0;
                for &n in &members {
                    let w = node_size[&n].0;
                    positions.insert(n, (left + w / 2.0, anchor_y));
                    left += w + GAP;
                }
            }
            ArchAlignAxis::Column => {
                let mut top = anchor_y - node_size[&members[0]].1 / 2.0;
                for &n in &members {
                    let h = node_size[&n].1;
                    positions.insert(n, (anchor_x, top + h / 2.0));
                    top += h + GAP;
                }
            }
        }
    }
}

/// Midpoint of the named side of a rect — where an edge port attaches.
pub(super) fn port_point(center: (f64, f64), w: f64, h: f64, side: ArchSide) -> (f64, f64) {
    let (cx, cy) = center;
    let hw = w / 2.0;
    let hh = h / 2.0;
    match side {
        ArchSide::Top => (cx, cy - hh),
        ArchSide::Bottom => (cx, cy + hh),
        ArchSide::Left => (cx - hw, cy),
        ArchSide::Right => (cx + hw, cy),
    }
}

/// Straight orthogonal route between two ports. Ports on horizontal sides
/// (`L`/`R`) leave the box horizontally; vertical sides (`T`/`B`) leave
/// vertically. Same-axis ports connect with a two-segment jog (or a straight
/// line when already aligned); mixed axes connect with a single elbow.
pub(super) fn ortho_route(
    pa: (f64, f64),
    sa: ArchSide,
    pb: (f64, f64),
    sb: ArchSide,
) -> Vec<(f64, f64)> {
    let horiz = |s| matches!(s, ArchSide::Left | ArchSide::Right);
    let (a_h, b_h) = (horiz(sa), horiz(sb));
    match (a_h, b_h) {
        (true, true) => {
            if (pa.1 - pb.1).abs() < 0.5 {
                vec![pa, pb]
            } else {
                let mx = (pa.0 + pb.0) / 2.0;
                vec![pa, (mx, pa.1), (mx, pb.1), pb]
            }
        }
        (false, false) => {
            if (pa.0 - pb.0).abs() < 0.5 {
                vec![pa, pb]
            } else {
                let my = (pa.1 + pb.1) / 2.0;
                vec![pa, (pa.0, my), (pb.0, my), pb]
            }
        }
        (true, false) => vec![pa, (pb.0, pa.1), pb],
        (false, true) => vec![pa, (pa.0, pb.1), pb],
    }
}

pub(super) fn polyline_path(pts: &[(f64, f64)]) -> String {
    let mut s = String::new();
    for (i, (x, y)) in pts.iter().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        let _ = write!(s, "{cmd}{} {}", fnum(*x), fnum(*y));
    }
    s
}
