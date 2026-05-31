//! Mindmap renderer. Radial layout from a central root.

use std::fmt::Write as _;

use crate::parse::{MindmapDiagram, MindmapNode, MindmapShape};

use super::builder::{fnum, SvgBuilder};
use super::theme::Theme;

const NODE_PAD_X: f64 = 12.0;
const NODE_H: f64 = 28.0;
const LEVEL_GAP: f64 = 130.0;
const SIBLING_GAP: f64 = 14.0;
const TEXT_PX: f64 = 7.0;

#[derive(Clone)]
struct Laid {
    node: MindmapNode,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    children: Vec<Laid>,
    subtree_h: f64,
}

pub(crate) fn render(d: &MindmapDiagram, theme: &Theme) -> String {
    let Some(root) = d.root.clone() else {
        let mut svg = SvgBuilder::new(200.0, 80.0);
        svg.text(
            100.0,
            40.0,
            &format!(
                "text-anchor=\"middle\" fill=\"{}\" font-size=\"13\"",
                theme.fg_muted
            ),
            "(empty mindmap)",
        );
        return svg.finish();
    };

    // Layout: assign each subtree a vertical band, then root.x = 0, children to the right.
    let mut laid = layout(&root, 0);
    let total_h = laid.subtree_h;
    shift(&mut laid, 30.0, 30.0 + total_h / 2.0);

    let (max_x, max_y) = bbox(&laid);
    let width = max_x + 30.0;
    let height = (max_y + 30.0).max(total_h + 60.0);

    let mut svg = SvgBuilder::new(width, height);

    draw_edges(&laid, &mut svg, theme);
    draw_nodes(&laid, &mut svg, theme, 0);

    svg.finish()
}

fn layout(n: &MindmapNode, depth: usize) -> Laid {
    let w = (n.text.chars().count() as f64) * TEXT_PX + NODE_PAD_X * 2.0;
    let w = w.max(40.0);
    let mut children: Vec<Laid> = n.children.iter().map(|c| layout(c, depth + 1)).collect();
    let mut total = 0.0;
    for (i, c) in children.iter().enumerate() {
        total += c.subtree_h;
        if i + 1 < n.children.len() {
            total += SIBLING_GAP;
        }
    }
    let subtree_h = total.max(NODE_H);
    let mut cursor = -subtree_h / 2.0;
    for c in &mut children {
        let dy = cursor + c.subtree_h / 2.0;
        shift(c, depth as f64 * 0.0, dy);
        cursor += c.subtree_h + SIBLING_GAP;
    }
    Laid {
        node: n.clone(),
        x: depth as f64 * LEVEL_GAP,
        y: 0.0,
        w,
        h: NODE_H,
        children,
        subtree_h,
    }
}

fn shift(laid: &mut Laid, dx: f64, dy: f64) {
    laid.x += dx;
    laid.y += dy;
    for c in &mut laid.children {
        shift(c, dx, dy);
    }
}

fn bbox(laid: &Laid) -> (f64, f64) {
    let mut mx = laid.x + laid.w;
    let mut my = laid.y + laid.h / 2.0;
    for c in &laid.children {
        let (cx, cy) = bbox(c);
        mx = mx.max(cx);
        my = my.max(cy);
    }
    (mx, my)
}

fn draw_edges(laid: &Laid, svg: &mut SvgBuilder, theme: &Theme) {
    let stroke = theme.flow_edge_stroke;
    for c in &laid.children {
        let x1 = laid.x + laid.w;
        let y1 = laid.y;
        let x2 = c.x;
        let y2 = c.y;
        let mx = (x1 + x2) / 2.0;
        let mut path = String::new();
        let _ = write!(
            path,
            "M{} {}C{} {}, {} {}, {} {}",
            fnum(x1),
            fnum(y1),
            fnum(mx),
            fnum(y1),
            fnum(mx),
            fnum(y2),
            fnum(x2),
            fnum(y2)
        );
        svg.path(
            &path,
            &format!("fill=\"none\" stroke=\"{stroke}\" stroke-width=\"1.5\""),
        );
        draw_edges(c, svg, theme);
    }
}

fn draw_nodes(laid: &Laid, svg: &mut SvgBuilder, theme: &Theme, depth: usize) {
    let fg = theme.fg;
    let fill = theme.flow_node_fill;
    let stroke = theme.flow_node_stroke;
    let n = &laid.node;
    let cx = laid.x + laid.w / 2.0;
    let cy = laid.y;
    let half_w = laid.w / 2.0;
    let half_h = laid.h / 2.0;

    match n.shape {
        MindmapShape::Default => {
            svg.line(
                laid.x,
                cy + half_h,
                laid.x + laid.w,
                cy + half_h,
                &format!("stroke=\"{stroke}\" stroke-width=\"1\""),
            );
        }
        MindmapShape::Square => {
            svg.rect(
                laid.x,
                cy - half_h,
                laid.w,
                laid.h,
                &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.5\""),
            );
        }
        MindmapShape::Rounded => {
            let _ = depth;
            svg.rect(
                laid.x,
                cy - half_h,
                laid.w,
                laid.h,
                &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.5\" rx=\"8\""),
            );
        }
        MindmapShape::Circle => {
            let r = half_w.max(half_h);
            svg.circle(
                cx,
                cy,
                r,
                &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.5\""),
            );
        }
        MindmapShape::Bang => {
            // Star-like outline approximated as rounded shape with thick stroke.
            svg.rect(laid.x, cy - half_h, laid.w, laid.h,
                &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"2.5\" stroke-dasharray=\"4 2\" rx=\"4\""));
        }
        MindmapShape::Cloud => {
            // Approximate cloud by series of arcs; use stadium shape.
            svg.rect(
                laid.x,
                cy - half_h,
                laid.w,
                laid.h,
                &format!(
                    "fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.5\" rx=\"{}\"",
                    fnum(half_h)
                ),
            );
        }
        MindmapShape::Hexagon => {
            let d = format!(
                "M{l} {c}L{a} {t}L{b} {t}L{r} {c}L{b} {bb}L{a} {bb}Z",
                l = fnum(laid.x),
                r = fnum(laid.x + laid.w),
                t = fnum(cy - half_h),
                bb = fnum(cy + half_h),
                c = fnum(cy),
                a = fnum(laid.x + half_h),
                b = fnum(laid.x + laid.w - half_h),
            );
            svg.path(
                &d,
                &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.5\""),
            );
        }
    }

    svg.text(
        cx,
        cy + 4.0,
        &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"13\""),
        &n.text,
    );

    if let Some(icon) = &n.icon {
        svg.text(
            cx,
            cy + half_h + 14.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"10\" font-style=\"italic\""),
            icon,
        );
    }

    for c in &laid.children {
        draw_nodes(c, svg, theme, depth + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::MindmapNode;

    #[test]
    fn produces_svg() {
        let d = MindmapDiagram {
            root: Some(MindmapNode {
                text: "root".into(),
                shape: MindmapShape::Circle,
                icon: None,
                children: vec![MindmapNode {
                    text: "A".into(),
                    shape: MindmapShape::Rounded,
                    icon: None,
                    children: vec![],
                }],
            }),
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">root<"));
        assert!(svg.contains(">A<"));
    }
}
