//! Quadrant chart renderer. 0..1 square divided into 4 quadrants with
//! background tints, axis-end labels, and scatter points.

use crate::parse::QuadrantDiagram;

use super::builder::SvgBuilder;
use super::theme::Theme;

const PAD: f64 = 40.0;
const SIZE: f64 = 460.0;
const TITLE_GAP: f64 = 32.0;

pub(crate) fn render(d: &QuadrantDiagram, theme: &Theme) -> String {
    let fg = theme.fg;
    let fg_muted = theme.fg_muted;

    let title_h = if d.title.is_some() { TITLE_GAP } else { 0.0 };
    let width = PAD * 2.0 + SIZE + 60.0;
    let height = PAD * 2.0 + SIZE + title_h + 30.0;
    let chart_left = PAD + 30.0;
    let chart_top = PAD + title_h;

    let mut svg = SvgBuilder::new(width, height).font(theme.font_family, theme.font_size);

    if let Some(t) = &d.title {
        svg.text(
            width / 2.0,
            PAD + 18.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\" font-weight=\"bold\""),
            t,
        );
    }

    let half = SIZE / 2.0;
    // Quadrant backgrounds + labels (q2 top-left, q1 top-right, q3 bottom-left, q4 bottom-right).
    let qrects = [
        (d.q2.as_deref(), chart_left, chart_top, theme.pie_color(0)),
        (
            d.q1.as_deref(),
            chart_left + half,
            chart_top,
            theme.pie_color(1),
        ),
        (
            d.q3.as_deref(),
            chart_left,
            chart_top + half,
            theme.pie_color(2),
        ),
        (
            d.q4.as_deref(),
            chart_left + half,
            chart_top + half,
            theme.pie_color(3),
        ),
    ];
    for (label, x, y, color) in qrects {
        svg.rect(
            x,
            y,
            half,
            half,
            &format!(
                "fill=\"{color}\" fill-opacity=\"0.15\" stroke=\"{fg_muted}\" stroke-width=\"1\""
            ),
        );
        if let Some(l) = label {
            svg.text(
                x + half / 2.0,
                y + 18.0,
                &format!(
                    "text-anchor=\"middle\" fill=\"{fg}\" font-size=\"13\" font-weight=\"bold\""
                ),
                l,
            );
        }
    }

    // Outer border + midlines.
    svg.rect(
        chart_left,
        chart_top,
        SIZE,
        SIZE,
        &format!("fill=\"none\" stroke=\"{fg}\" stroke-width=\"1.5\""),
    );
    svg.line(
        chart_left + half,
        chart_top,
        chart_left + half,
        chart_top + SIZE,
        &format!("stroke=\"{fg_muted}\" stroke-width=\"1\""),
    );
    svg.line(
        chart_left,
        chart_top + half,
        chart_left + SIZE,
        chart_top + half,
        &format!("stroke=\"{fg_muted}\" stroke-width=\"1\""),
    );

    // Axis labels.
    if let Some(l) = &d.x_axis_left {
        svg.text(
            chart_left,
            chart_top + SIZE + 22.0,
            &format!("text-anchor=\"start\" fill=\"{fg}\" font-size=\"12\""),
            l,
        );
    }
    if let Some(r) = &d.x_axis_right {
        svg.text(
            chart_left + SIZE,
            chart_top + SIZE + 22.0,
            &format!("text-anchor=\"end\" fill=\"{fg}\" font-size=\"12\""),
            r,
        );
    }
    if let Some(b) = &d.y_axis_bottom {
        svg.text(
            chart_left - 8.0,
            chart_top + SIZE - 4.0,
            &format!("text-anchor=\"end\" fill=\"{fg}\" font-size=\"12\""),
            b,
        );
    }
    if let Some(t) = &d.y_axis_top {
        svg.text(
            chart_left - 8.0,
            chart_top + 12.0,
            &format!("text-anchor=\"end\" fill=\"{fg}\" font-size=\"12\""),
            t,
        );
    }

    // Points.
    for (i, p) in d.points.iter().enumerate() {
        let px = chart_left + p.x.clamp(0.0, 1.0) * SIZE;
        let py = chart_top + (1.0 - p.y.clamp(0.0, 1.0)) * SIZE;

        // Resolve styling: class defaults first, then per-point overrides.
        let class = p.class_name.as_deref().and_then(|name| d.classes.get(name));
        let radius = p
            .radius
            .or_else(|| class.and_then(|c| c.radius))
            .unwrap_or(6.0);
        let fill = p
            .color
            .clone()
            .or_else(|| class.and_then(|c| c.color.clone()))
            .unwrap_or_else(|| theme.pie_color(i + 4).to_string());
        let stroke = p
            .stroke_color
            .clone()
            .or_else(|| class.and_then(|c| c.stroke_color.clone()))
            .unwrap_or_else(|| "#fff".to_string());
        let stroke_width = p
            .stroke_width
            .clone()
            .or_else(|| class.and_then(|c| c.stroke_width.clone()))
            .unwrap_or_else(|| "1.5".to_string());

        svg.circle(
            px,
            py,
            radius,
            &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{stroke_width}\""),
        );
        svg.text(
            px + 9.0,
            py + 4.0,
            &format!("fill=\"{fg}\" font-size=\"11\""),
            &p.label,
        );
    }

    svg.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::QuadrantPoint;

    #[test]
    fn produces_svg() {
        let d = QuadrantDiagram {
            title: Some("Chart".into()),
            x_axis_left: Some("Low".into()),
            x_axis_right: Some("High".into()),
            y_axis_bottom: Some("L".into()),
            y_axis_top: Some("H".into()),
            q1: Some("Q1".into()),
            q2: Some("Q2".into()),
            q3: Some("Q3".into()),
            q4: Some("Q4".into()),
            points: vec![QuadrantPoint {
                label: "A".into(),
                x: 0.3,
                y: 0.6,
                radius: None,
                color: None,
                stroke_color: None,
                stroke_width: None,
                class_name: None,
            }],
            classes: Default::default(),
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">Chart<"));
        assert!(svg.contains(">A<"));
        assert!(svg.contains(">Q1<"));
    }

    #[test]
    fn honors_radius_and_color() {
        let d = QuadrantDiagram {
            points: vec![QuadrantPoint {
                label: "A".into(),
                x: 0.3,
                y: 0.6,
                radius: Some(12.0),
                color: Some("#ff0000".into()),
                stroke_color: Some("#00ff00".into()),
                stroke_width: Some("3px".into()),
                class_name: None,
            }],
            ..Default::default()
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("r=\"12\""));
        assert!(svg.contains("fill=\"#ff0000\""));
        assert!(svg.contains("stroke=\"#00ff00\""));
        assert!(svg.contains("stroke-width=\"3px\""));
    }
}
