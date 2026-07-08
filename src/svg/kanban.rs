//! Kanban renderer. Columns side-by-side, cards stacked vertically.

use crate::parse::KanbanDiagram;

use super::builder::{escape, SvgBuilder};
use super::color::readable_text_color;
use super::theme::Theme;

const PAD: f64 = 24.0;
const COL_W: f64 = 200.0;
const COL_GAP: f64 = 18.0;
const HEAD_H: f64 = 36.0;
const CARD_H: f64 = 60.0;
const CARD_GAP: f64 = 10.0;

pub(crate) fn render(d: &KanbanDiagram, theme: &Theme) -> String {
    let fg = &theme.fg;
    let fg_muted = &theme.fg_muted;
    let stroke: &str = &theme.flow_node_stroke;
    let fill = &theme.flow_node_fill;

    let cols = d.columns.len().max(1);
    let max_tasks = d.columns.iter().map(|c| c.tasks.len()).max().unwrap_or(0);
    let width = PAD * 2.0 + cols as f64 * COL_W + (cols.saturating_sub(1) as f64) * COL_GAP;
    let height = PAD * 2.0 + HEAD_H + max_tasks.max(1) as f64 * (CARD_H + CARD_GAP) + 30.0;

    let mut svg = SvgBuilder::new(width, height).theme(theme);

    for (i, col) in d.columns.iter().enumerate() {
        let x = PAD + i as f64 * (COL_W + COL_GAP);
        let color = theme.cscale_color(i);
        // Column header.
        svg.rect(
            x,
            PAD,
            COL_W,
            HEAD_H,
            &format!("fill=\"{color}\" fill-opacity=\"0.85\" rx=\"6\""),
        );
        let tc = readable_text_color(color);
        svg.text(
            x + COL_W / 2.0,
            PAD + HEAD_H / 2.0 + 5.0,
            &format!("text-anchor=\"middle\" fill=\"{tc}\" font-size=\"14\" font-weight=\"bold\""),
            &col.label,
        );
        // Column body.
        svg.rect(x, PAD + HEAD_H, COL_W, max_tasks.max(1) as f64 * (CARD_H + CARD_GAP) + CARD_GAP,
            &format!("fill=\"{fg}\" fill-opacity=\"0.04\" stroke=\"{fg_muted}\" stroke-width=\"0.5\" rx=\"4\""));
        // Cards.
        for (j, t) in col.tasks.iter().enumerate() {
            let cy = PAD + HEAD_H + CARD_GAP + j as f64 * (CARD_H + CARD_GAP);
            // Upstream color-codes the left border of the card by priority.
            let (card_stroke, card_sw) = match priority_color(t.priority.as_deref()) {
                Some(c) => (c, 3.0),
                None => (stroke, 1.0),
            };
            svg.rect(
                x + 8.0,
                cy,
                COL_W - 16.0,
                CARD_H,
                &format!(
                    "fill=\"{fill}\" stroke=\"{card_stroke}\" stroke-width=\"{card_sw}\" rx=\"4\""
                ),
            );
            svg.text(
                x + 16.0,
                cy + 20.0,
                &format!("fill=\"{fg}\" font-size=\"13\" font-weight=\"bold\""),
                &t.text,
            );
            if let Some(a) = &t.assigned {
                svg.text(
                    x + 16.0,
                    cy + CARD_H - 10.0,
                    &format!("fill=\"{fg_muted}\" font-size=\"11\""),
                    &format!("@{a}"),
                );
            }
            if let Some(tk) = &t.ticket {
                draw_ticket(
                    &mut svg,
                    x + COL_W - 16.0,
                    cy + CARD_H - 10.0,
                    tk,
                    d.ticket_base_url.as_deref(),
                    fg_muted,
                );
            }
        }
    }

    svg.finish()
}

/// Border color for a card by its priority, matching upstream's four levels.
/// Any other value (e.g. `Medium`) uses the default node stroke.
fn priority_color(priority: Option<&str>) -> Option<&'static str> {
    match priority?.trim() {
        p if p.eq_ignore_ascii_case("Very High") => Some("#ff0000"),
        p if p.eq_ignore_ascii_case("High") => Some("#ff8800"),
        p if p.eq_ignore_ascii_case("Low") => Some("#00b0f0"),
        p if p.eq_ignore_ascii_case("Very Low") => Some("#8fd6ff"),
        _ => None,
    }
}

/// Draw the ticket id at `(x, y)` (right-anchored), hyperlinked when a
/// `ticketBaseUrl` is configured — `#TICKET#` in it is replaced by the id.
fn draw_ticket(svg: &mut SvgBuilder, x: f64, y: f64, ticket: &str, base: Option<&str>, fill: &str) {
    let attrs = format!("text-anchor=\"end\" fill=\"{fill}\" font-size=\"11\"");
    if let Some(base) = base {
        let href = if base.contains("#TICKET#") {
            base.replace("#TICKET#", ticket)
        } else {
            format!("{base}{ticket}")
        };
        svg.raw(&format!("<a href=\"{}\" target=\"_blank\">", escape(&href)));
        svg.text(x, y, &attrs, ticket);
        svg.raw("</a>");
    } else {
        svg.text(x, y, &attrs, ticket);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{KanbanColumn, KanbanTask};

    fn task(text: &str) -> KanbanTask {
        KanbanTask {
            id: text.into(),
            text: text.into(),
            assigned: None,
            priority: None,
            ticket: None,
        }
    }

    #[test]
    fn produces_svg() {
        let d = KanbanDiagram {
            columns: vec![KanbanColumn {
                id: "todo".into(),
                label: "Todo".into(),
                tasks: vec![KanbanTask {
                    assigned: Some("Alice".into()),
                    ..task("Task A")
                }],
            }],
            ticket_base_url: None,
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">Todo<"));
        assert!(svg.contains(">Task A<"));
        assert!(svg.contains("@Alice"));
    }

    #[test]
    fn column_header_uses_dark_text_on_pale_fill() {
        // Regression for #314: pale cScale headers need dark text, not white.
        let d = KanbanDiagram {
            columns: vec![KanbanColumn {
                id: "doing".into(),
                label: "In Progress".into(),
                tasks: vec![],
            }],
            ticket_base_url: None,
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("fill=\"#333333\" font-size=\"14\" font-weight=\"bold\""));
        assert!(!svg.contains("fill=\"#fff\" font-size=\"14\" font-weight=\"bold\""));
    }

    #[test]
    fn priority_colors_border_and_ticket_links() {
        let d = KanbanDiagram {
            columns: vec![KanbanColumn {
                id: "todo".into(),
                label: "Todo".into(),
                tasks: vec![KanbanTask {
                    priority: Some("Very High".into()),
                    ticket: Some("MC-2037".into()),
                    ..task("Blog")
                }],
            }],
            ticket_base_url: Some("https://tracker/#TICKET#".into()),
        };
        let svg = render(&d, &Theme::default());
        // Priority border color, not literal `[Very High]` text.
        assert!(svg.contains("#ff0000"));
        assert!(!svg.contains("[Very High]"));
        // Ticket rendered and linked with the id substituted into the base URL.
        assert!(svg.contains(">MC-2037<"));
        assert!(svg.contains("href=\"https://tracker/MC-2037\""));
    }

    #[test]
    fn ticket_without_base_url_is_plain_text() {
        let d = KanbanDiagram {
            columns: vec![KanbanColumn {
                id: "todo".into(),
                label: "Todo".into(),
                tasks: vec![KanbanTask {
                    ticket: Some("MC-1".into()),
                    ..task("Card")
                }],
            }],
            ticket_base_url: None,
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.contains(">MC-1<"));
        assert!(!svg.contains("href"));
    }
}
