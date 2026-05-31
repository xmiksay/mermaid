//! User-journey renderer.
//!
//! Layout: tasks across the x-axis, score (0-5) on the y-axis, line+points
//! per actor-set. Sections appear as labeled bands above the chart.

use std::fmt::Write as _;

use crate::parse::JourneyDiagram;

use super::builder::{fnum, SvgBuilder};
use super::theme::Theme;

const PAD: f64 = 30.0;
const TITLE_GAP: f64 = 32.0;
const SECTION_BAND: f64 = 26.0;
const TASK_WIDTH: f64 = 110.0;
const CHART_HEIGHT: f64 = 220.0;
const AXIS_PAD_LEFT: f64 = 30.0;
const LEGEND_GAP: f64 = 26.0;

pub(crate) fn render(d: &JourneyDiagram, theme: &Theme) -> String {
    let fg = theme.fg;
    let fg_muted = theme.fg_muted;

    let total_tasks: usize = d.sections.iter().map(|s| s.tasks.len()).sum();
    let chart_w = (total_tasks.max(1) as f64) * TASK_WIDTH;
    let title_h = if d.title.is_some() { TITLE_GAP } else { 0.0 };

    let actors: Vec<String> = collect_actors(d);
    let legend_h = if actors.is_empty() { 0.0 } else { LEGEND_GAP };

    let width = PAD * 2.0 + AXIS_PAD_LEFT + chart_w;
    let height = PAD * 2.0 + title_h + SECTION_BAND + CHART_HEIGHT + legend_h + 30.0;

    let mut svg = SvgBuilder::new(width, height).font(theme.font_family, theme.font_size);

    if let Some(t) = &d.title {
        svg.text(
            width / 2.0,
            PAD + 18.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\" font-weight=\"bold\""),
            t,
        );
    }

    let section_y = PAD + title_h;
    let chart_top = section_y + SECTION_BAND;
    let chart_bottom = chart_top + CHART_HEIGHT;
    let chart_left = PAD + AXIS_PAD_LEFT;

    // Section bands across the top.
    let mut x = chart_left;
    for (si, sec) in d.sections.iter().enumerate() {
        let w = sec.tasks.len() as f64 * TASK_WIDTH;
        if w > 0.0 {
            let color = theme.pie_color(si);
            svg.rect(
                x,
                section_y,
                w,
                SECTION_BAND - 4.0,
                &format!(
                    "fill=\"{color}\" fill-opacity=\"0.25\" stroke=\"{color}\" stroke-width=\"1\""
                ),
            );
            if !sec.name.is_empty() {
                svg.text(
                    x + w / 2.0,
                    section_y + SECTION_BAND / 2.0 + 2.0,
                    &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"13\" font-weight=\"bold\""),
                    &sec.name,
                );
            }
        }
        x += w;
    }

    // Score grid (0..=5).
    for s in 0..=5 {
        let y = chart_bottom - (s as f64 / 5.0) * CHART_HEIGHT;
        svg.line(
            chart_left,
            y,
            chart_left + chart_w,
            y,
            &format!("stroke=\"{fg_muted}\" stroke-width=\"1\" stroke-dasharray=\"2 3\""),
        );
        svg.text(
            chart_left - 6.0,
            y + 4.0,
            &format!("text-anchor=\"end\" fill=\"{fg_muted}\" font-size=\"11\""),
            &s.to_string(),
        );
    }

    // Per-actor lines and points.
    let task_xs: Vec<(f64, &super::super::parse::ast::JourneyTask)> = {
        let mut out = Vec::new();
        let mut cursor = chart_left + TASK_WIDTH / 2.0;
        for sec in &d.sections {
            for t in &sec.tasks {
                out.push((cursor, t));
                cursor += TASK_WIDTH;
            }
        }
        out
    };

    // Task name labels under chart.
    for (cx, t) in &task_xs {
        svg.text(
            *cx,
            chart_bottom + 16.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\""),
            &t.name,
        );
    }

    // Plot per actor.
    for (ai, actor) in actors.iter().enumerate() {
        let color = theme.pie_color((ai + 3) % 10);
        let mut path = String::new();
        let mut first = true;
        let mut prev: Option<(f64, f64)> = None;
        for (cx, t) in &task_xs {
            if !t.actors.iter().any(|a| a == actor) && (!actor.is_empty() || !t.actors.is_empty()) {
                if let Some(p) = prev {
                    // Draw running segment up to here, then break.
                    let _ = p; // path break: start new subpath
                }
                first = true;
                prev = None;
                continue;
            }
            let cy = chart_bottom - (t.score as f64).clamp(0.0, 5.0) / 5.0 * CHART_HEIGHT;
            if first {
                let _ = write!(path, "M{} {}", fnum(*cx), fnum(cy));
                first = false;
            } else {
                let _ = write!(path, "L{} {}", fnum(*cx), fnum(cy));
            }
            prev = Some((*cx, cy));
            svg.circle(
                *cx,
                cy,
                5.0,
                &format!("fill=\"{color}\" stroke=\"#fff\" stroke-width=\"1.5\""),
            );
        }
        if !path.is_empty() {
            svg.path(
                &path,
                &format!("fill=\"none\" stroke=\"{color}\" stroke-width=\"2\""),
            );
        }
    }

    // Legend.
    if !actors.is_empty() {
        let ly = chart_bottom + 36.0;
        let mut lx = chart_left;
        for (ai, actor) in actors.iter().enumerate() {
            let color = theme.pie_color((ai + 3) % 10);
            svg.circle(lx + 6.0, ly, 5.0, &format!("fill=\"{color}\""));
            svg.text(
                lx + 16.0,
                ly + 4.0,
                &format!("fill=\"{fg}\" font-size=\"12\""),
                if actor.is_empty() { "(none)" } else { actor },
            );
            lx += 30.0 + (actor.chars().count() as f64) * 7.5;
        }
    }

    svg.finish()
}

fn collect_actors(d: &JourneyDiagram) -> Vec<String> {
    let mut seen = Vec::new();
    for sec in &d.sections {
        for t in &sec.tasks {
            if t.actors.is_empty() && !seen.iter().any(|x: &String| x.is_empty()) {
                seen.push(String::new());
            }
            for a in &t.actors {
                if !seen.contains(a) {
                    seen.push(a.clone());
                }
            }
        }
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{JourneySection, JourneyTask};

    #[test]
    fn produces_svg() {
        let d = JourneyDiagram {
            title: Some("Day".into()),
            sections: vec![JourneySection {
                name: "Morning".into(),
                tasks: vec![JourneyTask {
                    name: "Wake".into(),
                    score: 3,
                    actors: vec!["Me".into()],
                }],
            }],
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">Day<"));
        assert!(svg.contains(">Wake<"));
        assert!(svg.contains(">Morning<"));
    }
}
