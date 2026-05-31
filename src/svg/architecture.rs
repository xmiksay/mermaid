//! architecture-beta renderer. Services are grouped into boxes; edges drawn
//! between service edges (T/B/L/R sides).

use std::collections::BTreeMap;

use crate::parse::{ArchService, ArchSide, ArchitectureDiagram};

use super::builder::SvgBuilder;
use super::theme::Theme;

const PAD: f64 = 30.0;
const SERVICE_W: f64 = 110.0;
const SERVICE_H: f64 = 70.0;
const GAP: f64 = 24.0;
const GROUP_PAD: f64 = 16.0;
const COLS: usize = 3;

pub(crate) fn render(d: &ArchitectureDiagram, theme: &Theme) -> String {
    let fg = theme.fg;
    let fg_muted = theme.fg_muted;
    let stroke = theme.flow_node_stroke;
    let fill = theme.flow_node_fill;

    // Group services by parent.
    let mut by_parent: BTreeMap<Option<String>, Vec<&ArchService>> = BTreeMap::new();
    for s in &d.services {
        by_parent.entry(s.parent.clone()).or_default().push(s);
    }
    for j in &d.junctions {
        by_parent.entry(j.parent.clone()).or_default(); // ensure group entry
    }

    // Place groups: each group is a separate column block stacked vertically.
    let mut group_y = PAD;
    let mut max_w: f64 = 0.0;
    let mut centers: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    let mut svg = SvgBuilder::new(0.0, 0.0); // placeholder, will reset

    // Buffer drawing instructions: we don't know size yet, do two-pass layout.
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
    let mut placed_groups: Vec<(f64, f64, f64, f64, String)> = Vec::new();
    let mut placed_services: Vec<Placed> = Vec::new();

    // Top-level groups first.
    let group_order: Vec<Option<String>> = {
        let mut v: Vec<Option<String>> = Vec::new();
        v.push(None);
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

    for parent in &group_order {
        let services = by_parent.get(parent).cloned().unwrap_or_default();
        let group_def = parent
            .as_ref()
            .and_then(|p| d.groups.iter().find(|g| &g.id == p));
        let label_h = if group_def.is_some() { 22.0 } else { 0.0 };

        // Cell layout
        let cols = COLS.min(services.len().max(1));
        let rows = (services.len() + cols - 1) / cols.max(1);
        let inner_w = cols as f64 * SERVICE_W + (cols.saturating_sub(1) as f64) * GAP;
        let inner_h = rows as f64 * SERVICE_H + (rows.saturating_sub(1) as f64) * GAP;
        let box_w = inner_w + GROUP_PAD * 2.0;
        let box_h = inner_h + GROUP_PAD * 2.0 + label_h;
        let box_x = PAD;
        let box_y = group_y;

        if let Some(g) = group_def {
            placed_groups.push((
                box_x,
                box_y,
                box_w,
                box_h,
                g.label.clone().unwrap_or_else(|| g.id.clone()),
            ));
        }

        for (i, s) in services.iter().enumerate() {
            let col = i % cols;
            let row = i / cols;
            let x = box_x + GROUP_PAD + col as f64 * (SERVICE_W + GAP);
            let y = box_y + GROUP_PAD + label_h + row as f64 * (SERVICE_H + GAP);
            placed_services.push(Placed {
                x,
                y,
                w: SERVICE_W,
                h: SERVICE_H,
                label: s.label.clone().unwrap_or_else(|| s.id.clone()),
                id: s.id.clone(),
                icon: s.icon.clone(),
                is_junction: false,
            });
            centers.insert(s.id.clone(), (x + SERVICE_W / 2.0, y + SERVICE_H / 2.0));
        }

        max_w = max_w.max(box_w);
        group_y += box_h + GAP;
    }

    // Junctions floating at top-level.
    for j in &d.junctions {
        if j.parent.is_some() {
            continue;
        }
        let x = PAD + max_w + GAP;
        let y = PAD + (placed_services.len() as f64 * 8.0).min(80.0);
        centers.insert(j.id.clone(), (x + 8.0, y + 8.0));
        placed_services.push(Placed {
            x,
            y,
            w: 16.0,
            h: 16.0,
            label: String::new(),
            id: j.id.clone(),
            icon: None,
            is_junction: true,
        });
        max_w = (max_w + 60.0).max(max_w);
    }

    let width = PAD * 2.0 + max_w;
    let height = PAD + group_y.max(160.0);
    svg.width = width.max(300.0);
    svg.height = height;

    // Group boxes.
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

    // Edges (drawn first so service rects sit on top of edges visually for junctions).
    for e in &d.edges {
        let (Some(a), Some(b)) = (centers.get(&e.from), centers.get(&e.to)) else {
            continue;
        };
        let ax = side_x(a, e.from_side);
        let bx = side_x(b, e.to_side);
        svg.line(
            ax.0,
            ax.1,
            bx.0,
            bx.1,
            &format!("stroke=\"{}\" stroke-width=\"1.5\"", theme.flow_edge_stroke),
        );
    }

    // Services.
    for p in &placed_services {
        if p.is_junction {
            svg.circle(
                p.x + p.w / 2.0,
                p.y + p.h / 2.0,
                6.0,
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
        if let Some(icon) = &p.icon {
            svg.text(
                p.x + p.w / 2.0,
                p.y + 18.0,
                &format!(
                    "text-anchor=\"middle\" fill=\"{fg}\" font-size=\"10\" font-style=\"italic\""
                ),
                &format!("[{icon}]"),
            );
        }
        svg.text(
            p.x + p.w / 2.0,
            p.y + p.h / 2.0 + 4.0,
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

fn side_x(c: &(f64, f64), side: ArchSide) -> (f64, f64) {
    let dx = SERVICE_W / 2.0;
    let dy = SERVICE_H / 2.0;
    match side {
        ArchSide::Top => (c.0, c.1 - dy),
        ArchSide::Bottom => (c.0, c.1 + dy),
        ArchSide::Left => (c.0 - dx, c.1),
        ArchSide::Right => (c.0 + dx, c.1),
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
