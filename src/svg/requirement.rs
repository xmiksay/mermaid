//! requirementDiagram renderer. Tabular boxes with sections.

use std::collections::BTreeMap;

use crate::parse::{ReqRelationKind, RequirementDiagram, RequirementKind};

use super::builder::SvgBuilder;
use super::theme::Theme;

const PAD: f64 = 30.0;
const BOX_W: f64 = 220.0;
const BOX_H_HEAD: f64 = 36.0;
const ROW_H: f64 = 20.0;
const COL_GAP: f64 = 80.0;
const ROW_GAP: f64 = 30.0;
const COLS: usize = 3;

pub(crate) fn render(d: &RequirementDiagram, theme: &Theme) -> String {
    let fg = theme.fg;
    let stroke = theme.flow_node_stroke;
    let fill = theme.flow_node_fill;

    // Lay out requirements then elements left-to-right in a grid.
    struct Box {
        name: String,
        title_kind: String,
        rows: Vec<(String, String)>,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    }

    let mut boxes: Vec<Box> = Vec::new();
    let mut idx = 0;

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
        let col = idx % COLS;
        let row = idx / COLS;
        let x = PAD + col as f64 * (BOX_W + COL_GAP);
        let y = PAD + row as f64 * (h + ROW_GAP) + 30.0;
        boxes.push(Box {
            name: r.name.clone(),
            title_kind: format!("«{kind_str}»"),
            rows,
            x,
            y,
            w: BOX_W,
            h,
        });
        idx += 1;
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
        let col = idx % COLS;
        let row = idx / COLS;
        let x = PAD + col as f64 * (BOX_W + COL_GAP);
        let y = PAD + row as f64 * (h + ROW_GAP) + 30.0;
        boxes.push(Box {
            name: e.name.clone(),
            title_kind: "«element»".into(),
            rows,
            x,
            y,
            w: BOX_W,
            h,
        });
        idx += 1;
    }

    let total_rows = (boxes.len() + COLS - 1) / COLS.max(1);
    let max_box_h = boxes.iter().map(|b| b.h as i64).max().unwrap_or(0) as f64;
    let width = PAD * 2.0 + COLS as f64 * BOX_W + (COLS.saturating_sub(1) as f64) * COL_GAP;
    let height = PAD * 2.0 + total_rows.max(1) as f64 * (max_box_h.max(60.0) + ROW_GAP) + 30.0;

    let mut svg = SvgBuilder::new(width, height);

    let by_name: BTreeMap<String, (f64, f64)> = boxes
        .iter()
        .map(|b| (b.name.clone(), (b.x + b.w / 2.0, b.y + b.h / 2.0)))
        .collect();

    // Relations.
    for rel in &d.relations {
        let (Some(a), Some(b)) = (by_name.get(&rel.from), by_name.get(&rel.to)) else {
            continue;
        };
        let label = match rel.kind {
            ReqRelationKind::Contains => "«contains»",
            ReqRelationKind::Copies => "«copies»",
            ReqRelationKind::Derives => "«derives»",
            ReqRelationKind::Satisfies => "«satisfies»",
            ReqRelationKind::Verifies => "«verifies»",
            ReqRelationKind::Refines => "«refines»",
            ReqRelationKind::Traces => "«traces»",
        };
        let dash = matches!(
            rel.kind,
            ReqRelationKind::Satisfies
                | ReqRelationKind::Verifies
                | ReqRelationKind::Refines
                | ReqRelationKind::Traces
        );
        let dasharray = if dash { "stroke-dasharray=\"5 3\"" } else { "" };
        svg.line(
            a.0,
            a.1,
            b.0,
            b.1,
            &format!("stroke=\"{stroke}\" stroke-width=\"1.5\" {dasharray}"),
        );
        let mx = (a.0 + b.0) / 2.0;
        let my = (a.1 + b.1) / 2.0;
        svg.rect(
            mx - 40.0,
            my - 9.0,
            80.0,
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

    // Boxes.
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
    }
}
