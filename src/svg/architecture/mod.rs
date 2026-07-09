//! architecture-beta renderer. Services are grouped into boxes; layout within
//! each group is driven by the edge **port hints** (`db:L -- R:server`): an
//! `L`/`R` pair shares a row (source's named side faces the target's named
//! side), a `T`/`B` pair shares a column, and edges are routed with straight
//! orthogonal segments between the pinned sides. An `id{group}` endpoint
//! resolves to a group box.

use std::collections::{BTreeMap, HashMap};

use crate::parse::ArchitectureDiagram;
use crate::sugiyama::NodeId;

use super::builder::SvgBuilder;
use super::geometry::polyline_midpoint;
use super::theme::Theme;

mod draw;
mod layout;
#[cfg(test)]
mod tests;

use draw::{draw_arch_icon, truncate_icon_name};
use layout::{apply_aligns, grid_place, ortho_route, polyline_path, port_point};

const PAD: f64 = 30.0;
const SERVICE_W: f64 = 80.0;
const SERVICE_H: f64 = 108.0;
const H_GAP: f64 = 40.0;
const V_GAP: f64 = 40.0;
const GROUP_PAD: f64 = 16.0;
const GROUP_HDR: f64 = 22.0;
const GROUP_GAP: f64 = 30.0;
/// Bare service icon: a large flat blue square (no surrounding container box),
/// matching upstream's ~80px service glyph. The label is drawn below it.
const SERVICE_ICON: f64 = 80.0;
/// White glyph drawn centred on the blue icon square.
const SERVICE_GLYPH: f64 = 48.0;
const GROUP_ICON_SIZE: f64 = 16.0;
const JUNCTION_R: f64 = 6.0;
/// Solid fill of a service icon square — a fixed blue to match the JS reference
/// (upstream renders service icons as solid blue tiles regardless of theme).
const ICON_TILE: &str = "#4a72d6";

struct Placed {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    label: String,
    icon: Option<String>,
    is_junction: bool,
}

pub(crate) fn render(d: &ArchitectureDiagram, theme: &Theme) -> String {
    let fg = &theme.fg;
    let fg_muted = &theme.fg_muted;

    // Collect node ids per parent group (None = top-level).
    let mut by_parent: BTreeMap<Option<String>, Vec<String>> = BTreeMap::new();
    for s in &d.services {
        by_parent
            .entry(s.parent.clone())
            .or_default()
            .push(s.id.clone());
    }
    for j in &d.junctions {
        by_parent
            .entry(j.parent.clone())
            .or_default()
            .push(j.id.clone());
    }

    // Stable group ordering: top-level group None first, then groups in source order,
    // then any parent that wasn't declared as a group.
    let group_order: Vec<Option<String>> = {
        let mut v: Vec<Option<String>> = vec![None];
        for g in &d.groups {
            v.push(Some(g.id.clone()));
        }
        for p in by_parent.keys() {
            if !v.contains(p) {
                v.push(p.clone());
            }
        }
        v
    };

    let mut placed_services: Vec<Placed> = Vec::new();
    // (x, y, w, h, label, icon)
    let mut placed_groups: Vec<(f64, f64, f64, f64, String, Option<String>)> = Vec::new();
    let mut centers: HashMap<String, (f64, f64, f64, f64)> = HashMap::new();
    // Group boxes are valid edge endpoints too (`id{group}` markers).
    let mut group_boxes: HashMap<String, (f64, f64, f64, f64)> = HashMap::new();
    let mut group_y = PAD;
    let mut max_x: f64 = PAD;

    for parent in &group_order {
        let ids = by_parent.get(parent).cloned().unwrap_or_default();
        if ids.is_empty() {
            continue;
        }
        let group_def = parent
            .as_ref()
            .and_then(|p| d.groups.iter().find(|g| &g.id == p));
        let label_h = if group_def.is_some() { GROUP_HDR } else { 0.0 };

        let id_to_node: HashMap<String, NodeId> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| (id.clone(), i as NodeId))
            .collect();
        let node_size: HashMap<NodeId, (f64, f64)> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let s = if d.junctions.iter().any(|j| &j.id == id) {
                    (JUNCTION_R * 2.0, JUNCTION_R * 2.0)
                } else {
                    (SERVICE_W, SERVICE_H)
                };
                (i as NodeId, s)
            })
            .collect();

        // Port-hint grid placement: L/R pairs share a row, T/B pairs share a
        // column. Then compress column/row indices to compact ranks and place
        // each node at its cell centre (group-local layout space).
        let grid = grid_place(&ids, &d.edges);
        let mut cols: Vec<i32> = grid.values().map(|&(c, _)| c).collect();
        cols.sort_unstable();
        cols.dedup();
        let mut rows: Vec<i32> = grid.values().map(|&(_, r)| r).collect();
        rows.sort_unstable();
        rows.dedup();
        let col_rank: HashMap<i32, usize> = cols.iter().enumerate().map(|(i, &c)| (c, i)).collect();
        let row_rank: HashMap<i32, usize> = rows.iter().enumerate().map(|(i, &r)| (r, i)).collect();

        let mut positions: HashMap<NodeId, (f64, f64)> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let n = i as NodeId;
                let &(c, r) = grid.get(id).unwrap_or(&(0, 0));
                let cx = col_rank[&c] as f64 * (SERVICE_W + H_GAP) + SERVICE_W / 2.0;
                let cy = row_rank[&r] as f64 * (SERVICE_H + V_GAP) + SERVICE_H / 2.0;
                (n, (cx, cy))
            })
            .collect();
        apply_aligns(&mut positions, &id_to_node, &node_size, &d.aligns);

        let inner_origin_x = PAD + GROUP_PAD;
        let inner_origin_y = group_y + GROUP_PAD + label_h;

        let mut group_min_x = f64::INFINITY;
        let mut group_min_y = f64::INFINITY;
        let mut group_max_x = f64::NEG_INFINITY;
        let mut group_max_y = f64::NEG_INFINITY;

        for (i, id) in ids.iter().enumerate() {
            let node_id = i as NodeId;
            let (cx, cy) = positions[&node_id];
            let is_j = d.junctions.iter().any(|j| &j.id == id);
            let (w, h) = node_size[&node_id];
            let x = inner_origin_x + cx - w / 2.0;
            let y = inner_origin_y + cy - h / 2.0;

            let svc = d.services.iter().find(|s| &s.id == id);
            placed_services.push(Placed {
                x,
                y,
                w,
                h,
                label: svc
                    .and_then(|s| s.label.clone())
                    .unwrap_or_else(|| id.clone()),
                icon: svc.and_then(|s| s.icon.clone()),
                is_junction: is_j,
            });
            centers.insert(id.clone(), (x, y, w, h));

            group_min_x = group_min_x.min(x);
            group_min_y = group_min_y.min(y);
            group_max_x = group_max_x.max(x + w);
            group_max_y = group_max_y.max(y + h);
        }

        if !group_min_x.is_finite() {
            continue;
        }

        let bx = group_min_x - GROUP_PAD;
        let by = group_min_y - GROUP_PAD - label_h;
        let bw = (group_max_x - group_min_x) + GROUP_PAD * 2.0;
        let bh = (group_max_y - group_min_y) + GROUP_PAD * 2.0 + label_h;

        if let Some(gdef) = group_def {
            placed_groups.push((
                bx,
                by,
                bw,
                bh,
                gdef.label.clone().unwrap_or_else(|| gdef.id.clone()),
                gdef.icon.clone(),
            ));
            group_boxes.insert(gdef.id.clone(), (bx, by, bw, bh));
        }

        group_y = by + bh + GROUP_GAP;
        max_x = max_x.max(bx + bw);
    }

    let width = (max_x + PAD).max(300.0);
    let height = (group_y - GROUP_GAP + PAD).max(160.0);
    let mut svg = SvgBuilder::new(width, height).theme(theme);

    svg.def_arrow_marker("arch-arrow", &theme.flow_edge_stroke, 9, 8);

    for (x, y, w, h, label, icon) in &placed_groups {
        svg.rect(*x, *y, *w, *h,
            &format!("fill=\"none\" stroke=\"{fg_muted}\" stroke-width=\"1.5\" stroke-dasharray=\"6 4\" rx=\"6\""));
        // Group title icon (e.g. `(cloud)`) rendered to the left of the label.
        let mut text_x = x + 10.0;
        if let Some(kind) = icon {
            let iy = y + 4.0;
            if draw_arch_icon(&mut svg, kind, text_x, iy, GROUP_ICON_SIZE, fg, "none") {
                text_x += GROUP_ICON_SIZE + 6.0;
            }
        }
        svg.text(
            text_x,
            y + 14.0,
            &format!("fill=\"{fg}\" font-size=\"12\""),
            label,
        );
    }

    for e in &d.edges {
        // An endpoint may be a service/junction (`centers`) or a group box.
        let resolve = |id: &String| centers.get(id).or_else(|| group_boxes.get(id));
        let (Some(a), Some(b)) = (resolve(&e.from), resolve(&e.to)) else {
            continue;
        };
        let acx = a.0 + a.2 / 2.0;
        let acy = a.1 + a.3 / 2.0;
        let bcx = b.0 + b.2 / 2.0;
        let bcy = b.1 + b.3 / 2.0;

        // Attach to the named sides and route orthogonally between the ports.
        let pa = port_point((acx, acy), a.2, a.3, e.from_side);
        let pb = port_point((bcx, bcy), b.2, b.3, e.to_side);
        let pts = ortho_route(pa, e.from_side, pb, e.to_side);

        let dashed = if e.group {
            " stroke-dasharray=\"5 3\""
        } else {
            ""
        };
        let marker = if e.to_arrow {
            " marker-end=\"url(#arch-arrow)\""
        } else {
            ""
        };
        let marker_start = if e.from_arrow {
            " marker-start=\"url(#arch-arrow)\""
        } else {
            ""
        };
        svg.path(
            &polyline_path(&pts),
            &format!(
                "fill=\"none\" stroke=\"{}\" stroke-width=\"1.5\"{dashed}{marker}{marker_start}",
                &theme.flow_edge_stroke
            ),
        );

        if let Some(title) = &e.label {
            let (mx, my) = polyline_midpoint(&pts);
            svg.text(
                mx,
                my - 3.0,
                &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"11\""),
                title,
            );
        }
    }

    for p in &placed_services {
        if p.is_junction {
            svg.circle(
                p.x + p.w / 2.0,
                p.y + p.h / 2.0,
                JUNCTION_R,
                &format!("fill=\"{fg}\""),
            );
            continue;
        }
        // Upstream renders a service as a large bare blue icon square with the
        // label below it — no surrounding container box.
        let tile = SERVICE_ICON;
        let tx = p.x + (p.w - tile) / 2.0;
        let ty = p.y;
        svg.rect(
            tx,
            ty,
            tile,
            tile,
            &format!("fill=\"{ICON_TILE}\" stroke=\"none\" rx=\"6\""),
        );
        let ix = tx + (tile - SERVICE_GLYPH) / 2.0;
        let iy = ty + (tile - SERVICE_GLYPH) / 2.0;
        let mut label_y = ty + tile + 16.0;
        // A pure-Rust renderer can't fetch Iconify packs (`logos:aws-lambda`
        // etc.), so unrecognized names fall back to a generic glyph plus the
        // name as a caption — the icon identity is shown, not silently lost.
        match &p.icon {
            Some(kind) => {
                if !draw_arch_icon(&mut svg, kind, ix, iy, SERVICE_GLYPH, "#ffffff", "none") {
                    svg.text(
                        p.x + p.w / 2.0,
                        ty + tile + 12.0,
                        &format!("text-anchor=\"middle\" fill=\"{fg_muted}\" font-size=\"9\""),
                        &truncate_icon_name(kind),
                    );
                    label_y = ty + tile + 26.0;
                }
            }
            None => {
                draw_arch_icon(&mut svg, "", ix, iy, SERVICE_GLYPH, "#ffffff", "none");
            }
        }
        svg.text(
            p.x + p.w / 2.0,
            label_y,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"13\""),
            &p.label,
        );
    }

    svg.finish()
}
