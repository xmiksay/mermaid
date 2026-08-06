//! Per-subgraph local-`direction` transposition: nesting-depth ordering plus
//! the transpose-in-place pass itself.

use std::collections::{HashMap, HashSet};

use crate::parse::{FlowDirection, FlowchartDiagram, Subgraph};
use crate::sugiyama::NodeId;

use super::nodes::collect_node_ids;

fn is_horizontal(d: FlowDirection) -> bool {
    matches!(d, FlowDirection::LeftRight | FlowDirection::RightLeft)
}

/// Nesting depth of each subgraph (root clusters = 0, each level of
/// `child_subgraph_ids` nesting adds one), used to order [`apply_local_directions`]
/// and [`super::nodes::compute_subgraph_boxes`] innermost-first so an
/// ancestor's transpose/frame carries its already-finalized descendants along
/// rigidly instead of re-deriving their positions.
pub(super) fn subgraph_depths(
    d: &FlowchartDiagram,
    sub_idx_by_id: &HashMap<&str, usize>,
) -> Vec<usize> {
    let mut depths = vec![0usize; d.subgraphs.len()];
    let mut is_child = vec![false; d.subgraphs.len()];
    for sub in &d.subgraphs {
        for child_id in &sub.child_subgraph_ids {
            if let Some(&ci) = sub_idx_by_id.get(child_id.as_str()) {
                is_child[ci] = true;
            }
        }
    }
    fn assign(
        i: usize,
        depth: usize,
        subs: &[Subgraph],
        sub_idx_by_id: &HashMap<&str, usize>,
        depths: &mut [usize],
    ) {
        depths[i] = depths[i].max(depth);
        for child_id in &subs[i].child_subgraph_ids {
            if let Some(&ci) = sub_idx_by_id.get(child_id.as_str()) {
                assign(ci, depth + 1, subs, sub_idx_by_id, depths);
            }
        }
    }
    for (i, &child) in is_child.iter().enumerate() {
        if !child {
            assign(i, 0, &d.subgraphs, sub_idx_by_id, &mut depths);
        }
    }
    depths
}

/// Apply each subgraph's local `direction` by transposing its members (and
/// their internal edges) in place about the cluster centre. Only clusters whose
/// flow axis differs from the diagram's are affected — a TD chain inside a
/// `direction LR` subgraph becomes a horizontal row, matching upstream.
///
/// Clusters are processed innermost-first (by nesting depth, [`subgraph_depths`])
/// so a nested subgraph's own local-direction layout (whether transposed by
/// its own pass or left alone because its local direction already matches the
/// global one) is fully finalized before any ancestor touches it. When an
/// ancestor transposes, only its *direct* member nodes get the per-node
/// swap-about-centre treatment; each direct child subgraph's recursive
/// membership is instead moved as a single rigid block (translated so its
/// centroid lands where the swap would put it, without re-swapping each
/// descendant's coordinate individually). Swapping every recursive descendant
/// uniformly (the previous approach) flattened out a nested subgraph's own
/// distinct orientation, e.g. reorienting an inner `direction TB` stack
/// sideways just because an outer `direction LR` cluster contains it.
pub(super) fn apply_local_directions(
    d: &FlowchartDiagram,
    global_dir: FlowDirection,
    id_to_u32: &HashMap<String, NodeId>,
    pos: &mut HashMap<NodeId, (f64, f64)>,
    edge_pts: &mut HashMap<(NodeId, NodeId), Vec<(f64, f64)>>,
) {
    let sub_idx_by_id: HashMap<&str, usize> = d
        .subgraphs
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();
    let depths = subgraph_depths(d, &sub_idx_by_id);
    let mut order: Vec<usize> = (0..d.subgraphs.len()).collect();
    order.sort_by(|&a, &b| depths[b].cmp(&depths[a]));

    for i in order {
        let sub = &d.subgraphs[i];
        let Some(local) = sub.direction else { continue };
        if is_horizontal(local) == is_horizontal(global_dir) {
            continue;
        }
        let mut ids: Vec<&str> = Vec::new();
        collect_node_ids(sub, &d.subgraphs, &sub_idx_by_id, &mut ids);
        let members: Vec<NodeId> = ids
            .iter()
            .filter_map(|id| id_to_u32.get(*id).copied())
            .collect();
        if members.len() < 2 {
            continue;
        }
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        );
        for &u in &members {
            let (x, y) = pos[&u];
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
        let (cx, cy) = ((min_x + max_x) / 2.0, (min_y + max_y) / 2.0);
        let transpose = |(x, y): (f64, f64)| (cx + (y - cy), cy + (x - cx));

        // Direct members: transpose individually about the cluster centre.
        let direct_members: HashSet<NodeId> = sub
            .node_ids
            .iter()
            .filter_map(|id| id_to_u32.get(id.as_str()).copied())
            .collect();
        // Each direct child subgraph's full recursive membership: moved as one
        // rigid block (translation only), keyed by a synthetic block index so
        // an edge between two different child blocks can be told apart from
        // an edge that stays within a single block.
        let mut child_blocks: Vec<(HashSet<NodeId>, (f64, f64))> = Vec::new();
        for child_id in &sub.child_subgraph_ids {
            let Some(&ci) = sub_idx_by_id.get(child_id.as_str()) else {
                continue;
            };
            let mut child_ids: Vec<&str> = Vec::new();
            collect_node_ids(
                &d.subgraphs[ci],
                &d.subgraphs,
                &sub_idx_by_id,
                &mut child_ids,
            );
            let block_members: HashSet<NodeId> = child_ids
                .iter()
                .filter_map(|id| id_to_u32.get(*id).copied())
                .collect();
            if block_members.is_empty() {
                continue;
            }
            let (mut bmin_x, mut bmin_y, mut bmax_x, mut bmax_y) = (
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            );
            for &u in &block_members {
                let (x, y) = pos[&u];
                bmin_x = bmin_x.min(x);
                bmin_y = bmin_y.min(y);
                bmax_x = bmax_x.max(x);
                bmax_y = bmax_y.max(y);
            }
            let (bcx, bcy) = ((bmin_x + bmax_x) / 2.0, (bmin_y + bmax_y) / 2.0);
            let (ncx, ncy) = transpose((bcx, bcy));
            child_blocks.push((block_members, (ncx - bcx, ncy - bcy)));
        }

        let member_set: HashSet<NodeId> = members.iter().copied().collect();
        for &u in &direct_members {
            let p = transpose(pos[&u]);
            pos.insert(u, p);
        }
        for (block_members, (dx, dy)) in &child_blocks {
            for &u in block_members {
                if let Some(p) = pos.get_mut(&u) {
                    p.0 += dx;
                    p.1 += dy;
                }
            }
        }

        // Classify each recursive member by which rigid group moved it, so an
        // edge's polyline is only replayed through the same operation (swap or
        // translate) when both endpoints share a group; an edge crossing
        // groups (e.g. a direct member to a nested child block) falls back to
        // a straight line between the new endpoints, matching how an edge
        // leaving the cluster entirely is already handled below.
        #[derive(PartialEq, Clone, Copy)]
        enum Group {
            Direct,
            Child(usize),
        }
        let mut node_group: HashMap<NodeId, Group> = HashMap::new();
        for &u in &direct_members {
            node_group.insert(u, Group::Direct);
        }
        for (ci, (block_members, _)) in child_blocks.iter().enumerate() {
            for &u in block_members {
                node_group.insert(u, Group::Child(ci));
            }
        }

        for (&(a, b), pts) in edge_pts.iter_mut() {
            let (a_in, b_in) = (member_set.contains(&a), member_set.contains(&b));
            if a_in && b_in {
                match (node_group.get(&a), node_group.get(&b)) {
                    (Some(Group::Direct), Some(Group::Direct)) => {
                        for p in pts.iter_mut() {
                            *p = transpose(*p);
                        }
                    }
                    (Some(Group::Child(i)), Some(Group::Child(j))) if i == j => {
                        let (_, (dx, dy)) = &child_blocks[*i];
                        for p in pts.iter_mut() {
                            p.0 += dx;
                            p.1 += dy;
                        }
                    }
                    _ => {
                        *pts = vec![pos[&a], pos[&b]];
                    }
                }
            } else if a_in || b_in {
                *pts = vec![pos[&a], pos[&b]];
            }
        }
    }
}
