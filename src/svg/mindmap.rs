//! Mindmap renderer. Two-sided layout fanning out from a central root.

use std::fmt::Write as _;

use std::collections::HashMap;

use crate::parse::ast::Style;
use crate::parse::{MindmapDiagram, MindmapNode, MindmapShape};

use super::builder::{fnum, SvgBuilder};
use super::metrics::text_width;
use super::style::resolve_style;
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
    /// +1 for a branch growing to the right of the root, -1 for the left.
    dir: f64,
}

pub(crate) fn render(d: &MindmapDiagram, theme: &Theme) -> String {
    let Some(root) = d.root.clone() else {
        let mut svg = SvgBuilder::new(200.0, 80.0).theme(theme);
        svg.text(
            100.0,
            40.0,
            &format!(
                "text-anchor=\"middle\" fill=\"{}\" font-size=\"13\"",
                &theme.fg_muted
            ),
            "(empty mindmap)",
        );
        return svg.finish();
    };

    // Lay out each first-level branch as a canonical right-growing subtree, then
    // alternate them onto the right and left of the root (a back-to-back pair of
    // half-trees) so the map fans out on both sides instead of only rightward.
    let font_size = theme.font_size;
    let root_w = node_width(&root.text, font_size);
    let mut right: Vec<Laid> = Vec::new();
    let mut left: Vec<Laid> = Vec::new();
    for (i, c) in root.children.iter().enumerate() {
        let branch = layout(c, 1, font_size);
        if i % 2 == 0 {
            right.push(branch);
        } else {
            left.push(branch);
        }
    }
    stack_group(&mut right);
    for l in &mut left {
        mirror(l, root_w);
    }
    stack_group(&mut left);

    let mut children = right;
    children.extend(left);
    let mut laid = Laid {
        node: root.clone(),
        x: 0.0,
        y: 0.0,
        w: root_w,
        h: NODE_H,
        children,
        subtree_h: 0.0,
        dir: 1.0,
    };

    // Frame the whole tree (both sides plus icons) and shift into positive space.
    let margin = 30.0;
    let (min_x, min_y, max_x, max_y) = bounds(&laid);
    shift(&mut laid, margin - min_x, margin - min_y);
    let width = (max_x - min_x) + margin * 2.0;
    let height = (max_y - min_y) + margin * 2.0;

    let mut svg = SvgBuilder::new(width, height).theme(theme);

    draw_edges(&laid, &mut svg, theme);
    draw_nodes(&laid, &mut svg, theme, &d.class_defs, 0);

    svg.finish()
}

fn node_width(text: &str, font_size: f64) -> f64 {
    (text_width(text, TEXT_PX, font_size) + NODE_PAD_X * 2.0).max(40.0)
}

fn layout(n: &MindmapNode, depth: usize, font_size: f64) -> Laid {
    let w = node_width(&n.text, font_size);
    let mut children: Vec<Laid> = n
        .children
        .iter()
        .map(|c| layout(c, depth + 1, font_size))
        .collect();
    let subtree_h = stack_group(&mut children);
    Laid {
        node: n.clone(),
        x: depth as f64 * LEVEL_GAP,
        y: 0.0,
        w,
        h: NODE_H,
        children,
        subtree_h,
        dir: 1.0,
    }
}

/// Distribute `children` into vertical bands centered on y=0, returning the
/// total band height (clamped to a single node).
fn stack_group(children: &mut [Laid]) -> f64 {
    let mut total = 0.0;
    for (i, c) in children.iter().enumerate() {
        total += c.subtree_h;
        if i + 1 < children.len() {
            total += SIBLING_GAP;
        }
    }
    let subtree_h = total.max(NODE_H);
    let mut cursor = -subtree_h / 2.0;
    for c in children.iter_mut() {
        let dy = cursor + c.subtree_h / 2.0;
        shift(c, 0.0, dy);
        cursor += c.subtree_h + SIBLING_GAP;
    }
    subtree_h
}

/// Reflect a canonical (right-growing) subtree horizontally about the root
/// centre so it grows leftward, and mark every node as a left-side branch.
fn mirror(laid: &mut Laid, root_w: f64) {
    laid.x = root_w - (laid.x + laid.w);
    laid.dir = -1.0;
    for c in &mut laid.children {
        mirror(c, root_w);
    }
}

fn shift(laid: &mut Laid, dx: f64, dy: f64) {
    laid.x += dx;
    laid.y += dy;
    for c in &mut laid.children {
        shift(c, dx, dy);
    }
}

fn bounds(laid: &Laid) -> (f64, f64, f64, f64) {
    let mut min_x = laid.x;
    let mut max_x = laid.x + laid.w;
    let mut min_y = laid.y - laid.h / 2.0;
    let mut max_y = laid.y + laid.h / 2.0;
    if laid.node.icon.is_some() {
        max_y = max_y.max(laid.y + laid.h / 2.0 + 12.0 + ICON_SIZE);
    }
    for c in &laid.children {
        let (a, b, cc, d) = bounds(c);
        min_x = min_x.min(a);
        min_y = min_y.min(b);
        max_x = max_x.max(cc);
        max_y = max_y.max(d);
    }
    (min_x, min_y, max_x, max_y)
}

fn draw_edges(laid: &Laid, svg: &mut SvgBuilder, theme: &Theme) {
    let stroke = &theme.flow_edge_stroke;
    for c in &laid.children {
        // A right branch leaves the parent's right edge for the child's left
        // edge; a left branch is mirrored, so leave the left edge for the right.
        let (x1, x2) = if c.dir >= 0.0 {
            (laid.x + laid.w, c.x)
        } else {
            (laid.x, c.x + c.w)
        };
        let y1 = laid.y;
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

fn draw_nodes(
    laid: &Laid,
    svg: &mut SvgBuilder,
    theme: &Theme,
    class_defs: &HashMap<String, Style>,
    depth: usize,
) {
    let n = &laid.node;
    let rs = resolve_style(class_defs, &n.classes, &Style::new());
    let fg = rs.label_fill(&theme.fg).to_string();
    let fg = fg.as_str();
    let fill = rs
        .fill
        .clone()
        .unwrap_or_else(|| theme.flow_node_fill.to_string());
    let fill = fill.as_str();
    let stroke = rs.stroke_or(&theme.flow_node_stroke).to_string();
    let stroke = stroke.as_str();
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
        &format!(
            "text-anchor=\"middle\" fill=\"{fg}\" font-size=\"13\"{}",
            rs.text_attrs()
        ),
        &n.text,
    );

    if let Some(icon) = &n.icon {
        // Real Font Awesome glyphs aren't available without the font, so map
        // the class string onto a small builtin glyph rather than printing the
        // raw `fa fa-book` text (matching the architecture renderer's approach).
        draw_mindmap_icon(svg, icon, cx, cy + half_h + 12.0, fg);
    }

    for c in &laid.children {
        draw_nodes(c, svg, theme, class_defs, depth + 1);
    }
}

const ICON_SIZE: f64 = 16.0;

/// Extract the meaningful name from a Font Awesome class string
/// (`fa fa-book` / `fab fa-github` / `book`) — the last `fa-`-prefixed token,
/// or the last token otherwise.
fn icon_name(icon: &str) -> &str {
    icon.split_whitespace()
        .filter_map(|t| t.strip_prefix("fa-"))
        .next_back()
        .or_else(|| icon.split_whitespace().next_back())
        .unwrap_or("")
}

fn draw_mindmap_icon(svg: &mut SvgBuilder, icon: &str, cx: f64, top_y: f64, stroke: &str) {
    let paths: &[&str] = match icon_name(icon) {
        "book" => &[
            "M6 5 H15 C16 5 17 6 17 7 V27 C17 26 16 25 15 25 H6 Z",
            "M26 5 H17 C16 5 15 6 15 7 V27 C15 26 16 25 17 25 H26 Z",
        ],
        "star" => &["M16 3 L20 13 L31 13 L22 20 L25 30 L16 24 L7 30 L10 20 L1 13 L12 13 Z"],
        "clock" | "hourglass" => &[
            "M16 4 A12 12 0 1 0 16 28 A12 12 0 1 0 16 4 Z",
            "M16 9 V16 L21 20",
        ],
        "user" | "users" => &[
            "M16 6 A5 5 0 1 0 16 16 A5 5 0 1 0 16 6 Z",
            "M6 28 C6 20 26 20 26 28",
        ],
        "cog" | "gear" | "settings" => &[
            "M16 11 A5 5 0 1 0 16 21 A5 5 0 1 0 16 11 Z",
            "M16 2 V7 M16 25 V30 M2 16 H7 M25 16 H30 M6 6 L9 9 M23 23 L26 26 M26 6 L23 9 M9 23 L6 26",
        ],
        "cloud" => {
            &["M9 24 C4 24 3 17 9 16 C9 11 16 9 18 14 C22 11 27 14 25 18 C30 19 28 24 24 24 Z"]
        }
        "database" | "db" => &[
            "M4 8 C4 4 28 4 28 8 L28 24 C28 28 4 28 4 24 Z",
            "M4 8 C4 12 28 12 28 8",
        ],
        "check" => &["M5 17 L13 25 L27 7"],
        "heart" => &["M16 27 C4 18 4 8 12 8 C15 8 16 11 16 11 C16 11 17 8 20 8 C28 8 28 18 16 27 Z"],
        // Unknown icon: a generic tag glyph rather than the raw class text.
        _ => &["M6 6 H20 L26 16 L20 26 H6 Z", "M11 12 A2 2 0 1 0 11 12.1 Z"],
    };
    let s = ICON_SIZE / 32.0;
    let x = cx - ICON_SIZE / 2.0;
    let _ = write!(
        svg.body,
        "<g transform=\"translate({x} {y}) scale({s})\" fill=\"none\" stroke=\"{stroke}\" stroke-width=\"2\" stroke-linejoin=\"round\" stroke-linecap=\"round\">",
        x = fnum(x),
        y = fnum(top_y),
        s = fnum(s),
    );
    for p in paths {
        let _ = write!(svg.body, "<path d=\"{p}\"/>");
    }
    svg.raw("</g>");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::MindmapNode;

    #[test]
    fn produces_svg() {
        let d = MindmapDiagram {
            class_defs: Default::default(),
            root: Some(MindmapNode {
                text: "root".into(),
                shape: MindmapShape::Circle,
                icon: None,
                classes: vec![],
                children: vec![MindmapNode {
                    text: "A".into(),
                    shape: MindmapShape::Rounded,
                    icon: Some("fa fa-book".into()),
                    classes: vec![],
                    children: vec![],
                }],
            }),
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">root<"));
        assert!(svg.contains(">A<"));
        // The raw Font Awesome class string must not leak into the output as text.
        assert!(!svg.contains("fa fa-book"));
        assert!(!svg.contains("fa-book"));
    }

    #[test]
    fn first_level_branches_split_left_and_right() {
        let leaf = |t: &str| MindmapNode {
            text: t.into(),
            shape: MindmapShape::Default,
            icon: None,
            classes: vec![],
            children: vec![],
        };
        let d = MindmapDiagram {
            class_defs: Default::default(),
            root: Some(MindmapNode {
                text: "root".into(),
                shape: MindmapShape::Circle,
                icon: None,
                classes: vec![],
                children: vec![leaf("Right"), leaf("Left")],
            }),
        };
        // Build the laid-out tree directly so we can inspect the sides.
        let root = d.root.clone().unwrap();
        let font_size = 14.0;
        let root_w = node_width(&root.text, font_size);
        let right = layout(&root.children[0], 1, font_size);
        let mut left = layout(&root.children[1], 1, font_size);
        mirror(&mut left, root_w);
        // The even-index branch grows right (dir +1, to the right of the root);
        // the odd-index branch is mirrored to the left (dir -1).
        assert_eq!(right.dir, 1.0);
        assert_eq!(left.dir, -1.0);
        assert!(right.x >= root_w, "right branch starts past the root");
        assert!(left.x + left.w <= 0.0, "left branch ends before the root");
        // Both labels still make it into the rendered document.
        let svg = render(&d, &Theme::default());
        assert!(svg.contains(">Right<") && svg.contains(">Left<"));
    }

    #[test]
    fn classdef_recolors_node() {
        use crate::parse::parse;
        let d = match parse(
            "mindmap\nroot(Root)\n  A[Node]\n  :::hot\nclassDef hot fill:#abc123,color:#ffffff\n",
        )
        .unwrap()
        {
            crate::parse::Diagram::Mindmap(m) => m,
            _ => panic!("not mindmap"),
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.contains("fill=\"#abc123\""));
        assert!(svg.contains("fill=\"#ffffff\""));
    }

    #[test]
    fn icon_name_extraction() {
        assert_eq!(icon_name("fa fa-book"), "book");
        assert_eq!(icon_name("fab fa-github"), "github");
        assert_eq!(icon_name("book"), "book");
        assert_eq!(icon_name(""), "");
    }
}
