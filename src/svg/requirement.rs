//! requirementDiagram renderer. Boxes are placed by sugiyama (layered by
//! relation edges); connectors follow the routed waypoints with rect clipping.

use std::collections::HashMap;
use std::fmt::Write as _;

use crate::parse::{ReqRelationKind, RequirementDiagram, RequirementKind};
use crate::sugiyama::{layout_with, Graph, LayoutConfig, NodeId};

use super::builder::{fnum, SvgBuilder};
use super::theme::Theme;

const PAD: f64 = 30.0;
const BOX_W: f64 = 220.0;
const BOX_H_HEAD: f64 = 36.0;
const ROW_H: f64 = 20.0;

struct Box {
    name: String,
    title_kind: String,
    rows: Vec<(String, String)>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

pub(crate) fn render(d: &RequirementDiagram, theme: &Theme) -> String {
    let fg = theme.fg;
    let stroke = theme.flow_node_stroke;
    let fill = theme.flow_node_fill;

    let mut boxes: Vec<Box> = Vec::new();

    for r in &d.requirements {
        let kind_str = match r.kind {
            RequirementKind::Requirement => "requirement",
            RequirementKind::Functional => "functional",
            RequirementKind::Interface => "interface",
            RequirementKind::Performance => "performance",
            RequirementKind::Physical => "physical",
            RequirementKind::DesignConstraint => "designConstraint",
        };
        let mut rows = Vec::new();
        if let Some(id) = &r.id {
            rows.push(("id".into(), id.clone()));
        }
        if let Some(t) = &r.text {
            rows.push(("text".into(), t.clone()));
        }
        if let Some(t) = &r.risk {
            rows.push(("risk".into(), t.clone()));
        }
        if let Some(t) = &r.verifymethod {
            rows.push(("verify".into(), t.clone()));
        }
        let h = BOX_H_HEAD + rows.len() as f64 * ROW_H;
        boxes.push(Box {
            name: r.name.clone(),
            title_kind: format!("«{kind_str}»"),
            rows,
            x: 0.0,
            y: 0.0,
            w: BOX_W,
            h,
        });
    }
    for e in &d.elements {
        let mut rows = Vec::new();
        if let Some(t) = &e.type_ {
            rows.push(("type".into(), t.clone()));
        }
        if let Some(t) = &e.docref {
            rows.push(("docref".into(), t.clone()));
        }
        let h = BOX_H_HEAD + rows.len() as f64 * ROW_H;
        boxes.push(Box {
            name: e.name.clone(),
            title_kind: "«element»".into(),
            rows,
            x: 0.0,
            y: 0.0,
            w: BOX_W,
            h,
        });
    }

    let name_to_id: HashMap<String, NodeId> = boxes
        .iter()
        .enumerate()
        .map(|(i, b)| (b.name.clone(), i as NodeId))
        .collect();

    let mut g = Graph::default();
    for (i, b) in boxes.iter().enumerate() {
        g.nodes.push(i as NodeId);
        g.node_size.insert(i as NodeId, (b.w, b.h));
    }
    for rel in &d.relations {
        if let (Some(&u), Some(&v)) = (name_to_id.get(&rel.from), name_to_id.get(&rel.to)) {
            g.edges.push((u, v));
        }
    }

    let cfg = LayoutConfig {
        layer_gap: 80.0,
        node_gap: 50.0,
        ..LayoutConfig::default()
    };
    let layout = layout_with(&g, &cfg).unwrap_or_default();

    let origin = (PAD, PAD);
    for (i, b) in boxes.iter_mut().enumerate() {
        let id = i as NodeId;
        if let Some(&(cx, cy)) = layout.node_pos.get(&id) {
            b.x = origin.0 + cx - b.w / 2.0;
            b.y = origin.1 + cy - b.h / 2.0;
        } else {
            b.x = origin.0;
            b.y = origin.1;
        }
    }

    let mut max_x: f64 = 0.0;
    let mut max_y: f64 = 0.0;
    for b in &boxes {
        if b.x + b.w > max_x {
            max_x = b.x + b.w;
        }
        if b.y + b.h > max_y {
            max_y = b.y + b.h;
        }
    }
    let width = max_x + PAD;
    let height = max_y + PAD;

    let mut svg = SvgBuilder::new(width.max(300.0), height.max(120.0));

    svg.defs_raw(&format!(
        "<marker id=\"req-arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" \
         markerWidth=\"9\" markerHeight=\"9\" orient=\"auto-start-reverse\">\
         <path d=\"M0,0 L10,5 L0,10 z\" fill=\"{stroke}\"/></marker>"
    ));

    let by_name: HashMap<String, (f64, f64, f64, f64)> = boxes
        .iter()
        .map(|b| (b.name.clone(), (b.x, b.y, b.w, b.h)))
        .collect();

    for rel in &d.relations {
        let (Some(a), Some(b)) = (by_name.get(&rel.from), by_name.get(&rel.to)) else {
            continue;
        };
        let acx = a.0 + a.2 / 2.0;
        let acy = a.1 + a.3 / 2.0;
        let bcx = b.0 + b.2 / 2.0;
        let bcy = b.1 + b.3 / 2.0;

        let pts: Vec<(f64, f64)> =
            if let (Some(&u), Some(&v)) = (name_to_id.get(&rel.from), name_to_id.get(&rel.to)) {
                layout
                    .edge_points
                    .get(&(u, v))
                    .map(|raw| {
                        let mut v: Vec<(f64, f64)> = Vec::with_capacity(raw.len());
                        v.push((acx, acy));
                        for (px, py) in &raw[1..raw.len().saturating_sub(1)] {
                            v.push((origin.0 + *px, origin.1 + *py));
                        }
                        v.push((bcx, bcy));
                        v
                    })
                    .unwrap_or_else(|| vec![(acx, acy), (bcx, bcy)])
            } else {
                vec![(acx, acy), (bcx, bcy)]
            };

        let first = clip_rect_to_edge(pts[1], (acx, acy), a.2, a.3);
        let last_idx = pts.len() - 1;
        let last = clip_rect_to_edge(pts[last_idx - 1], (bcx, bcy), b.2, b.3);
        let mut clipped: Vec<(f64, f64)> = Vec::with_capacity(pts.len());
        clipped.push(first);
        for p in &pts[1..last_idx] {
            clipped.push(*p);
        }
        clipped.push(last);

        let label = match rel.kind {
            ReqRelationKind::Contains => "«contains»",
            ReqRelationKind::Copies => "«copies»",
            ReqRelationKind::Derives => "«derives»",
            ReqRelationKind::Satisfies => "«satisfies»",
            ReqRelationKind::Verifies => "«verifies»",
            ReqRelationKind::Refines => "«refines»",
            ReqRelationKind::Traces => "«traces»",
        };
        let dashed = matches!(
            rel.kind,
            ReqRelationKind::Satisfies
                | ReqRelationKind::Verifies
                | ReqRelationKind::Refines
                | ReqRelationKind::Traces
        );
        let dash_attr = if dashed {
            " stroke-dasharray=\"5 3\""
        } else {
            ""
        };
        svg.path(
            &polyline_path(&clipped),
            &format!(
                "fill=\"none\" stroke=\"{stroke}\" stroke-width=\"1.5\"{dash_attr} marker-end=\"url(#req-arrow)\""
            ),
        );

        let (mx, my) = polyline_midpoint(&clipped);
        let lw = (label.chars().count() as f64 * 5.5 + 14.0).max(60.0);
        svg.rect(
            mx - lw / 2.0,
            my - 9.0,
            lw,
            16.0,
            &format!(
                "fill=\"{}\" stroke=\"{stroke}\" stroke-width=\"0.5\" rx=\"3\"",
                theme.flow_label_bg
            ),
        );
        svg.text(
            mx,
            my + 3.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"10\""),
            label,
        );
    }

    for b in &boxes {
        svg.rect(
            b.x,
            b.y,
            b.w,
            b.h,
            &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.5\""),
        );
        svg.text(
            b.x + b.w / 2.0,
            b.y + 16.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"11\" font-style=\"italic\""),
            &b.title_kind,
        );
        svg.text(
            b.x + b.w / 2.0,
            b.y + 30.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"13\" font-weight=\"bold\""),
            &b.name,
        );
        svg.line(
            b.x,
            b.y + BOX_H_HEAD,
            b.x + b.w,
            b.y + BOX_H_HEAD,
            &format!("stroke=\"{stroke}\" stroke-width=\"1\""),
        );
        for (i, (k, v)) in b.rows.iter().enumerate() {
            let ry = b.y + BOX_H_HEAD + i as f64 * ROW_H + 14.0;
            svg.text(
                b.x + 8.0,
                ry,
                &format!("fill=\"{fg}\" font-size=\"11\" font-weight=\"bold\""),
                k,
            );
            svg.text(
                b.x + 70.0,
                ry,
                &format!("fill=\"{fg}\" font-size=\"11\""),
                &truncate(v, 22),
            );
        }
    }

    svg.finish()
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

fn clip_rect_to_edge(from: (f64, f64), center: (f64, f64), w: f64, h: f64) -> (f64, f64) {
    let dx = from.0 - center.0;
    let dy = from.1 - center.1;
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

fn truncate(s: &str, n: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= n {
        s.to_string()
    } else {
        let mut out: String = chars[..n.saturating_sub(1)].iter().collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{ReqElement, ReqRelation, Requirement};

    #[test]
    fn produces_svg() {
        let d = RequirementDiagram {
            requirements: vec![Requirement {
                kind: RequirementKind::Requirement,
                name: "req1".into(),
                id: Some("1".into()),
                text: Some("the req".into()),
                risk: None,
                verifymethod: None,
            }],
            elements: vec![ReqElement {
                name: "e1".into(),
                type_: Some("sim".into()),
                docref: None,
            }],
            relations: vec![ReqRelation {
                from: "e1".into(),
                to: "req1".into(),
                kind: ReqRelationKind::Satisfies,
            }],
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">req1<"));
        assert!(svg.contains(">e1<"));
        assert!(svg.contains("req-arrow"));
    }
}
