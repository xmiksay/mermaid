//! Edge, node, shape and icon drawing for the laid-out mindmap tree.

use std::collections::HashMap;
use std::fmt::Write as _;

use crate::parse::ast::Style;
use crate::parse::MindmapShape;

use crate::svg::builder::{fnum, SvgBuilder};
use crate::svg::style::resolve_style;
use crate::svg::theme::Theme;

use super::{Laid, ICON_GAP, ICON_SIZE, NODE_PAD_X};

pub(super) fn draw_edges(laid: &Laid, svg: &mut SvgBuilder, theme: &Theme) {
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

/// Branch color for `section` from the categorical scale. Upstream's mindmap
/// section palette sits one slot past the generic scale (section 0 = `cScale1`,
/// …), giving the yellow/green/purple/magenta branch rotation; `-1`/root falls
/// back to `cScale0`.
fn branch_color(theme: &Theme, section: i32) -> String {
    theme
        .cscale_color((section + 1).max(0) as usize)
        .to_string()
}

pub(super) fn draw_nodes(
    laid: &Laid,
    svg: &mut SvgBuilder,
    theme: &Theme,
    class_defs: &HashMap<String, Style>,
) {
    let n = &laid.node;
    let rs = resolve_style(class_defs, &n.classes, &Style::new());
    let is_root = laid.depth == 0;

    // Defaults: the root is a solid bright-blue disc (the theme's saturated
    // primary lane color, matching upstream's blue root); every other node is a
    // borderless disc/rect filled in its branch color with a drop shadow.
    // Explicit `:::class` styling still overrides fill/stroke/label color.
    let default_fill = if is_root {
        theme.git_color(0).to_string()
    } else {
        branch_color(theme, laid.section)
    };
    let fill = rs.fill.clone().unwrap_or_else(|| default_fill.clone());
    let stroke = rs.stroke.clone().unwrap_or_else(|| "none".to_string());
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
            &format!(
                "fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"2\" filter=\"url(#mm-shadow)\""
            ),
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
            "text-anchor=\"middle\" fill=\"{fg}\" font-size=\"{}\"{}",
            fnum(laid.font),
            rs.text_attrs()
        ),
        &n.text,
    );

    for c in &laid.children {
        draw_nodes(c, svg, theme, class_defs);
    }
}

/// Draw a non-root node's borderless, drop-shadowed body centered on
/// `(laid.x, laid.y)`.
fn draw_shape(svg: &mut SvgBuilder, laid: &Laid, fill: &str, stroke: &str) {
    let (cx, cy) = (laid.x, laid.y);
    let (hw, hh) = (laid.w / 2.0, laid.h / 2.0);
    let x = cx - hw;
    let y = cy - hh;
    let base = format!("fill=\"{fill}\" stroke=\"{stroke}\" filter=\"url(#mm-shadow)\"");
    match laid.node.shape {
        // A bare `Default` node is a filled rounded rect like `Rounded` — never
        // the old thin-underline text (that was the "nodes unstyled" bug).
        MindmapShape::Default | MindmapShape::Rounded => {
            svg.rect(
                x,
                y,
                laid.w,
                laid.h,
                &format!("{base} stroke-width=\"2\" rx=\"8\""),
            );
        }
        MindmapShape::Square => {
            svg.rect(x, y, laid.w, laid.h, &format!("{base} stroke-width=\"2\""));
        }
        MindmapShape::Circle => {
            let r = hw.max(hh);
            svg.circle(cx, cy, r, &format!("{base} stroke-width=\"2\""));
        }
        MindmapShape::Bang => {
            svg.rect(
                x,
                y,
                laid.w,
                laid.h,
                &format!("{base} stroke-width=\"2.5\" stroke-dasharray=\"4 2\" rx=\"4\""),
            );
        }
        MindmapShape::Cloud => {
            svg.rect(
                x,
                y,
                laid.w,
                laid.h,
                &format!("{base} stroke-width=\"2\" rx=\"{}\"", fnum(hh)),
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
            svg.path(&d, &format!("{base} stroke-width=\"2\""));
        }
    }
}

/// Extract the meaningful name from a Font Awesome class string
/// (`fa fa-book` / `fab fa-github` / `book`) — the last `fa-`-prefixed token,
/// or the last token otherwise.
pub(super) fn icon_name(icon: &str) -> &str {
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

/// Whether `color` is dark enough to warrant white label text (perceptual
/// luminance below the midpoint). Non-hex colors are treated as light.
pub(super) fn is_dark(color: &str) -> bool {
    match parse_hex(color) {
        Some((r, g, b)) => (0.299 * r + 0.587 * g + 0.114 * b) < 140.0,
        None => false,
    }
}
