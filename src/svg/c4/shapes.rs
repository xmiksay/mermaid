//! C4 element shape primitives: boxes, cylinders, queues, the person icon, and
//! the palette / label helpers they share.

use crate::parse::{C4Element, C4ElementKind, C4ElementStyle};

use super::super::builder::{fnum, SvgBuilder};
use super::super::theme::Theme;

/// Effective colors for one element after applying any `UpdateElementStyle`
/// override on top of the built-in palette.
pub(super) struct ElementStyle {
    fill: String,
    border: String,
    text: String,
    muted: String,
}

pub(super) fn resolve_element_style(el: &C4Element, ov: Option<&C4ElementStyle>) -> ElementStyle {
    let (base_fill, base_border) = palette(el.kind, el.external);
    let fill = ov
        .and_then(|s| s.bg_color.clone())
        .unwrap_or_else(|| base_fill.to_string());
    let border = ov
        .and_then(|s| s.border_color.clone())
        .unwrap_or_else(|| base_border.to_string());
    let font = ov.and_then(|s| s.font_color.clone());
    let text = font
        .clone()
        .unwrap_or_else(|| text_color_for(&fill).to_string());
    let muted = font.unwrap_or_else(|| mute_text_color_for(&fill).to_string());
    ElementStyle {
        fill,
        border,
        text,
        muted,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_element(
    el: &C4Element,
    style: &ElementStyle,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    svg: &mut SvgBuilder,
    theme: &Theme,
) {
    match el.kind {
        C4ElementKind::SystemDb | C4ElementKind::ContainerDb | C4ElementKind::ComponentDb => {
            draw_cylinder(el, style, x, y, w, h, svg);
        }
        C4ElementKind::SystemQueue
        | C4ElementKind::ContainerQueue
        | C4ElementKind::ComponentQueue => draw_queue(el, style, x, y, w, h, svg),
        _ => draw_box(el, style, x, y, w, h, svg),
    }
    let _ = theme;
}

/// Height of the centered person figure (head + shoulders), reserved between the
/// stereotype and the name in a person box.
const PERSON_GLYPH_H: f64 = 30.0;

/// A large, horizontally centered person figure (head + shoulders), drawn in a
/// contrasting fill — matching upstream's centered glyph rather than a small
/// corner icon (#258). Upstream stacks it below the stereotype line (#327).
fn draw_person_icon(svg: &mut SvgBuilder, cx: f64, top: f64, fill: &str) {
    let color = if is_dark_fill(fill) {
        "#FFFFFF"
    } else {
        "#0B2B4A"
    };
    use std::fmt::Write as _;
    let _ = write!(
        svg.body,
        "<g fill=\"{color}\" stroke=\"none\">\
         <circle cx=\"{cx}\" cy=\"{head_y}\" r=\"8\"/>\
         <path d=\"M{lx} {by} C{lx} {ty} {rx} {ty} {rx} {by} Z\"/>\
         </g>",
        cx = fnum(cx),
        head_y = fnum(top + 8.0),
        lx = fnum(cx - 13.0),
        rx = fnum(cx + 13.0),
        ty = fnum(top + 16.0),
        by = fnum(top + 30.0),
    );
}

fn draw_box(
    el: &C4Element,
    style: &ElementStyle,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    svg: &mut SvgBuilder,
) {
    let fill = &style.fill;
    let border = &style.border;
    let text_fill = style.text.as_str();
    let muted = style.muted.as_str();
    // External elements are solid gray (differentiated by palette, not a dash).
    svg.rect(
        x,
        y,
        w,
        h,
        &format!("fill=\"{fill}\" stroke=\"{border}\" stroke-width=\"1.5\" rx=\"6\""),
    );
    write_label_block(svg, el, x, y, w, h, fill, text_fill, muted);
}

fn draw_cylinder(
    el: &C4Element,
    style: &ElementStyle,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    svg: &mut SvgBuilder,
) {
    let fill = &style.fill;
    let border = &style.border;
    let text_fill = style.text.as_str();
    let muted = style.muted.as_str();
    let rx = w / 2.0;
    let ry = 10.0;
    let top_y = y + ry;
    let bot_y = y + h - ry;
    svg.path(
        &format!(
            "M {lx} {top_y} L {lx} {bot_y} A {rx} {ry} 0 0 0 {rx_end} {bot_y} L {rx_end} {top_y}",
            lx = fnum(x),
            top_y = fnum(top_y),
            bot_y = fnum(bot_y),
            rx = fnum(rx),
            ry = fnum(ry),
            rx_end = fnum(x + w),
        ),
        &format!("fill=\"{fill}\" stroke=\"{border}\" stroke-width=\"1.5\""),
    );
    svg.path(
        &format!(
            "M {lx} {top_y} A {rx} {ry} 0 0 1 {rx_end} {top_y} A {rx} {ry} 0 0 1 {lx} {top_y} Z",
            lx = fnum(x),
            top_y = fnum(top_y),
            rx = fnum(rx),
            ry = fnum(ry),
            rx_end = fnum(x + w),
        ),
        &format!("fill=\"{fill}\" stroke=\"{border}\" stroke-width=\"1.5\""),
    );
    svg.path(
        &format!(
            "M {lx} {bot_y} A {rx} {ry} 0 0 0 {rx_end} {bot_y}",
            lx = fnum(x),
            bot_y = fnum(bot_y),
            rx = fnum(rx),
            ry = fnum(ry),
            rx_end = fnum(x + w),
        ),
        &format!("fill=\"none\" stroke=\"{border}\" stroke-width=\"1.5\""),
    );
    write_label_block(svg, el, x, y + ry, w, h - 2.0 * ry, fill, text_fill, muted);
}

fn draw_queue(
    el: &C4Element,
    style: &ElementStyle,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    svg: &mut SvgBuilder,
) {
    let fill = &style.fill;
    let border = &style.border;
    let text_fill = style.text.as_str();
    let muted = style.muted.as_str();
    let rx = h / 2.0;
    svg.rect(
        x,
        y,
        w,
        h,
        &format!(
            "fill=\"{fill}\" stroke=\"{border}\" stroke-width=\"1.5\" rx=\"{rx}\" ry=\"{rx}\""
        ),
    );
    write_label_block(svg, el, x, y, w, h, fill, text_fill, muted);
}

/// Stack a shape's text block top-down as upstream does: stereotype → (person
/// icon) → name → `[techn]` → description, with the description sinking toward
/// the bottom of the box instead of clustering under the name (#327).
#[allow(clippy::too_many_arguments)]
fn write_label_block(
    svg: &mut SvgBuilder,
    el: &C4Element,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    fill: &str,
    fg: &str,
    muted: &str,
) {
    let cx = x + w / 2.0;
    // Entity-encode the literal `<<…>>` stereotype so the inline-HTML label pass
    // doesn't read `<person>` as an (unknown, stripped) tag.
    let kind_label = kind_text(el.kind, el.external)
        .replace('<', "#lt;")
        .replace('>', "#gt;");
    let top = y + 6.0;
    svg.text(
        cx,
        top + 12.0,
        &format!("text-anchor=\"middle\" fill=\"{muted}\" font-size=\"10\" font-style=\"italic\""),
        &kind_label,
    );
    // A person's figure sits between the stereotype and the name.
    let mut next_y = top + 20.0;
    if matches!(el.kind, C4ElementKind::Person) {
        draw_person_icon(svg, cx, next_y, fill);
        next_y += PERSON_GLYPH_H + 4.0;
    }
    let title_y = next_y + 12.0;
    svg.text(
        cx,
        title_y,
        &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"13\" font-weight=\"bold\""),
        &el.label,
    );
    let mut next_y = title_y + 4.0;
    if let Some(t) = &el.technology {
        next_y += 14.0;
        svg.text(
            cx,
            next_y,
            &format!(
                "text-anchor=\"middle\" fill=\"{muted}\" font-size=\"10\" font-style=\"italic\""
            ),
            &format!("[{}]", t),
        );
    }
    if let Some(d) = &el.descr {
        let max_chars = ((w - 16.0) / 6.2).max(8.0) as usize;
        let bottom = (y + h) - 6.0;
        let max_lines = ((bottom - (next_y + 12.0)) / 12.0).max(1.0) as usize;
        let lines = wrap_text(d, max_chars, max_lines);
        // Sink the block so its last line rests near the bottom border, but never
        // overlap the name/tech above it.
        let block_h = lines.len() as f64 * 12.0;
        let first_baseline = (bottom - block_h + 10.0).max(next_y + 14.0);
        for (i, line) in lines.iter().enumerate() {
            svg.text(
                cx,
                first_baseline + (i as f64) * 12.0,
                &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"10\""),
                line,
            );
        }
    }
}

fn kind_text(kind: C4ElementKind, external: bool) -> &'static str {
    match (kind, external) {
        (C4ElementKind::Person, false) => "<<person>>",
        (C4ElementKind::Person, true) => "<<external_person>>",
        (C4ElementKind::System, false) => "<<system>>",
        (C4ElementKind::System, true) => "<<external_system>>",
        (C4ElementKind::SystemDb, false) => "<<system_db>>",
        (C4ElementKind::SystemDb, true) => "<<external_system_db>>",
        (C4ElementKind::SystemQueue, false) => "<<system_queue>>",
        (C4ElementKind::SystemQueue, true) => "<<external_system_queue>>",
        (C4ElementKind::Container, false) => "<<container>>",
        (C4ElementKind::Container, true) => "<<external_container>>",
        (C4ElementKind::ContainerDb, false) => "<<container_db>>",
        (C4ElementKind::ContainerDb, true) => "<<external_container_db>>",
        (C4ElementKind::ContainerQueue, false) => "<<container_queue>>",
        (C4ElementKind::ContainerQueue, true) => "<<external_container_queue>>",
        (C4ElementKind::Component, false) => "<<component>>",
        (C4ElementKind::Component, true) => "<<external_component>>",
        (C4ElementKind::ComponentDb, false) => "<<component_db>>",
        (C4ElementKind::ComponentDb, true) => "<<external_component_db>>",
        (C4ElementKind::ComponentQueue, false) => "<<component_queue>>",
        (C4ElementKind::ComponentQueue, true) => "<<external_component_queue>>",
        (C4ElementKind::Node, _) => "<<node>>",
    }
}

fn palette(kind: C4ElementKind, external: bool) -> (&'static str, &'static str) {
    if external {
        return ("#999999", "#6B6B6B");
    }
    match kind {
        C4ElementKind::Person => ("#08427B", "#073B6F"),
        C4ElementKind::System | C4ElementKind::SystemDb | C4ElementKind::SystemQueue => {
            ("#1168BD", "#0D5BA8")
        }
        C4ElementKind::Container | C4ElementKind::ContainerDb | C4ElementKind::ContainerQueue => {
            ("#438DD5", "#3A7DBE")
        }
        C4ElementKind::Component | C4ElementKind::ComponentDb | C4ElementKind::ComponentQueue => {
            ("#85BBF0", "#6FA8DC")
        }
        C4ElementKind::Node => ("#444444", "#2E2E2E"),
    }
}

fn text_color_for(fill: &str) -> &'static str {
    if is_dark_fill(fill) {
        "#FFFFFF"
    } else {
        "#0B2B4A"
    }
}

fn mute_text_color_for(fill: &str) -> &'static str {
    if is_dark_fill(fill) {
        "#D9E5F2"
    } else {
        "#3A5A7A"
    }
}

fn is_dark_fill(fill: &str) -> bool {
    if let Some(hex) = fill.strip_prefix('#') {
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f64;
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f64;
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f64;
            return (0.299 * r + 0.587 * g + 0.114 * b) < 140.0;
        }
    }
    false
}

fn wrap_text(s: &str, max_chars: usize, max_lines: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in s.split_whitespace() {
        if cur.is_empty() {
            cur.push_str(word);
            continue;
        }
        if cur.chars().count() + 1 + word.chars().count() <= max_chars {
            cur.push(' ');
            cur.push_str(word);
        } else {
            lines.push(std::mem::take(&mut cur));
            cur.push_str(word);
            if lines.len() >= max_lines {
                break;
            }
        }
    }
    if !cur.is_empty() && lines.len() < max_lines {
        lines.push(cur);
    }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
    }
    if let Some(last) = lines.last_mut() {
        if last.chars().count() > max_chars {
            let mut t: String = last.chars().take(max_chars.saturating_sub(1)).collect();
            t.push('…');
            *last = t;
        }
    }
    lines
}
