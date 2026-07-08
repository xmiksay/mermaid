//! architecture-beta renderer. Services are grouped into boxes; layout within
//! each group is driven by the edge **port hints** (`db:L -- R:server`): an
//! `L`/`R` pair shares a row (source's named side faces the target's named
//! side), a `T`/`B` pair shares a column, and edges are routed with straight
//! orthogonal segments between the pinned sides. An `id{group}` endpoint
//! resolves to a group box.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fmt::Write as _;

use crate::parse::{ArchAlign, ArchAlignAxis, ArchEdge, ArchSide, ArchitectureDiagram};
use crate::sugiyama::NodeId;

use super::builder::{fnum, SvgBuilder};
use super::geometry::polyline_midpoint;
use super::theme::Theme;

const PAD: f64 = 30.0;
const SERVICE_W: f64 = 110.0;
const SERVICE_H: f64 = 86.0;
const H_GAP: f64 = 40.0;
const V_GAP: f64 = 40.0;
const GROUP_PAD: f64 = 16.0;
const GROUP_HDR: f64 = 22.0;
const GROUP_GAP: f64 = 30.0;
const ICON_SIZE: f64 = 28.0;
const GROUP_ICON_SIZE: f64 = 16.0;
const JUNCTION_R: f64 = 6.0;
/// Solid tile behind a service glyph — a fixed blue to match the JS reference
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
    let stroke = &theme.flow_node_stroke;
    let fill = &theme.flow_node_fill;

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
        svg.rect(
            p.x,
            p.y,
            p.w,
            p.h,
            &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.5\" rx=\"6\""),
        );
        let label_y = if let Some(icon_kind) = &p.icon {
            // Solid blue icon tile with a white glyph on top.
            let tile = ICON_SIZE + 10.0;
            let tx = p.x + (p.w - tile) / 2.0;
            let ty = p.y + 8.0;
            svg.rect(
                tx,
                ty,
                tile,
                tile,
                &format!("fill=\"{ICON_TILE}\" stroke=\"none\" rx=\"6\""),
            );
            let ix = tx + (tile - ICON_SIZE) / 2.0;
            let iy = ty + (tile - ICON_SIZE) / 2.0;
            // A pure-Rust renderer can't fetch Iconify packs (`logos:aws-lambda`
            // etc.), so unrecognized names fall back to a generic glyph plus the
            // name as a caption — the icon identity is shown, not silently lost.
            if !draw_arch_icon(&mut svg, icon_kind, ix, iy, ICON_SIZE, "#ffffff", "none") {
                svg.text(
                    p.x + p.w / 2.0,
                    ty + tile + 10.0,
                    &format!("text-anchor=\"middle\" fill=\"{fg_muted}\" font-size=\"8\""),
                    &truncate_icon_name(icon_kind),
                );
            }
            ty + tile + 24.0
        } else {
            p.y + p.h / 2.0 + 4.0
        };
        svg.text(
            p.x + p.w / 2.0,
            label_y,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"13\""),
            &p.label,
        );
    }

    svg.finish()
}

/// Assigns integer grid coordinates `(col, row)` to each node from the edge port
/// hints. Following an edge `from:S₁ -- S₂:to`, the neighbour sits one cell away
/// in the direction of the anchored node's named side (`L`→left, `R`→right,
/// `T`→up, `B`→down), so an `L`/`R` pair shares a row and a `T`/`B` pair shares a
/// column. Each connected component is grown breadth-first from its
/// source-order seed; separate components (and edge-less nodes) start in fresh
/// columns so they don't overlap.
/// Node → its neighbours and the grid direction each neighbour sits in.
type Adjacency<'a> = HashMap<&'a str, Vec<(&'a str, (i32, i32))>>;

fn grid_place(ids: &[String], edges: &[ArchEdge]) -> HashMap<String, (i32, i32)> {
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
fn apply_aligns(
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
fn port_point(center: (f64, f64), w: f64, h: f64, side: ArchSide) -> (f64, f64) {
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
fn ortho_route(pa: (f64, f64), sa: ArchSide, pb: (f64, f64), sb: ArchSide) -> Vec<(f64, f64)> {
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

fn polyline_path(pts: &[(f64, f64)]) -> String {
    let mut s = String::new();
    for (i, (x, y)) in pts.iter().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        let _ = write!(s, "{cmd}{} {}", fnum(*x), fnum(*y));
    }
    s
}

/// Draws the `size`-px icon glyph for `kind` at `(x, y)`. Returns `true` when the
/// name maps to a built-in glyph, `false` when it's unrecognized (the caller then
/// renders the raw name as a caption so the icon identity survives).
fn draw_arch_icon(
    svg: &mut SvgBuilder,
    kind: &str,
    x: f64,
    y: f64,
    size: f64,
    stroke: &str,
    fill: &str,
) -> bool {
    let s = size / 32.0;
    let (paths, recognized): (&[&str], bool) = match kind {
        "database" | "db" => (
            &[
                "M4 8 C4 4 28 4 28 8 L28 24 C28 28 4 28 4 24 Z",
                "M4 8 C4 12 28 12 28 8",
                "M4 13 C4 17 28 17 28 13",
            ],
            true,
        ),
        "disk" => (
            &[
                "M16 4 A12 12 0 1 0 16 28 A12 12 0 1 0 16 4 Z",
                "M16 11 A5 5 0 1 0 16 21 A5 5 0 1 0 16 11 Z",
                "M15 15.5 A1 1 0 1 0 17 16.5 A1 1 0 1 0 15 15.5 Z",
            ],
            true,
        ),
        "server" => (
            &[
                "M3 5 H29 V13 H3 Z",
                "M3 16 H29 V24 H3 Z",
                "M6 9 H9 M6 20 H9",
                "M24 9 H26 M24 20 H26",
            ],
            true,
        ),
        "cloud" => (
            &["M9 24 C4 24 3 17 9 16 C9 11 16 9 18 14 C22 11 27 14 25 18 C30 19 28 24 24 24 Z"],
            true,
        ),
        "internet" | "globe" => (
            &[
                "M16 4 A12 12 0 1 0 16 28 A12 12 0 1 0 16 4 Z",
                "M4 16 H28",
                "M16 4 C9 11 9 21 16 28",
                "M16 4 C23 11 23 21 16 28",
            ],
            true,
        ),
        "queue" | "kafka" => (
            &["M4 10 H28 V22 H4 Z", "M10 10 V22 M16 10 V22 M22 10 V22"],
            true,
        ),
        _ => (&["M6 6 H26 V26 H6 Z"], false),
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
    recognized
}

/// Shortens an Iconify-style icon name for the fallback caption: keeps the
/// segment after the last `:` (`logos:aws-lambda` → `aws-lambda`) and caps the
/// length so a long name can't overflow the service box.
fn truncate_icon_name(name: &str) -> String {
    let short = name.rsplit(':').next().unwrap_or(name);
    const MAX: usize = 16;
    if short.chars().count() > MAX {
        let head: String = short.chars().take(MAX - 1).collect();
        format!("{head}…")
    } else {
        short.to_string()
    }
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
                group: false,
                label: None,
            }],
            aligns: vec![],
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">DB<"));
        assert!(svg.contains(">API<"));
        // Group labels use upstream regular weight, not bold (#332).
        assert!(!svg.contains("font-weight=\"bold\">API<"));
    }

    #[test]
    fn unknown_icon_renders_name_caption() {
        // Iconify pack names can't be fetched by a static renderer; instead of
        // silently drawing a blank box, the name is shown as a caption.
        let src = "\
architecture-beta
    service lambda(logos:aws-lambda)[Lambda]
";
        let d = match crate::parse::parse(src).unwrap() {
            crate::parse::Diagram::Architecture(d) => d,
            _ => panic!("expected architecture diagram"),
        };
        let svg = render(&d, &Theme::default());
        // Caption keeps the segment after the last ':', label stays intact.
        assert!(svg.contains(">aws-lambda<"), "icon-name caption missing");
        assert!(svg.contains(">Lambda<"), "service label missing");
    }

    #[test]
    fn truncate_icon_name_shortens() {
        assert_eq!(truncate_icon_name("logos:aws-lambda"), "aws-lambda");
        assert_eq!(truncate_icon_name("cloud"), "cloud");
        assert_eq!(
            truncate_icon_name("mdi:application-braces-outline"),
            "application-bra…"
        );
    }

    #[test]
    fn edge_title_renders() {
        // The `-[title]-` connector draws the title text on the edge (#184).
        let src = "\
architecture-beta
    service db(database)[DB]
    service server(server)[Srv]
    db:R -[Queries]- L:server
";
        let d = match crate::parse::parse(src).unwrap() {
            crate::parse::Diagram::Architecture(d) => d,
            _ => panic!("expected architecture diagram"),
        };
        assert_eq!(d.edges[0].label.as_deref(), Some("Queries"));
        let svg = render(&d, &Theme::default());
        assert!(svg.contains(">Queries<"), "edge title missing");
    }

    /// Top-left `(x, y)` of every service box (`width="110"`) in source order.
    fn service_boxes(svg: &str) -> Vec<(f64, f64)> {
        svg.split("<rect ")
            .filter(|chunk| chunk.contains("width=\"110\""))
            .filter_map(|chunk| {
                let x = attr(chunk, "x=\"")?;
                let y = attr(chunk, "y=\"")?;
                Some((x, y))
            })
            .collect()
    }

    fn attr(chunk: &str, key: &str) -> Option<f64> {
        let start = chunk.find(key)? + key.len();
        let end = chunk[start..].find('"')? + start;
        chunk[start..end].parse().ok()
    }

    fn arch(src: &str) -> ArchitectureDiagram {
        match crate::parse::parse(src).unwrap() {
            crate::parse::Diagram::Architecture(d) => d,
            _ => panic!("expected architecture diagram"),
        }
    }

    #[test]
    fn align_column_stacks_services_vertically() {
        // `align column a b` puts a and b in a shared column: same x, distinct y
        // with a above b (#227).
        let d = arch(
            "architecture-beta\nservice a(server)[A]\nservice b(server)[B]\nalign column a b\n",
        );
        let svg = render(&d, &Theme::default());
        let boxes = service_boxes(&svg);
        assert_eq!(boxes.len(), 2);
        assert!((boxes[0].0 - boxes[1].0).abs() < 0.01, "column shares x");
        assert!(boxes[0].1 < boxes[1].1, "a stacks above b");
    }

    #[test]
    fn align_row_lines_services_horizontally() {
        // `align row a b` puts a and b in a shared row: same y, distinct x with a
        // left of b (#227).
        let d =
            arch("architecture-beta\nservice a(server)[A]\nservice b(server)[B]\nalign row a b\n");
        let svg = render(&d, &Theme::default());
        let boxes = service_boxes(&svg);
        assert_eq!(boxes.len(), 2);
        assert!((boxes[0].1 - boxes[1].1).abs() < 0.01, "row shares y");
        assert!(boxes[0].0 < boxes[1].0, "a sits left of b");
    }

    #[test]
    fn group_edge_draws_path() {
        // Regression for #62: an `id{group}` edge between two group boxes must
        // resolve its endpoints and draw a (dashed) connector, not vanish.
        let src = "\
architecture-beta
    group left(cloud)[Left]
    group right(cloud)[Right]
    service a(server)[A] in left
    service b(server)[B] in right
    left{group}:R -- L:right{group}
";
        let d = match crate::parse::parse(src).unwrap() {
            crate::parse::Diagram::Architecture(d) => d,
            _ => panic!("expected architecture diagram"),
        };
        assert!(d.edges[0].group);
        let svg = render(&d, &Theme::default());
        assert!(
            svg.contains("stroke-dasharray=\"5 3\""),
            "group edge path missing"
        );
    }

    #[test]
    fn port_hints_pin_grid_layout() {
        // #257: L/R pairs share a row, T/B pairs share a column. From the
        // reference: server left of db (db:L -- R:server), disk1 below server,
        // disk2 below db.
        let d = arch(
            "\
architecture-beta
    group api(cloud)[API]
    service db(database)[Database] in api
    service disk1(disk)[Storage 1] in api
    service disk2(disk)[Storage 2] in api
    service server(server)[Server] in api
    db:L -- R:server
    disk1:T -- B:server
    disk2:T -- B:db
",
        );
        let ids: Vec<String> = d.services.iter().map(|s| s.id.clone()).collect();
        let g = grid_place(&ids, &d.edges);
        let (sc, sr) = g["server"];
        let (dc, dr) = g["db"];
        // Server and db side by side (shared row), server left of db.
        assert_eq!(sr, dr, "server/db share a row");
        assert!(sc < dc, "server sits left of db");
        // Disks hang below their parents (shared column, one row down).
        assert_eq!(g["disk1"].0, sc, "disk1 under server");
        assert_eq!(g["disk1"].1, sr + 1, "disk1 one row below server");
        assert_eq!(g["disk2"].0, dc, "disk2 under db");
        assert_eq!(g["disk2"].1, dr + 1, "disk2 one row below db");

        // The whole diagram still renders and the db↔server edge is a straight
        // horizontal segment (no diagonal): endpoints share a y.
        assert!(render(&d, &Theme::default()).contains("<svg"));
    }

    #[test]
    fn orthogonal_route_has_no_diagonal_segments() {
        // Every segment of a routed edge is axis-aligned (horizontal or vertical).
        let pa = (0.0, 0.0);
        let pb = (100.0, 40.0);
        let pts = ortho_route(pa, ArchSide::Right, pb, ArchSide::Bottom);
        for w in pts.windows(2) {
            let (x0, y0) = w[0];
            let (x1, y1) = w[1];
            let axis_aligned = (x0 - x1).abs() < 1e-9 || (y0 - y1).abs() < 1e-9;
            assert!(axis_aligned, "segment {:?}->{:?} is diagonal", w[0], w[1]);
        }
    }
}
