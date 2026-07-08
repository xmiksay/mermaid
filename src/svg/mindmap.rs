//! Mindmap renderer. Deterministic radial tree fanning out from a central root
//! circle, matching upstream Mermaid's radial silhouette: branch-colored filled
//! rounded nodes and thick edges from the categorical theme scale.

use std::f64::consts::{FRAC_PI_2, PI};
use std::fmt::Write as _;

use std::collections::HashMap;

use crate::parse::ast::Style;
use crate::parse::{MindmapDiagram, MindmapNode, MindmapShape};

use super::builder::{fnum, SvgBuilder};
use super::metrics::text_width;
use super::style::resolve_style;
use super::theme::Theme;

const NODE_PAD_X: f64 = 14.0;
const NODE_H: f64 = 32.0;
/// Radius added per depth level; first ring sits this far from the centre.
const RING_GAP: f64 = 160.0;
const TEXT_PX: f64 = 7.0;
const ICON_SIZE: f64 = 16.0;
/// Gap between an in-node icon glyph and its label text.
const ICON_GAP: f64 = 6.0;

#[derive(Clone)]
struct Laid {
    node: MindmapNode,
    /// Node centre in layout space.
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    /// Radius of the root circle (only meaningful at `depth == 0`).
    r: f64,
    depth: usize,
    /// Index of the first-level branch this node belongs to (`-1` for the root),
    /// used to pick a branch color from the theme scale.
    section: i32,
    children: Vec<Laid>,
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

    let font_size = theme.font_size;
    // Root sits at the origin; its children are dealt around the full circle by
    // angular sector (proportional to each subtree's leaf count), and every
    // descendant is fanned outward within its parent's sector.
    let mut laid = build(&root, 0, -1, -FRAC_PI_2, -FRAC_PI_2 + 2.0 * PI, font_size);

    // Frame the whole radial layout and shift into positive space.
    let margin = 24.0;
    let (min_x, min_y, max_x, max_y) = bounds(&laid);
    shift(&mut laid, margin - min_x, margin - min_y);
    let width = (max_x - min_x) + margin * 2.0;
    let height = (max_y - min_y) + margin * 2.0;

    let mut svg = SvgBuilder::new(width, height).theme(theme);

    draw_edges(&laid, &mut svg, theme);
    draw_nodes(&laid, &mut svg, theme, &d.class_defs);

    svg.finish()
}

/// Leaves in a subtree (a leaf counts as one) — the angular weight of a node.
fn leaves(n: &MindmapNode) -> usize {
    if n.children.is_empty() {
        1
    } else {
        n.children.iter().map(leaves).sum()
    }
}

fn node_size(n: &MindmapNode, font_size: f64) -> (f64, f64) {
    let icon_w = if n.icon.is_some() {
        ICON_SIZE + ICON_GAP
    } else {
        0.0
    };
    let tw = text_width(&n.text, TEXT_PX, font_size);
    let w = (tw + NODE_PAD_X * 2.0 + icon_w).max(48.0);
    (w, NODE_H)
}

/// Build the laid-out subtree for `n`, placing it at the centre of the angular
/// sector `[a0, a1)` at radius `depth * RING_GAP` and recursing on its children.
fn build(n: &MindmapNode, depth: usize, section: i32, a0: f64, a1: f64, font_size: f64) -> Laid {
    let angle = (a0 + a1) / 2.0;
    let (w, h) = node_size(n, font_size);
    let r = depth as f64 * RING_GAP;
    let (x, y) = (r * angle.cos(), r * angle.sin());
    let root_r = if depth == 0 {
        (text_width(&n.text, TEXT_PX, font_size) / 2.0 + NODE_PAD_X + 6.0).max(28.0)
    } else {
        0.0
    };

    let total = leaves(n).max(1) as f64;
    let mut cursor = a0;
    let mut children = Vec::with_capacity(n.children.len());
    for (i, c) in n.children.iter().enumerate() {
        let span = (a1 - a0) * (leaves(c) as f64) / total;
        let child_section = if depth == 0 { i as i32 } else { section };
        children.push(build(
            c,
            depth + 1,
            child_section,
            cursor,
            cursor + span,
            font_size,
        ));
        cursor += span;
    }

    Laid {
        node: n.clone(),
        x,
        y,
        w,
        h,
        r: root_r,
        depth,
        section,
        children,
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
    let (hw, hh) = if laid.depth == 0 {
        (laid.r, laid.r)
    } else {
        (laid.w / 2.0, laid.h / 2.0)
    };
    let mut min_x = laid.x - hw;
    let mut max_x = laid.x + hw;
    let mut min_y = laid.y - hh;
    let mut max_y = laid.y + hh;
    for c in &laid.children {
        let (a, b, cc, dd) = bounds(c);
        min_x = min_x.min(a);
        min_y = min_y.min(b);
        max_x = max_x.max(cc);
        max_y = max_y.max(dd);
    }
    (min_x, min_y, max_x, max_y)
}

fn draw_edges(laid: &Laid, svg: &mut SvgBuilder, theme: &Theme) {
    for c in &laid.children {
        // Thick spoke in the child's branch color, tapering with depth so the
        // trunks near the root read heavier than the twigs (upstream taper).
        let color = branch_color(theme, c.section);
        let sw = (8.0 - 2.0 * (c.depth as f64 - 1.0)).max(2.0);
        let mut path = String::new();
        let _ = write!(
            path,
            "M{} {}L{} {}",
            fnum(laid.x),
            fnum(laid.y),
            fnum(c.x),
            fnum(c.y),
        );
        svg.path(
            &path,
            &format!(
                "fill=\"none\" stroke=\"{color}\" stroke-width=\"{}\" stroke-linecap=\"round\"",
                fnum(sw)
            ),
        );
        draw_edges(c, svg, theme);
    }
}

/// Branch color for `section` from the categorical scale (`-1`/root falls back
/// to slot 0).
fn branch_color(theme: &Theme, section: i32) -> String {
    theme.cscale_color(section.max(0) as usize).to_string()
}

fn draw_nodes(
    laid: &Laid,
    svg: &mut SvgBuilder,
    theme: &Theme,
    class_defs: &HashMap<String, Style>,
) {
    let n = &laid.node;
    let rs = resolve_style(class_defs, &n.classes, &Style::new());
    let is_root = laid.depth == 0;

    // Defaults: the root is a solid dark disc; every other node is filled in its
    // branch color with a slightly darker border. Explicit `:::class` styling
    // still overrides fill/stroke/label color.
    let default_fill = if is_root {
        darken(&theme.flow_node_stroke, 0.45)
    } else {
        branch_color(theme, laid.section)
    };
    let fill = rs.fill.clone().unwrap_or_else(|| default_fill.clone());
    let stroke = rs.stroke.clone().unwrap_or_else(|| darken(&fill, 0.22));
    let default_text = if is_dark(&fill) {
        "#ffffff".to_string()
    } else {
        theme.fg.to_string()
    };
    let fg = rs.label_fill(&default_text).to_string();

    let cx = laid.x;
    let cy = laid.y;
    if is_root {
        svg.circle(
            cx,
            cy,
            laid.r,
            &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"3\""),
        );
    } else {
        draw_shape(svg, laid, &fill, &stroke);
    }

    // Icon (if any) sits at the left inside the node; the label is centered in
    // the remaining width so the two never overlap.
    let mut text_cx = cx;
    if let Some(icon) = &n.icon {
        let icon_cx = cx - laid.w / 2.0 + NODE_PAD_X + ICON_SIZE / 2.0;
        draw_mindmap_icon(svg, icon, icon_cx, cy, &fg);
        text_cx = cx + (ICON_SIZE + ICON_GAP) / 2.0;
    }

    svg.text(
        text_cx,
        cy + 4.0,
        &format!(
            "text-anchor=\"middle\" fill=\"{fg}\" font-size=\"13\"{}",
            rs.text_attrs()
        ),
        &n.text,
    );

    for c in &laid.children {
        draw_nodes(c, svg, theme, class_defs);
    }
}

/// Draw a non-root node's outline centered on `(laid.x, laid.y)`.
fn draw_shape(svg: &mut SvgBuilder, laid: &Laid, fill: &str, stroke: &str) {
    let (cx, cy) = (laid.x, laid.y);
    let (hw, hh) = (laid.w / 2.0, laid.h / 2.0);
    let x = cx - hw;
    let y = cy - hh;
    match laid.node.shape {
        // A bare `Default` node is a filled rounded rect like `Rounded` — never
        // the old thin-underline text (that was the "nodes unstyled" bug).
        MindmapShape::Default | MindmapShape::Rounded => {
            svg.rect(
                x,
                y,
                laid.w,
                laid.h,
                &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"2\" rx=\"8\""),
            );
        }
        MindmapShape::Square => {
            svg.rect(
                x,
                y,
                laid.w,
                laid.h,
                &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"2\""),
            );
        }
        MindmapShape::Circle => {
            let r = hw.max(hh);
            svg.circle(
                cx,
                cy,
                r,
                &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"2\""),
            );
        }
        MindmapShape::Bang => {
            svg.rect(
                x,
                y,
                laid.w,
                laid.h,
                &format!(
                    "fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"2.5\" stroke-dasharray=\"4 2\" rx=\"4\""
                ),
            );
        }
        MindmapShape::Cloud => {
            svg.rect(
                x,
                y,
                laid.w,
                laid.h,
                &format!(
                    "fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"2\" rx=\"{}\"",
                    fnum(hh)
                ),
            );
        }
        MindmapShape::Hexagon => {
            let d = format!(
                "M{l} {c}L{a} {t}L{b} {t}L{r} {c}L{b} {bb}L{a} {bb}Z",
                l = fnum(x),
                r = fnum(x + laid.w),
                t = fnum(cy - hh),
                bb = fnum(cy + hh),
                c = fnum(cy),
                a = fnum(x + hh),
                b = fnum(x + laid.w - hh),
            );
            svg.path(
                &d,
                &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"2\""),
            );
        }
    }
}

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

/// Draw a small builtin glyph for `icon` centered at `(cx, cy)`. Real Font
/// Awesome glyphs aren't available without the font, so the class string is
/// mapped onto a builtin path set rather than printing the raw `fa fa-book`.
fn draw_mindmap_icon(svg: &mut SvgBuilder, icon: &str, cx: f64, cy: f64, stroke: &str) {
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
    let y = cy - ICON_SIZE / 2.0;
    let _ = write!(
        svg.body,
        "<g transform=\"translate({x} {y}) scale({s})\" fill=\"none\" stroke=\"{stroke}\" stroke-width=\"2\" stroke-linejoin=\"round\" stroke-linecap=\"round\">",
        x = fnum(x),
        y = fnum(y),
        s = fnum(s),
    );
    for p in paths {
        let _ = write!(svg.body, "<path d=\"{p}\"/>");
    }
    svg.raw("</g>");
}

/// Parse `#rgb`/`#rrggbb` into 0..255 channels, or `None` for named colors.
fn parse_hex(s: &str) -> Option<(f64, f64, f64)> {
    let h = s.trim().strip_prefix('#')?;
    let (r, g, b) = match h.len() {
        6 => (&h[0..2], &h[2..4], &h[4..6]),
        3 => (&h[0..1], &h[1..2], &h[2..3]),
        _ => return None,
    };
    let dup = h.len() == 3;
    let ch = |c: &str| {
        let v = u8::from_str_radix(c, 16).ok()? as f64;
        Some(if dup { v * 17.0 } else { v })
    };
    Some((ch(r)?, ch(g)?, ch(b)?))
}

/// Darken a hex color toward black by `f` (0..1); non-hex colors pass through.
fn darken(color: &str, f: f64) -> String {
    match parse_hex(color) {
        Some((r, g, b)) => {
            let k = 1.0 - f;
            format!(
                "#{:02x}{:02x}{:02x}",
                (r * k).round() as u8,
                (g * k).round() as u8,
                (b * k).round() as u8,
            )
        }
        None => color.to_string(),
    }
}

/// Whether `color` is dark enough to warrant white label text (perceptual
/// luminance below the midpoint). Non-hex colors are treated as light.
fn is_dark(color: &str) -> bool {
    match parse_hex(color) {
        Some((r, g, b)) => (0.299 * r + 0.587 * g + 0.114 * b) < 140.0,
        None => false,
    }
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
    fn radial_layout_fans_children_around_root() {
        let leaf = |t: &str| MindmapNode {
            text: t.into(),
            shape: MindmapShape::Default,
            icon: None,
            classes: vec![],
            children: vec![],
        };
        let root = MindmapNode {
            text: "root".into(),
            shape: MindmapShape::Circle,
            icon: None,
            classes: vec![],
            children: vec![leaf("A"), leaf("B"), leaf("C"), leaf("D")],
        };
        let laid = build(&root, 0, -1, -FRAC_PI_2, -FRAC_PI_2 + 2.0 * PI, 14.0);
        // The root sits at the origin; every child sits on the first ring.
        assert_eq!((laid.x, laid.y), (0.0, 0.0));
        for c in &laid.children {
            let r = (c.x * c.x + c.y * c.y).sqrt();
            assert!((r - RING_GAP).abs() < 1e-6, "child off the first ring");
        }
        // Four evenly-fanned branches must not all sit on one side of the root:
        // some grow to the right (x>0) and some to the left (x<0).
        assert!(laid.children.iter().any(|c| c.x > 1.0));
        assert!(laid.children.iter().any(|c| c.x < -1.0));
        // First-level branches carry their own section index.
        let sections: Vec<i32> = laid.children.iter().map(|c| c.section).collect();
        assert_eq!(sections, vec![0, 1, 2, 3]);
    }

    #[test]
    fn descendants_inherit_branch_section() {
        let d = match crate::parse::parse(
            "mindmap\nroot((R))\n  Branch\n    Child\n      Grandchild\n",
        )
        .unwrap()
        {
            crate::parse::Diagram::Mindmap(m) => m,
            _ => panic!("not mindmap"),
        };
        let root = d.root.clone().unwrap();
        let laid = build(&root, 0, -1, -FRAC_PI_2, -FRAC_PI_2 + 2.0 * PI, 14.0);
        let branch = &laid.children[0];
        assert_eq!(branch.section, 0);
        assert_eq!(branch.children[0].section, 0);
        assert_eq!(branch.children[0].children[0].section, 0);
    }

    #[test]
    fn icon_attaches_to_annotated_node() {
        // The book icon annotates `Mindmap`, the clock annotates `Gantt`; each
        // glyph must render inside its own node, not float onto a sibling.
        let d = match crate::parse::parse(
            "mindmap\nroot((R))\n  Diagrams\n    Mindmap\n      ::icon(fa fa-book)\n    Gantt\n      ::icon(fa fa-clock)\n",
        )
        .unwrap()
        {
            crate::parse::Diagram::Mindmap(m) => m,
            _ => panic!("not mindmap"),
        };
        let diagrams = &d.root.as_ref().unwrap().children[0];
        assert_eq!(diagrams.children[0].text, "Mindmap");
        assert_eq!(diagrams.children[0].icon.as_deref(), Some("fa fa-book"));
        assert_eq!(diagrams.children[1].text, "Gantt");
        assert_eq!(diagrams.children[1].icon.as_deref(), Some("fa fa-clock"));
        // Both glyphs are drawn, and no raw class string leaks.
        let svg = render(&d, &Theme::default());
        assert!(!svg.contains("fa-book"));
        assert!(!svg.contains("fa-clock"));
    }

    #[test]
    fn branch_nodes_are_filled_from_the_scale() {
        let d = match crate::parse::parse("mindmap\nroot((R))\n  First\n  Second\n").unwrap() {
            crate::parse::Diagram::Mindmap(m) => m,
            _ => panic!("not mindmap"),
        };
        let theme = Theme::default();
        let svg = render(&d, &theme);
        // Nodes are filled rounded rects in the branch (cScale) colors, not bare
        // underlined text.
        assert!(svg.contains(&format!("fill=\"{}\"", theme.cscale_color(0))));
        assert!(svg.contains(&format!("fill=\"{}\"", theme.cscale_color(1))));
        assert!(svg.contains("rx=\"8\""));
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

    #[test]
    fn color_helpers() {
        assert_eq!(darken("#ffffff", 0.5), "#808080");
        assert_eq!(darken("#B9B9FF", 0.0), "#b9b9ff");
        // Non-hex passes through untouched.
        assert_eq!(darken("red", 0.5), "red");
        assert!(is_dark("#000000"));
        assert!(!is_dark("#ffffff"));
        assert!(!is_dark("#B9B9FF"));
    }
}
