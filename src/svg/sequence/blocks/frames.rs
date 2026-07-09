//! Second pass: draw `rect <color>` background bands and block frames from the
//! collected events.

use std::collections::HashMap;

use crate::svg::builder::fnum;

use super::super::*;

/// Participant ids an event touches, appended to `out`. Used to size a frame or
/// band to only the participants involved in the messages it encloses (#123).
fn collect_ids(kind: &EventKind, out: &mut Vec<String>) {
    match kind {
        EventKind::Message { msg, .. } => {
            out.push(msg.from.clone());
            out.push(msg.to.clone());
        }
        EventKind::Note(n) => out.extend(n.participants.iter().cloned()),
        EventKind::Activate(id)
        | EventKind::Deactivate(id)
        | EventKind::Create(id)
        | EventKind::Destroy(id) => out.push(id.clone()),
        _ => {}
    }
}

/// `(min_x, max_x)` of the participants referenced by `events[range]`, or `None`
/// when the range touches no positioned participant.
fn extents(
    events: &[Event],
    range: std::ops::Range<usize>,
    x_of: &HashMap<String, f64>,
) -> Option<(f64, f64)> {
    let mut ids: Vec<String> = Vec::new();
    for ev in &events[range] {
        collect_ids(&ev.kind, &mut ids);
    }
    let xs: Vec<f64> = ids.iter().filter_map(|id| x_of.get(id).copied()).collect();
    if xs.is_empty() {
        return None;
    }
    Some((
        xs.iter().copied().fold(f64::INFINITY, f64::min),
        xs.iter().copied().fold(f64::NEG_INFINITY, f64::max),
    ))
}

/// Full-diagram extents, the fallback when a frame/band encloses no message.
fn all_extents(x_of: &HashMap<String, f64>) -> (f64, f64) {
    (
        x_of.values().copied().fold(f64::INFINITY, f64::min),
        x_of.values().copied().fold(f64::NEG_INFINITY, f64::max),
    )
}

/// Map each `BlockOpen` event index to its matching `BlockClose` index.
fn pair_blocks(events: &[Event]) -> HashMap<usize, usize> {
    let mut map = HashMap::new();
    let mut stack: Vec<usize> = Vec::new();
    for (i, ev) in events.iter().enumerate() {
        match ev.kind {
            EventKind::BlockOpen { .. } => stack.push(i),
            EventKind::BlockClose => {
                if let Some(open) = stack.pop() {
                    map.insert(open, i);
                }
            }
            _ => {}
        }
    }
    map
}

/// Draw `rect <color>` background bands. Bands nest strictly (LIFO), so a plain
/// stack of open `(index, y_top, color)` tuples matches each close to its open;
/// the band spans only the participants involved between the pair.
pub(in crate::svg::sequence) fn draw_rect_bands(
    svg: &mut SvgBuilder,
    events: &[Event],
    x_of: &HashMap<String, f64>,
) {
    if x_of.is_empty() {
        return;
    }
    let mut stack: Vec<(usize, f64, Option<String>)> = Vec::new();
    for (i, ev) in events.iter().enumerate() {
        match &ev.kind {
            EventKind::RectOpen { color } => stack.push((i, ev.y, color.clone())),
            EventKind::RectClose => {
                if let Some((open_i, y_top, color)) = stack.pop() {
                    let (min_x, max_x) =
                        extents(events, open_i + 1..i, x_of).unwrap_or_else(|| all_extents(x_of));
                    let fill = color.as_deref().unwrap_or("rgba(0,0,0,0.05)");
                    let x = min_x - 20.0;
                    svg.rect(
                        x,
                        y_top,
                        (max_x + 20.0) - x,
                        ev.y - y_top,
                        &format!("fill=\"{fill}\" stroke=\"none\""),
                    );
                }
            }
            _ => {}
        }
    }
}

pub(in crate::svg::sequence) fn draw_block_frames(
    svg: &mut SvgBuilder,
    events: &[Event],
    x_of: &HashMap<String, f64>,
    theme: &Theme,
    zenuml: bool,
) {
    let fg = &theme.fg;
    let close_of = pair_blocks(events);
    // Each open records its own participant extents so branch dividers (drawn
    // before the close) and the closing frame share the same span.
    let mut stack: Vec<(usize, BlockKind, String, f64, f64)> = Vec::new();
    for (i, ev) in events.iter().enumerate() {
        match &ev.kind {
            EventKind::BlockOpen { kind, label } => {
                let (min_x, max_x) = close_of
                    .get(&i)
                    .and_then(|&close| extents(events, i + 1..close, x_of))
                    .unwrap_or_else(|| all_extents(x_of));
                stack.push((i, *kind, label.clone(), min_x, max_x));
            }
            EventKind::BlockBranch { label } => {
                if let Some(&(_, _, _, min_x, max_x)) = stack.last() {
                    let y_branch = ev.y;
                    if zenuml {
                        // ZenUML separates the alternate (else/catch) compartment
                        // with a solid divider and a `[ … ]` guard row — the
                        // region is *not* shaded (issue #315).
                        svg.line(
                            min_x - 16.0,
                            y_branch,
                            max_x + 16.0,
                            y_branch,
                            "stroke=\"#666\" stroke-width=\"1\"",
                        );
                        if !label.is_empty() {
                            svg.text(
                                min_x - 12.0,
                                y_branch + 13.0,
                                &format!("fill=\"{fg}\" font-size=\"11\""),
                                &format!("[ {label} ]"),
                            );
                        }
                    } else {
                        let divider = &theme.actor_stroke;
                        svg.line(
                            min_x - 16.0,
                            y_branch,
                            max_x + 16.0,
                            y_branch,
                            &format!(
                                "stroke=\"{divider}\" stroke-width=\"1\" stroke-dasharray=\"2 2\""
                            ),
                        );
                        if !label.is_empty() {
                            svg.text(
                                (min_x + max_x) / 2.0,
                                y_branch - 4.0,
                                &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"11\""),
                                &format!("[{label}]"),
                            );
                        }
                    }
                }
            }
            EventKind::BlockClose => {
                if let Some((open_idx, kind, label, min_x, max_x)) = stack.pop() {
                    let y_top = events[open_idx].y;
                    let y_bot = ev.y;
                    draw_block_frame(svg, kind, &label, min_x, max_x, y_top, y_bot, theme, zenuml);
                }
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_block_frame(
    svg: &mut SvgBuilder,
    kind: BlockKind,
    label: &str,
    min_x: f64,
    max_x: f64,
    y_top: f64,
    y_bot: f64,
    theme: &Theme,
    zenuml: bool,
) {
    let fg = &theme.fg;
    let frame_label_fill = &theme.frame_label_fill;
    let frame_stroke = &theme.actor_stroke;
    let frame_x = min_x - 16.0;
    let frame_w = (max_x + 16.0) - frame_x;
    let frame_h = y_bot - y_top;
    let title = match kind {
        BlockKind::Alt => "alt",
        BlockKind::Par => "par",
        BlockKind::Critical => "critical",
        BlockKind::Loop => "loop",
        BlockKind::Opt => "opt",
        BlockKind::Break => "break",
    };
    if zenuml {
        // ZenUML: a solid frame whose header row carries a `◇ Operator` diamond
        // (no gray bar), a divider beneath it, and the guard `[ … ]` on its own
        // row below the header (issue #315).
        svg.rect(
            frame_x,
            y_top,
            frame_w,
            frame_h,
            "fill=\"none\" stroke=\"#666\" stroke-width=\"1\"",
        );
        svg.text(
            frame_x + 6.0,
            y_top + 13.0,
            &format!("fill=\"{fg}\" font-size=\"11\" font-weight=\"bold\""),
            // `◇ Alt` — the operator capitalized (`alt` → `Alt`, ASCII titles).
            &format!("\u{25c7} {}{}", title[..1].to_uppercase(), &title[1..]),
        );
        svg.line(
            frame_x,
            y_top + 18.0,
            frame_x + frame_w,
            y_top + 18.0,
            "stroke=\"#666\" stroke-width=\"1\"",
        );
        if !label.is_empty() {
            svg.text(
                frame_x + 8.0,
                y_top + 31.0,
                &format!("fill=\"{fg}\" font-size=\"11\""),
                &format!("[ {label} ]"),
            );
        }
        return;
    }
    // Dotted, theme-colored border (upstream's frame chrome).
    svg.rect(
        frame_x,
        y_top,
        frame_w,
        frame_h,
        &format!(
            "fill=\"none\" stroke=\"{frame_stroke}\" stroke-width=\"1\" stroke-dasharray=\"2 2\""
        ),
    );
    // Pentagon/flag label tab in the upper-left: a rectangle with the
    // bottom-right corner beveled (upstream's `.labelBox`).
    let tab_h = 18.0;
    let bevel = 7.0;
    let tab = format!(
        "M{x} {y}h{w}v{v}l-{b} {b}h-{hw}z",
        x = fnum(frame_x),
        y = fnum(y_top),
        w = fnum(BLOCK_LABEL_W),
        v = fnum(tab_h - bevel),
        b = fnum(bevel),
        hw = fnum(BLOCK_LABEL_W - bevel),
    );
    svg.path(
        &tab,
        &format!("fill=\"{frame_label_fill}\" stroke=\"{frame_stroke}\" stroke-width=\"1\""),
    );
    // Operator (`alt`/`loop`/…) in the tab: upstream's `.labelText` is regular
    // weight, not bold (#329).
    svg.text(
        frame_x + 6.0,
        y_top + 13.0,
        &format!("fill=\"{fg}\" font-size=\"11\""),
        title,
    );
    if !label.is_empty() {
        // Guard/condition text centered in the frame, black (upstream), not the
        // gray italic that used to sit beside the tab.
        svg.text(
            (min_x + max_x) / 2.0,
            y_top + 13.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"11\""),
            &format!("[{label}]"),
        );
    }
}
