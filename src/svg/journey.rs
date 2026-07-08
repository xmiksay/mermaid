//! User-journey renderer.
//!
//! Faithful to upstream Mermaid's composition (not a line chart): a colored
//! **section band** groups the tasks beneath it; each task is a rounded
//! **task box** with small **actor dots** straddling its top edge. Below the
//! task row a horizontal **time axis** (arrow-tipped) carries a score-driven
//! **face glyph** per task, hung on a dashed **stem**: the face's vertical
//! position encodes the score (score 5 nearest the axis, score 1 lowest), so
//! the smile/frown *and* the height both read the score. A vertical **actor
//! legend** (alphabetical, matching upstream) sits in the top-left gutter.

use crate::parse::JourneyDiagram;

use super::builder::{fnum, SvgBuilder};
use super::color::readable_text_color;
use super::theme::Theme;

const MARGIN: f64 = 20.0;
const TITLE_H: f64 = 40.0;
/// Left gutter reserved for the title and the actor legend; tasks start here.
const LEFT_MARGIN: f64 = 160.0;
const TASK_W: f64 = 150.0;
const TASK_H: f64 = 45.0;
const TASK_GAP: f64 = 20.0;
const SECTION_BAND_H: f64 = 24.0;
const FACE_R: f64 = 14.0;
const LEGEND_ROW: f64 = 24.0;
/// Gap from the task-box bottom down to the horizontal time axis.
const AXIS_GAP: f64 = 18.0;
/// Distance from the axis to the *edge* of the highest-scoring (score 5) face.
const FACE_TOP_GAP: f64 = 22.0;
/// Vertical travel of the face center between score 5 (top) and score 1.
const SCORE_SPAN: f64 = 84.0;

/// Face-center y for a task score, hung below the axis. Score is clamped to the
/// 1..=5 journey range; 5 rides nearest the axis, 1 sinks to the bottom.
fn face_cy_for(score: i32, axis_y: f64) -> f64 {
    let frac = (5 - score.clamp(1, 5)) as f64 / 4.0;
    axis_y + FACE_TOP_GAP + FACE_R + frac * SCORE_SPAN
}

pub(crate) fn render(d: &JourneyDiagram, theme: &Theme) -> String {
    let fg = &theme.fg;
    let fg_muted = &theme.fg_muted;

    let actors = collect_actors(d);
    let title_h = if d.title.is_some() { TITLE_H } else { 0.0 };
    let total_tasks: usize = d.sections.iter().map(|s| s.tasks.len()).sum();

    let content_top = MARGIN + title_h;
    let band_y = content_top;
    let task_y = band_y + SECTION_BAND_H + 12.0;
    let task_bottom = task_y + TASK_H;
    let axis_y = task_bottom + AXIS_GAP;
    // The lowest-scoring task hangs its face furthest down, setting the bottom.
    let lowest_score = d
        .sections
        .iter()
        .flat_map(|s| s.tasks.iter())
        .map(|t| t.score.clamp(1, 5))
        .min()
        .unwrap_or(5);
    let faces_bottom = face_cy_for(lowest_score, axis_y) + FACE_R;

    let tasks_span = if total_tasks == 0 {
        0.0
    } else {
        total_tasks as f64 * TASK_W + (total_tasks as f64 - 1.0) * TASK_GAP
    };
    let width = LEFT_MARGIN + tasks_span + MARGIN;
    let legend_bottom = content_top + actors.len() as f64 * LEGEND_ROW;
    let height = faces_bottom.max(legend_bottom) + MARGIN;

    let mut svg = SvgBuilder::new(width, height).theme(theme);
    if total_tasks > 0 {
        svg.def_arrow_marker("journey-axis", fg_muted, 8, 8);
    }

    if let Some(t) = &d.title {
        svg.text(
            LEFT_MARGIN,
            MARGIN + 20.0,
            &format!("fill=\"{fg}\" font-size=\"20\" font-weight=\"bold\""),
            t,
        );
    }

    // Actor legend in the top-left gutter.
    for (ai, actor) in actors.iter().enumerate() {
        let cy = content_top + 8.0 + ai as f64 * LEGEND_ROW;
        let color = theme.cscale_color(ai);
        svg.circle(
            MARGIN + 8.0,
            cy,
            7.0,
            &format!("fill=\"{color}\" stroke=\"{fg}\" stroke-width=\"1\""),
        );
        svg.text(
            MARGIN + 22.0,
            cy + 4.0,
            &format!("fill=\"{fg_muted}\" font-size=\"12\""),
            actor,
        );
    }

    // Horizontal time axis (arrow-tipped) running under the whole task row.
    if total_tasks > 0 {
        svg.line(
            LEFT_MARGIN,
            axis_y,
            LEFT_MARGIN + tasks_span + 12.0,
            axis_y,
            &format!(
                "stroke=\"{fg_muted}\" stroke-width=\"1.5\" \
                 marker-end=\"url(#journey-axis)\""
            ),
        );
    }

    // Section bands, task boxes, actor dots, and the score-driven faces hung
    // on dashed stems below the axis.
    let mut cursor = LEFT_MARGIN;
    for (si, sec) in d.sections.iter().enumerate() {
        if sec.tasks.is_empty() {
            continue;
        }
        let color = theme.cscale_color(si);
        let band_x0 = cursor;

        for t in &sec.tasks {
            let tx = cursor;
            let center = tx + TASK_W / 2.0;

            svg.rect(
                tx,
                task_y,
                TASK_W,
                TASK_H,
                &format!(
                    "rx=\"4\" ry=\"4\" fill=\"{color}\" fill-opacity=\"0.25\" \
                     stroke=\"{color}\" stroke-width=\"1\""
                ),
            );
            svg.text(
                center,
                task_y + TASK_H / 2.0 + 4.0,
                &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"13\""),
                &t.name,
            );

            // Dashed stem: axis → face, with a dot where it meets the axis.
            let face_cy = face_cy_for(t.score, axis_y);
            svg.line(
                center,
                axis_y,
                center,
                face_cy - FACE_R,
                &format!("stroke=\"{fg_muted}\" stroke-width=\"1\" stroke-dasharray=\"3 3\""),
            );
            svg.circle(center, axis_y, 2.5, &format!("fill=\"{color}\""));
            draw_face(&mut svg, center, face_cy, t.score, theme);

            // Actor dots straddle the top edge of the task box.
            let mut dx = tx + 14.0;
            for a in &t.actors {
                if let Some(idx) = actors.iter().position(|x| x == a) {
                    let ac = theme.cscale_color(idx);
                    svg.circle(
                        dx,
                        task_y,
                        6.0,
                        &format!("fill=\"{ac}\" stroke=\"{fg}\" stroke-width=\"1\""),
                    );
                    dx += 13.0;
                }
            }

            cursor += TASK_W + TASK_GAP;
        }

        let band_w = (cursor - TASK_GAP) - band_x0;
        svg.rect(
            band_x0,
            band_y,
            band_w,
            SECTION_BAND_H,
            &format!("rx=\"3\" ry=\"3\" fill=\"{color}\" stroke=\"{color}\""),
        );
        if !sec.name.is_empty() {
            let tc = readable_text_color(color);
            svg.text(
                band_x0 + band_w / 2.0,
                band_y + SECTION_BAND_H / 2.0 + 4.0,
                &format!(
                    "text-anchor=\"middle\" fill=\"{tc}\" font-size=\"13\" font-weight=\"bold\""
                ),
                &sec.name,
            );
        }
    }

    svg.finish()
}

/// Draw a score-driven face glyph: eyes plus a mouth that smiles (score ≥ 4),
/// stays flat (score = 3), or frowns (score ≤ 2).
fn draw_face(svg: &mut SvgBuilder, cx: f64, cy: f64, score: i32, theme: &Theme) {
    let stroke = &theme.fg_muted;
    let fill = &theme.bg;

    svg.circle(
        cx,
        cy,
        FACE_R,
        &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"2\""),
    );
    svg.circle(cx - 5.0, cy - 4.0, 1.6, &format!("fill=\"{stroke}\""));
    svg.circle(cx + 5.0, cy - 4.0, 1.6, &format!("fill=\"{stroke}\""));

    let mouth = if score >= 4 {
        // Smile: control point below the endpoints.
        format!(
            "M{} {} Q{} {} {} {}",
            fnum(cx - 6.0),
            fnum(cy + 3.0),
            fnum(cx),
            fnum(cy + 9.0),
            fnum(cx + 6.0),
            fnum(cy + 3.0),
        )
    } else if score <= 2 {
        // Frown: control point above the endpoints.
        format!(
            "M{} {} Q{} {} {} {}",
            fnum(cx - 6.0),
            fnum(cy + 8.0),
            fnum(cx),
            fnum(cy + 2.0),
            fnum(cx + 6.0),
            fnum(cy + 8.0),
        )
    } else {
        format!(
            "M{} {} L{} {}",
            fnum(cx - 6.0),
            fnum(cy + 6.0),
            fnum(cx + 6.0),
            fnum(cy + 6.0),
        )
    };
    svg.path(
        &mouth,
        &format!("fill=\"none\" stroke=\"{stroke}\" stroke-width=\"1.5\" stroke-linecap=\"round\""),
    );
}

/// Unique actor names, sorted alphabetically to match upstream's legend order;
/// each name's index assigns its stable legend color (shared with its dots).
fn collect_actors(d: &JourneyDiagram) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for sec in &d.sections {
        for t in &sec.tasks {
            for a in &t.actors {
                if !seen.contains(a) {
                    seen.push(a.clone());
                }
            }
        }
    }
    seen.sort();
    seen
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{JourneySection, JourneyTask};

    fn sample() -> JourneyDiagram {
        JourneyDiagram {
            title: Some("Day".into()),
            sections: vec![JourneySection {
                name: "Morning".into(),
                tasks: vec![
                    JourneyTask {
                        name: "Wake".into(),
                        score: 5,
                        actors: vec!["Me".into()],
                    },
                    JourneyTask {
                        name: "Grump".into(),
                        score: 1,
                        actors: vec!["Me".into(), "Cat".into()],
                    },
                ],
            }],
        }
    }

    #[test]
    fn produces_svg_with_title_task_and_section() {
        let svg = render(&sample(), &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">Day<"));
        assert!(svg.contains(">Wake<"));
        assert!(svg.contains(">Morning<"));
    }

    #[test]
    fn section_title_uses_dark_text_on_pale_band() {
        // Regression for #314: pale cScale bands need dark text, not white.
        let svg = render(&sample(), &Theme::default());
        assert!(svg.contains("fill=\"#333333\" font-size=\"13\" font-weight=\"bold\""));
        assert!(!svg.contains("fill=\"#fff\" font-size=\"13\" font-weight=\"bold\""));
    }

    #[test]
    fn draws_task_boxes_not_a_line_chart() {
        let svg = render(&sample(), &Theme::default());
        // Task boxes are rounded rects; no polyline path fill="none" chart lines.
        assert!(svg.contains("<rect"));
        assert!(svg.contains("rx=\"4\""));
        // Faces are present as circles with a mouth path.
        assert!(svg.contains("Q"), "expected a curved mouth path");
    }

    #[test]
    fn draws_time_axis_and_dashed_stems() {
        let svg = render(&sample(), &Theme::default());
        // Arrow-tipped horizontal axis.
        assert!(svg.contains("marker-end=\"url(#journey-axis)\""));
        assert!(svg.contains("<marker id=\"journey-axis\""));
        // Faces hang on dashed stems.
        assert!(svg.contains("stroke-dasharray=\"3 3\""));
    }

    #[test]
    fn face_height_encodes_score() {
        // Score 5 rides nearest the axis; score 1 sinks lower (larger y).
        let axis = 100.0;
        assert!(face_cy_for(5, axis) < face_cy_for(3, axis));
        assert!(face_cy_for(3, axis) < face_cy_for(1, axis));
        // Out-of-range scores clamp to the 1..=5 band.
        assert_eq!(face_cy_for(9, axis), face_cy_for(5, axis));
        assert_eq!(face_cy_for(-2, axis), face_cy_for(1, axis));
    }

    #[test]
    fn score_drives_mouth_shape() {
        // Smile (score 5) and frown (score 1) both use quadratic mouths; a
        // neutral score uses a straight line instead.
        let neutral = JourneyDiagram {
            title: None,
            sections: vec![JourneySection {
                name: String::new(),
                tasks: vec![JourneyTask {
                    name: "Meh".into(),
                    score: 3,
                    actors: vec![],
                }],
            }],
        };
        let svg = render(&neutral, &Theme::default());
        // Neutral mouth is an "L" line segment, not a "Q" curve.
        assert!(svg.contains("L"));
    }

    #[test]
    fn legend_lists_unique_actors_alphabetically() {
        let svg = render(&sample(), &Theme::default());
        assert!(svg.contains(">Me<"));
        assert!(svg.contains(">Cat<"));
        // Upstream orders the legend alphabetically, not by first appearance.
        let actors = collect_actors(&sample());
        assert_eq!(actors, vec!["Cat".to_string(), "Me".to_string()]);
    }
}
