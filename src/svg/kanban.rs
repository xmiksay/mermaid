//! Kanban renderer. Columns side-by-side, cards stacked vertically.

use crate::parse::KanbanDiagram;

use super::builder::SvgBuilder;
use super::theme::Theme;

const PAD: f64 = 24.0;
const COL_W: f64 = 200.0;
const COL_GAP: f64 = 18.0;
const HEAD_H: f64 = 36.0;
const CARD_H: f64 = 60.0;
const CARD_GAP: f64 = 10.0;

pub(crate) fn render(d: &KanbanDiagram, theme: &Theme) -> String {
    let fg = theme.fg;
    let fg_muted = theme.fg_muted;
    let stroke = theme.flow_node_stroke;
    let fill = theme.flow_node_fill;

    let cols = d.columns.len().max(1);
    let max_tasks = d.columns.iter().map(|c| c.tasks.len()).max().unwrap_or(0);
    let width = PAD * 2.0 + cols as f64 * COL_W + (cols.saturating_sub(1) as f64) * COL_GAP;
    let height = PAD * 2.0 + HEAD_H + max_tasks.max(1) as f64 * (CARD_H + CARD_GAP) + 30.0;

    let mut svg = SvgBuilder::new(width, height);

    for (i, col) in d.columns.iter().enumerate() {
        let x = PAD + i as f64 * (COL_W + COL_GAP);
        let color = theme.pie_color(i);
        // Column header.
        svg.rect(
            x,
            PAD,
            COL_W,
            HEAD_H,
            &format!("fill=\"{color}\" fill-opacity=\"0.85\" rx=\"6\""),
        );
        svg.text(
            x + COL_W / 2.0,
            PAD + HEAD_H / 2.0 + 5.0,
            &format!("text-anchor=\"middle\" fill=\"#fff\" font-size=\"14\" font-weight=\"bold\""),
            &col.label,
        );
        // Column body.
        svg.rect(x, PAD + HEAD_H, COL_W, max_tasks.max(1) as f64 * (CARD_H + CARD_GAP) + CARD_GAP,
            &format!("fill=\"{fg}\" fill-opacity=\"0.04\" stroke=\"{fg_muted}\" stroke-width=\"0.5\" rx=\"4\""));
        // Cards.
        for (j, t) in col.tasks.iter().enumerate() {
            let cy = PAD + HEAD_H + CARD_GAP + j as f64 * (CARD_H + CARD_GAP);
            svg.rect(
                x + 8.0,
                cy,
                COL_W - 16.0,
                CARD_H,
                &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1\" rx=\"4\""),
            );
            svg.text(
                x + 16.0,
                cy + 20.0,
                &format!("fill=\"{fg}\" font-size=\"13\" font-weight=\"bold\""),
                &t.text,
            );
            let mut meta = Vec::new();
            if let Some(a) = &t.assigned {
                meta.push(format!("@{a}"));
            }
            if let Some(p) = &t.priority {
                meta.push(format!("[{p}]"));
            }
            if !meta.is_empty() {
                svg.text(
                    x + 16.0,
                    cy + CARD_H - 10.0,
                    &format!("fill=\"{fg_muted}\" font-size=\"11\""),
                    &meta.join("  "),
                );
            }
        }
    }

    svg.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{KanbanColumn, KanbanTask};

    #[test]
    fn produces_svg() {
        let d = KanbanDiagram {
            columns: vec![KanbanColumn {
                id: "todo".into(),
                label: "Todo".into(),
                tasks: vec![KanbanTask {
                    id: "a".into(),
                    text: "Task A".into(),
                    assigned: Some("Alice".into()),
                    priority: None,
                }],
            }],
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">Todo<"));
        assert!(svg.contains(">Task A<"));
        assert!(svg.contains("@Alice"));
    }
}
