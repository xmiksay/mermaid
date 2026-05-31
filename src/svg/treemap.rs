//! Treemap renderer. Simple slice-and-dice layout (alternating direction by
//! depth). Produces nested rectangles sized by value.

use crate::parse::{TreemapDiagram, TreemapNode};

use super::builder::SvgBuilder;
use super::theme::Theme;

const PAD: f64 = 24.0;
const TITLE_GAP: f64 = 32.0;
const CHART_W: f64 = 640.0;
const CHART_H: f64 = 420.0;
const HEADER_H: f64 = 22.0;

pub(crate) fn render(d: &TreemapDiagram, theme: &Theme) -> String {
    let fg = theme.fg;
    let title_h = if d.title.is_some() { TITLE_GAP } else { 0.0 };
    let width = PAD * 2.0 + CHART_W;
    let height = PAD * 2.0 + title_h + CHART_H;
    let mut svg = SvgBuilder::new(width, height);

    if let Some(t) = &d.title {
        svg.text(
            width / 2.0,
            PAD + 18.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\" font-weight=\"bold\""),
            t,
        );
    }

    let x0 = PAD;
    let y0 = PAD + title_h;
    layout(&d.root, x0, y0, CHART_W, CHART_H, 0, &mut svg, theme);

    svg.finish()
}

fn node_value(n: &TreemapNode) -> f64 {
    if let Some(v) = n.value {
        return v;
    }
    let s: f64 = n.children.iter().map(node_value).sum();
    if s == 0.0 {
        1.0
    } else {
        s
    }
}

#[allow(clippy::too_many_arguments)]
fn layout(
    nodes: &[TreemapNode],
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    depth: usize,
    svg: &mut SvgBuilder,
    theme: &Theme,
) {
    if nodes.is_empty() || w <= 2.0 || h <= 2.0 {
        return;
    }
    let total: f64 = nodes.iter().map(node_value).sum();
    let horizontal = depth.is_multiple_of(2);
    let mut offset = 0.0;
    for (i, n) in nodes.iter().enumerate() {
        let frac = node_value(n) / total.max(1e-9);
        let (nx, ny, nw, nh) = if horizontal {
            let nw = w * frac;
            let r = (x + offset, y, nw, h);
            offset += nw;
            r
        } else {
            let nh = h * frac;
            let r = (x, y + offset, w, nh);
            offset += nh;
            r
        };
        let color = theme.pie_color(i + depth);
        svg.rect(
            nx,
            ny,
            nw,
            nh,
            &format!(
                "fill=\"{color}\" fill-opacity=\"{op}\" stroke=\"#fff\" stroke-width=\"1.5\"",
                op = if n.children.is_empty() {
                    "0.85"
                } else {
                    "0.25"
                }
            ),
        );
        let label = &n.label;
        if nw > 24.0 && nh > 16.0 {
            let font_size = if nw < 60.0 { 10 } else { 12 };
            svg.text(
                nx + 4.0,
                ny + 12.0,
                &format!(
                    "fill=\"{}\" font-size=\"{font_size}\" font-weight=\"bold\"",
                    if n.children.is_empty() {
                        "#fff"
                    } else {
                        theme.fg
                    }
                ),
                label,
            );
            if let Some(v) = n.value {
                if n.children.is_empty() && nh > 28.0 {
                    svg.text(
                        nx + 4.0,
                        ny + 24.0,
                        "fill=\"#fff\" font-size=\"9\"",
                        &format!("{v}"),
                    );
                }
            }
        }
        if !n.children.is_empty() && nw > 30.0 && nh > HEADER_H + 10.0 {
            layout(
                &n.children,
                nx + 4.0,
                ny + HEADER_H,
                nw - 8.0,
                nh - HEADER_H - 4.0,
                depth + 1,
                svg,
                theme,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_svg() {
        let d = TreemapDiagram {
            title: Some("Tree".into()),
            root: vec![TreemapNode {
                label: "A".into(),
                value: None,
                children: vec![
                    TreemapNode {
                        label: "A1".into(),
                        value: Some(3.0),
                        children: vec![],
                    },
                    TreemapNode {
                        label: "A2".into(),
                        value: Some(7.0),
                        children: vec![],
                    },
                ],
            }],
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">Tree<"));
        assert!(svg.contains(">A1<"));
    }
}
