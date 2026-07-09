//! requirementDiagram renderer. Boxes are placed by sugiyama (layered by
//! relation edges); connectors follow the routed waypoints with rect clipping.

use std::collections::HashMap;

use crate::parse::ast::FlowDirection;
use crate::parse::{ReqRelationKind, RequirementDiagram, RequirementKind};
use crate::sugiyama::{layout_with, Graph, LayoutConfig, NodeId};

use super::builder::{curve_basis_path, SvgBuilder};
use super::geometry::{clip_rect, polyline_midpoint};
use super::style::resolve_style;
use super::theme::Theme;

const PAD: f64 = 30.0;
const BOX_W: f64 = 220.0;
const BOX_H_HEAD: f64 = 36.0;
const ROW_H: f64 = 20.0;

struct Box {
    name: String,
    title_kind: String,
    rows: Vec<String>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// Upstream (Mermaid 11.x) title-cased type name shown inside the `<<…>>`
/// header (see [`angle_wrap`]).
fn requirement_title(kind: RequirementKind) -> &'static str {
    match kind {
        RequirementKind::Requirement => "Requirement",
        RequirementKind::Functional => "Functional Requirement",
        RequirementKind::Interface => "Interface Requirement",
        RequirementKind::Performance => "Performance Requirement",
        RequirementKind::Physical => "Physical Requirement",
        RequirementKind::DesignConstraint => "Design Constraint",
    }
}

/// Wrap a stereotype in upstream's `<<…>>` guillemet form. Angle brackets are
/// emitted as `#lt;`/`#gt;` entity codes so the inline-HTML parser doesn't
/// mistake `<Requirement>` for a tag and strip it; they decode back to `<`/`>`
/// after tag scanning.
fn angle_wrap(inner: &str) -> String {
    format!("#lt;#lt;{inner}#gt;#gt;")
}

/// Title-case the leading letter of an enum-valued field (`high` → `High`,
/// `test` → `Test`) to match upstream's display values.
fn title_value(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

pub(crate) fn render(d: &RequirementDiagram, theme: &Theme) -> String {
    let fg = &theme.fg;
    let stroke = &theme.flow_node_stroke;
    let fill = &theme.flow_node_fill;
    // Relations follow upstream's thin dark-gray edge color, not the purple
    // node stroke — `contains` circle and the plain arrowhead included.
    let edge = &theme.flow_edge_stroke;

    let mut boxes: Vec<Box> = Vec::new();

    for r in &d.requirements {
        let mut rows = Vec::new();
        if let Some(id) = &r.id {
            rows.push(format!("ID: {id}"));
        }
        if let Some(t) = &r.text {
            rows.push(format!("Text: {t}"));
        }
        if let Some(t) = &r.risk {
            rows.push(format!("Risk: {}", title_value(t)));
        }
        if let Some(t) = &r.verifymethod {
            rows.push(format!("Verification: {}", title_value(t)));
        }
        let h = BOX_H_HEAD + rows.len() as f64 * ROW_H;
        boxes.push(Box {
            name: r.name.clone(),
            title_kind: angle_wrap(requirement_title(r.kind)),
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
            rows.push(format!("Type: {t}"));
        }
        if let Some(t) = &e.docref {
            rows.push(format!("Doc Ref: {t}"));
        }
        let h = BOX_H_HEAD + rows.len() as f64 * ROW_H;
        boxes.push(Box {
            name: e.name.clone(),
            title_kind: angle_wrap("Element"),
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

    // For LR/RL the sugiyama layout still runs top-down; we swap node sizes
    // going in and transpose coordinates coming out (same trick as flowchart).
    let dir = d.direction;
    let horizontal = matches!(dir, FlowDirection::LeftRight | FlowDirection::RightLeft);

    let mut g = Graph::default();
    for (i, b) in boxes.iter().enumerate() {
        g.nodes.push(i as NodeId);
        let size = if horizontal { (b.h, b.w) } else { (b.w, b.h) };
        g.node_size.insert(i as NodeId, size);
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
    let raw_h = layout.height;

    let origin = (PAD, PAD);
    let transform = move |(sx, sy): (f64, f64)| -> (f64, f64) {
        let (tx, ty) = match dir {
            FlowDirection::TopDown => (sx, sy),
            FlowDirection::BottomTop => (sx, raw_h - sy),
            FlowDirection::LeftRight => (sy, sx),
            FlowDirection::RightLeft => (raw_h - sy, sx),
        };
        (origin.0 + tx, origin.1 + ty)
    };

    for (i, b) in boxes.iter_mut().enumerate() {
        let id = i as NodeId;
        if let Some(&pos) = layout.node_pos.get(&id) {
            let (cx, cy) = transform(pos);
            b.x = cx - b.w / 2.0;
            b.y = cy - b.h / 2.0;
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

    let mut svg = SvgBuilder::new(width.max(300.0), height.max(120.0)).theme(theme);

    // Upstream's relation arrowhead is a small *open* (line) chevron, not a
    // filled triangle: a stroked `M…L…L…` V with no `fill` and no closing `z`.
    svg.defs_raw(&format!(
        "<marker id=\"req-arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" \
         markerWidth=\"8\" markerHeight=\"8\" orient=\"auto-start-reverse\">\
         <path d=\"M1,1 L9,5 L1,9\" fill=\"none\" stroke=\"{edge}\" stroke-width=\"1.5\"/></marker>"
    ));
    svg.defs_raw(&format!(
        "<marker id=\"req-contains\" viewBox=\"0 0 20 20\" refX=\"19\" refY=\"10\" \
         markerWidth=\"18\" markerHeight=\"18\" orient=\"auto-start-reverse\">\
         <circle cx=\"10\" cy=\"10\" r=\"9\" fill=\"{fill}\" stroke=\"{edge}\" stroke-width=\"1\"/>\
         <path d=\"M1,10 L19,10 M10,1 L10,19\" stroke=\"{edge}\" stroke-width=\"1\"/></marker>"
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
                        for &p in &raw[1..raw.len().saturating_sub(1)] {
                            v.push(transform(p));
                        }
                        v.push((bcx, bcy));
                        v
                    })
                    .unwrap_or_else(|| vec![(acx, acy), (bcx, bcy)])
            } else {
                vec![(acx, acy), (bcx, bcy)]
            };

        // Router always returns both endpoints; guard the invariant so a
        // regression skips the edge instead of underflowing `pts.len() - 1`.
        if pts.len() < 2 {
            continue;
        }
        let first = clip_rect(pts[1], (acx, acy), (a.2, a.3));
        let last_idx = pts.len() - 1;
        let last = clip_rect(pts[last_idx - 1], (bcx, bcy), (b.2, b.3));
        let mut clipped: Vec<(f64, f64)> = Vec::with_capacity(pts.len());
        clipped.push(first);
        for p in &pts[1..last_idx] {
            clipped.push(*p);
        }
        clipped.push(last);

        let kind_word = match rel.kind {
            ReqRelationKind::Contains => "contains",
            ReqRelationKind::Copies => "copies",
            ReqRelationKind::Derives => "derives",
            ReqRelationKind::Satisfies => "satisfies",
            ReqRelationKind::Verifies => "verifies",
            ReqRelationKind::Refines => "refines",
            ReqRelationKind::Traces => "traces",
        };
        let label = angle_wrap(kind_word);
        // Upstream draws only `contains` solid (crossed-circle head); every
        // other relation kind (copies/derives/satisfies/verifies/refines/
        // traces) is dashed with a thin arrowhead.
        let dashed = !matches!(rel.kind, ReqRelationKind::Contains);
        let dash_attr = if dashed {
            " stroke-dasharray=\"5 3\""
        } else {
            ""
        };
        // `contains` draws upstream's crossed-circle containment head at the
        // *container* (`from`) end — a `marker-start` — so the glyph sits on the
        // container's box edge regardless of which syntactic direction
        // (`-> contains` or `<- contains`) the parser recorded. Every other
        // relation puts a plain arrowhead at the `to` end (`marker-end`).
        let marker_attr = if rel.kind == ReqRelationKind::Contains {
            "marker-start=\"url(#req-contains)\"".to_string()
        } else {
            "marker-end=\"url(#req-arrow)\"".to_string()
        };
        svg.path(
            &curve_basis_path(&clipped),
            &format!(
                "fill=\"none\" stroke=\"{edge}\" stroke-width=\"1.5\"{dash_attr} {marker_attr}"
            ),
        );

        let (mx, my) = polyline_midpoint(&clipped);
        // Width from the displayed glyphs (`<<kind>>`), not the entity-coded
        // form that carries the `#lt;`/`#gt;` sentinels.
        let label_w = super::metrics::text_width(&format!("<<{kind_word}>>"), 5.5, theme.font_size);
        let lw = (label_w + 14.0).max(60.0);
        // Upstream draws the relation label on a light-gray background patch
        // (`.relationshipLabelBox`, rgb(232,232,232) at 0.85 opacity), no
        // border/pill, that just masks the line beneath it.
        svg.rect(
            mx - lw / 2.0,
            my - 9.0,
            lw,
            16.0,
            "fill=\"#e8e8e8\" fill-opacity=\"0.85\"",
        );
        svg.text(
            mx,
            my + 3.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"10\""),
            &label,
        );
    }

    let no_classes: Vec<String> = Vec::new();
    let no_style = crate::parse::ast::Style::new();
    for b in &boxes {
        let classes = d.node_classes.get(&b.name).unwrap_or(&no_classes);
        let inline = d.node_styles.get(&b.name).unwrap_or(&no_style);
        let rs = resolve_style(&d.class_defs, classes, inline);
        let box_stroke = rs.stroke_or(stroke).to_string();
        let text_fill = rs.label_fill(fg).to_string();
        svg.rect(b.x, b.y, b.w, b.h, &rs.shape_attrs(fill, stroke, "1.5"));
        svg.text(
            b.x + b.w / 2.0,
            b.y + 16.0,
            &format!("text-anchor=\"middle\" fill=\"{text_fill}\" font-size=\"11\""),
            &b.title_kind,
        );
        svg.text(
            b.x + b.w / 2.0,
            b.y + 30.0,
            &format!(
                "text-anchor=\"middle\" fill=\"{text_fill}\" font-size=\"13\" font-weight=\"bold\""
            ),
            &b.name,
        );
        svg.line(
            b.x,
            b.y + BOX_H_HEAD,
            b.x + b.w,
            b.y + BOX_H_HEAD,
            &format!("stroke=\"{box_stroke}\" stroke-width=\"1\""),
        );
        for (i, line) in b.rows.iter().enumerate() {
            let ry = b.y + BOX_H_HEAD + i as f64 * ROW_H + 14.0;
            svg.text(
                b.x + 8.0,
                ry,
                &format!("fill=\"{text_fill}\" font-size=\"11\""),
                &truncate(line, 34),
            );
        }
    }

    svg.finish()
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
mod tests;
