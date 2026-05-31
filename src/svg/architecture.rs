//! architecture-beta renderer. Services are grouped into boxes; layout within
//! each group is driven by sugiyama using the intra-group edges. Inter-group
//! edges are drawn as straight clipped lines after groups are positioned.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use crate::parse::ArchitectureDiagram;
use crate::sugiyama::{layout_with, Graph, LayoutConfig, NodeId};

use super::builder::{fnum, SvgBuilder};
use super::theme::Theme;

const PAD: f64 = 30.0;
const SERVICE_W: f64 = 110.0;
const SERVICE_H: f64 = 86.0;
const GROUP_PAD: f64 = 16.0;
const GROUP_HDR: f64 = 22.0;
const GROUP_GAP: f64 = 30.0;
const ICON_SIZE: f64 = 28.0;
const JUNCTION_R: f64 = 6.0;

struct Placed {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    label: String,
    id: String,
    icon: Option<String>,
    is_junction: bool,
}

pub(crate) fn render(d: &ArchitectureDiagram, theme: &Theme) -> String {
    let fg = theme.fg;
    let fg_muted = theme.fg_muted;
    let stroke = theme.flow_node_stroke;
    let fill = theme.flow_node_fill;

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
    let mut placed_groups: Vec<(f64, f64, f64, f64, String)> = Vec::new();
    let mut centers: HashMap<String, (f64, f64, f64, f64)> = HashMap::new();
    let mut routes: HashMap<(String, String), Vec<(f64, f64)>> = HashMap::new();
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

        // Build a sugiyama Graph for this group only.
        let id_to_node: HashMap<String, NodeId> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| (id.clone(), i as NodeId))
            .collect();

        let mut g = Graph::default();
        for (i, id) in ids.iter().enumerate() {
            g.nodes.push(i as NodeId);
            let size = if d.junctions.iter().any(|j| &j.id == id) {
                (JUNCTION_R * 2.0, JUNCTION_R * 2.0)
            } else {
                (SERVICE_W, SERVICE_H)
            };
            g.node_size.insert(i as NodeId, size);
        }
        for e in &d.edges {
            if let (Some(&u), Some(&v)) = (id_to_node.get(&e.from), id_to_node.get(&e.to)) {
                g.edges.push((u, v));
            }
        }

        let cfg = LayoutConfig {
            layer_gap: 60.0,
            node_gap: 36.0,
            ..LayoutConfig::default()
        };
        let layout = layout_with(&g, &cfg).unwrap_or_default();

        let inner_origin_x = PAD + GROUP_PAD;
        let inner_origin_y = group_y + GROUP_PAD + label_h;

        let mut group_min_x = f64::INFINITY;
        let mut group_min_y = f64::INFINITY;
        let mut group_max_x = f64::NEG_INFINITY;
        let mut group_max_y = f64::NEG_INFINITY;

        for (i, id) in ids.iter().enumerate() {
            let node_id = i as NodeId;
            let (cx, cy) = layout
                .node_pos
                .get(&node_id)
                .copied()
                .unwrap_or((SERVICE_W / 2.0, SERVICE_H / 2.0));
            let is_j = d.junctions.iter().any(|j| &j.id == id);
            let (w, h) = if is_j {
                (JUNCTION_R * 2.0, JUNCTION_R * 2.0)
            } else {
                (SERVICE_W, SERVICE_H)
            };
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
                id: id.clone(),
                icon: svc.and_then(|s| s.icon.clone()),
                is_junction: is_j,
            });
            centers.insert(id.clone(), (x, y, w, h));

            if x < group_min_x {
                group_min_x = x;
            }
            if y < group_min_y {
                group_min_y = y;
            }
            if x + w > group_max_x {
                group_max_x = x + w;
            }
            if y + h > group_max_y {
                group_max_y = y + h;
            }
        }

        // Capture routed polylines for intra-group edges (translated to canvas coords).
        for e in &d.edges {
            if let (Some(&u), Some(&v)) = (id_to_node.get(&e.from), id_to_node.get(&e.to)) {
                if let Some(pts) = layout.edge_points.get(&(u, v)) {
                    let translated: Vec<(f64, f64)> = pts
                        .iter()
                        .map(|(x, y)| (inner_origin_x + x, inner_origin_y + y))
                        .collect();
                    routes.insert((e.from.clone(), e.to.clone()), translated);
                }
            }
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
            ));
        }

        group_y = by + bh + GROUP_GAP;
        if bx + bw > max_x {
            max_x = bx + bw;
        }
    }

    let width = (max_x + PAD).max(300.0);
    let height = (group_y - GROUP_GAP + PAD).max(160.0);
    let mut svg = SvgBuilder::new(width, height).font(theme.font_family, theme.font_size);

    svg.defs_raw(&format!(
        "<marker id=\"arch-arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" \
         markerWidth=\"8\" markerHeight=\"8\" orient=\"auto-start-reverse\">\
         <path d=\"M0,0 L10,5 L0,10 z\" fill=\"{}\"/></marker>",
        theme.flow_edge_stroke
    ));

    for (x, y, w, h, label) in &placed_groups {
        svg.rect(*x, *y, *w, *h,
            &format!("fill=\"none\" stroke=\"{fg_muted}\" stroke-width=\"1.5\" stroke-dasharray=\"6 4\" rx=\"6\""));
        svg.text(
            x + 10.0,
            y + 14.0,
            &format!("fill=\"{fg}\" font-size=\"12\" font-weight=\"bold\""),
            label,
        );
    }

    for e in &d.edges {
        let (Some(a), Some(b)) = (centers.get(&e.from), centers.get(&e.to)) else {
            continue;
        };
        let acx = a.0 + a.2 / 2.0;
        let acy = a.1 + a.3 / 2.0;
        let bcx = b.0 + b.2 / 2.0;
        let bcy = b.1 + b.3 / 2.0;

        let routed: Vec<(f64, f64)> = routes
            .get(&(e.from.clone(), e.to.clone()))
            .cloned()
            .unwrap_or_default();

        let mut pts: Vec<(f64, f64)> = if routed.len() >= 2 {
            let mut v: Vec<(f64, f64)> = Vec::with_capacity(routed.len());
            v.push((acx, acy));
            for p in &routed[1..routed.len() - 1] {
                v.push(*p);
            }
            v.push((bcx, bcy));
            v
        } else {
            vec![(acx, acy), (bcx, bcy)]
        };

        // Clip endpoints to the rect boundaries pointing toward the next waypoint.
        let first = clip_rect((acx, acy), a.2, a.3, pts[1]);
        let last_idx = pts.len() - 1;
        let last = clip_rect((bcx, bcy), b.2, b.3, pts[last_idx - 1]);
        pts[0] = first;
        pts[last_idx] = last;

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
                theme.flow_edge_stroke
            ),
        );
        if let Some(label) = &e.label {
            if !label.is_empty() {
                let (mx, my) = polyline_midpoint(&pts);
                let lw = (label.chars().count() as f64 * 6.0 + 12.0).max(40.0);
                svg.rect(
                    mx - lw / 2.0,
                    my - 8.0,
                    lw,
                    14.0,
                    &format!("fill=\"{}\" stroke=\"none\" rx=\"2\"", theme.flow_label_bg),
                );
                svg.text(
                    mx,
                    my + 3.0,
                    &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"10\""),
                    label,
                );
            }
        }
        let _ = e.from_side;
        let _ = e.to_side;
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
        svg.rect(
            p.x,
            p.y,
            p.w,
            p.h,
            &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.5\" rx=\"6\""),
        );
        let label_y = if let Some(icon_kind) = &p.icon {
            let ix = p.x + (p.w - ICON_SIZE) / 2.0;
            let iy = p.y + 10.0;
            draw_arch_icon(&mut svg, icon_kind, ix, iy, stroke, fill);
            p.y + ICON_SIZE + 26.0
        } else {
            p.y + p.h / 2.0 + 4.0
        };
        svg.text(
            p.x + p.w / 2.0,
            label_y,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"13\""),
            &p.label,
        );
        svg.text(
            p.x + p.w / 2.0,
            p.y + p.h - 6.0,
            &format!("text-anchor=\"middle\" fill=\"{fg_muted}\" font-size=\"9\""),
            &p.id,
        );
    }

    svg.finish()
}

fn clip_rect(center: (f64, f64), w: f64, h: f64, toward: (f64, f64)) -> (f64, f64) {
    let dx = toward.0 - center.0;
    let dy = toward.1 - center.1;
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return center;
    }
    let hw = w / 2.0;
    let hh = h / 2.0;
    let tx = if dx.abs() < 1e-9 {
        f64::INFINITY
    } else {
        hw / dx.abs()
    };
    let ty = if dy.abs() < 1e-9 {
        f64::INFINITY
    } else {
        hh / dy.abs()
    };
    let t = tx.min(ty);
    (center.0 + dx * t, center.1 + dy * t)
}

fn polyline_path(pts: &[(f64, f64)]) -> String {
    let mut s = String::new();
    for (i, (x, y)) in pts.iter().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        let _ = write!(s, "{cmd}{} {}", fnum(*x), fnum(*y));
    }
    s
}

fn polyline_midpoint(pts: &[(f64, f64)]) -> (f64, f64) {
    if pts.len() < 2 {
        return pts.first().copied().unwrap_or((0.0, 0.0));
    }
    let mut segs = Vec::with_capacity(pts.len() - 1);
    let mut total = 0.0;
    for w in pts.windows(2) {
        let dx = w[1].0 - w[0].0;
        let dy = w[1].1 - w[0].1;
        let l = (dx * dx + dy * dy).sqrt();
        segs.push(l);
        total += l;
    }
    let half = total / 2.0;
    let mut walked = 0.0;
    for (i, w) in pts.windows(2).enumerate() {
        if walked + segs[i] >= half {
            let t = (half - walked) / segs[i].max(1e-9);
            return (
                w[0].0 + t * (w[1].0 - w[0].0),
                w[0].1 + t * (w[1].1 - w[0].1),
            );
        }
        walked += segs[i];
    }
    pts[pts.len() / 2]
}

fn draw_arch_icon(svg: &mut SvgBuilder, kind: &str, x: f64, y: f64, stroke: &str, fill: &str) {
    let s = ICON_SIZE / 32.0;
    let paths: &[&str] = match kind {
        "database" | "db" | "disk" => &[
            "M4 8 C4 4 28 4 28 8 L28 24 C28 28 4 28 4 24 Z",
            "M4 8 C4 12 28 12 28 8",
            "M4 13 C4 17 28 17 28 13",
        ],
        "server" => &[
            "M3 5 H29 V13 H3 Z",
            "M3 16 H29 V24 H3 Z",
            "M6 9 H9 M6 20 H9",
            "M24 9 H26 M24 20 H26",
        ],
        "cloud" => {
            &["M9 24 C4 24 3 17 9 16 C9 11 16 9 18 14 C22 11 27 14 25 18 C30 19 28 24 24 24 Z"]
        }
        "internet" | "globe" => &[
            "M16 4 A12 12 0 1 0 16 28 A12 12 0 1 0 16 4 Z",
            "M4 16 H28",
            "M16 4 C9 11 9 21 16 28",
            "M16 4 C23 11 23 21 16 28",
        ],
        "queue" | "kafka" => &["M4 10 H28 V22 H4 Z", "M10 10 V22 M16 10 V22 M22 10 V22"],
        _ => &["M6 6 H26 V26 H6 Z"],
    };
    let _ = write!(
        svg.body,
        "<g transform=\"translate({x} {y}) scale({s})\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.5\" stroke-linejoin=\"round\" stroke-linecap=\"round\">",
        x = fnum(x),
        y = fnum(y),
        s = fnum(s),
    );
    for p in paths {
        let _ = write!(svg.body, "<path d=\"{p}\"/>");
    }
    svg.raw("</g>");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{ArchEdge, ArchGroup, ArchService, ArchSide};

    #[test]
    fn produces_svg() {
        let d = ArchitectureDiagram {
            groups: vec![ArchGroup {
                id: "api".into(),
                icon: Some("cloud".into()),
                label: Some("API".into()),
                parent: None,
            }],
            services: vec![
                ArchService {
                    id: "db".into(),
                    icon: Some("database".into()),
                    label: Some("DB".into()),
                    parent: Some("api".into()),
                },
                ArchService {
                    id: "disk".into(),
                    icon: Some("disk".into()),
                    label: Some("Disk".into()),
                    parent: Some("api".into()),
                },
            ],
            junctions: vec![],
            edges: vec![ArchEdge {
                from: "db".into(),
                from_side: ArchSide::Left,
                from_arrow: false,
                to: "disk".into(),
                to_side: ArchSide::Right,
                to_arrow: false,
                label: None,
                group: false,
            }],
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">DB<"));
        assert!(svg.contains(">API<"));
    }
}
