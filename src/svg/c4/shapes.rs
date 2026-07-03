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
    if matches!(el.kind, C4ElementKind::Person) {
        draw_person_icon(svg, x + w - 28.0, y + 6.0, &style.fill);
    }
    let _ = theme;
}

fn draw_person_icon(svg: &mut SvgBuilder, x: f64, y: f64, fill: &str) {
    let stroke = if is_dark_fill(fill) {
        "#FFFFFF"
    } else {
        "#0B2B4A"
    };
    use std::fmt::Write as _;
    let _ = write!(
        svg.body,
        "<g transform=\"translate({x} {y})\" fill=\"none\" stroke=\"{stroke}\" stroke-width=\"1.5\" stroke-linecap=\"round\" stroke-linejoin=\"round\">\
         <circle cx=\"11\" cy=\"6\" r=\"4\" fill=\"{stroke}\"/>\
         <path d=\"M2 22 C2 14 20 14 20 22\" fill=\"{stroke}\"/>\
         </g>",
        x = fnum(x),
        y = fnum(y),
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
    svg.rect(
        x,
        y,
        w,
        h,
        &format!(
            "fill=\"{fill}\" stroke=\"{border}\" stroke-width=\"1.5\" rx=\"6\"{dash}",
            dash = if el.external {
                " stroke-dasharray=\"5 3\""
            } else {
                ""
            }
        ),
    );
    write_label_block(svg, el, x, y, w, h, text_fill, muted);
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
    let dash = if el.external {
        " stroke-dasharray=\"5 3\""
    } else {
        ""
    };
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
        &format!("fill=\"{fill}\" stroke=\"{border}\" stroke-width=\"1.5\"{dash}"),
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
        &format!("fill=\"{fill}\" stroke=\"{border}\" stroke-width=\"1.5\"{dash}"),
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
        &format!("fill=\"none\" stroke=\"{border}\" stroke-width=\"1.5\"{dash}"),
    );
    write_label_block(svg, el, x, y + ry, w, h - 2.0 * ry, text_fill, muted);
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
    let dash = if el.external {
        " stroke-dasharray=\"5 3\""
    } else {
        ""
    };
    svg.rect(
        x,
        y,
        w,
        h,
        &format!(
            "fill=\"{fill}\" stroke=\"{border}\" stroke-width=\"1.5\" rx=\"{rx}\" ry=\"{rx}\"{dash}"
        ),
    );
    write_label_block(svg, el, x, y, w, h, text_fill, muted);
}

#[allow(clippy::too_many_arguments)]
fn write_label_block(
    svg: &mut SvgBuilder,
    el: &C4Element,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    fg: &str,
    muted: &str,
) {
    let cx = x + w / 2.0;
    let kind_label = kind_text(el.kind, el.external);
    let top = y + 6.0;
    svg.text(
        cx,
        top + 12.0,
        &format!("text-anchor=\"middle\" fill=\"{muted}\" font-size=\"10\" font-style=\"italic\""),
        kind_label,
    );
    let title_y = top + 32.0;
    svg.text(
        cx,
        title_y,
        &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"13\" font-weight=\"bold\""),
        &el.label,
    );
    let mut next_y = title_y + 16.0;
    if let Some(t) = &el.technology {
        svg.text(
            cx,
            next_y,
            &format!(
                "text-anchor=\"middle\" fill=\"{muted}\" font-size=\"10\" font-style=\"italic\""
            ),
            &format!("[{}]", t),
        );
        next_y += 14.0;
    }
    if let Some(d) = &el.descr {
        let max_chars = ((w - 16.0) / 6.2).max(8.0) as usize;
        let max_lines = (((y + h) - next_y - 4.0) / 12.0).max(1.0) as usize;
        let lines = wrap_text(d, max_chars, max_lines);
        for (i, line) in lines.iter().enumerate() {
            svg.text(
                cx,
                next_y + (i as f64) * 12.0,
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
