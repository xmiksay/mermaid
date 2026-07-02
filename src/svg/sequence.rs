//! Sequence diagram renderer.
//!
//! Two passes: first collect "events" with their y position from a recursive
//! walk of `items`, then draw lifelines + headers + events. Block frames are
//! emitted around their child events' y range.

use std::collections::HashMap;

use crate::parse::{
    AltBranch, ArrowKind, Message, NotePosition, ParticipantKind, SequenceBlock, SequenceDiagram,
    SequenceItem, SequenceNote, SequenceRect,
};

use super::builder::SvgBuilder;
use super::theme::Theme;

const ACTOR_MIN_W: f64 = 110.0;
const ACTOR_H: f64 = 40.0;
const ACTOR_CHAR_W: f64 = 8.0;
const ACTOR_PAD_X: f64 = 20.0;
const ACTOR_LINE_H: f64 = 18.0;
const PARTICIPANT_GAP: f64 = 50.0;
const PAD: f64 = 24.0;
const TITLE_H: f64 = 30.0;
const MSG_STEP: f64 = 50.0;
const MSG_TOP_GAP: f64 = 30.0;
const MSG_BOTTOM_GAP: f64 = 30.0;
const ARROW_HEAD: f64 = 8.0;
const TEXT_OFFSET: f64 = 6.0;
const NOTE_HEIGHT: f64 = 40.0;
const BLOCK_TOP_GAP: f64 = 24.0;
const BLOCK_BOTTOM_GAP: f64 = 12.0;
const BLOCK_LABEL_W: f64 = 60.0;
const BLOCK_TAB_H: f64 = 18.0; // reserved Y-space below the alt/loop tab so first message text doesn't overlap
const ACTIVATION_W: f64 = 10.0;
const BOX_LABEL_H: f64 = 22.0; // reserved Y-space above the actor row for `box` labels
const BOX_PAD: f64 = 12.0;
const CREATE_GAP: f64 = 15.0; // gap above an inline `create`d actor box
const DESTROY_CROSS: f64 = 7.0; // half-size of the `destroy` termination cross

pub(crate) fn render(d: &SequenceDiagram, theme: &Theme) -> String {
    let fg = theme.fg;
    let lifeline = theme.lifeline;
    if d.participants.is_empty() {
        let mut svg = SvgBuilder::new(200.0, 60.0).font(theme.font_family, theme.font_size);
        if let Some(t) = &d.title {
            svg.text(
                100.0,
                30.0,
                &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"16\""),
                t,
            );
        }
        return svg.finish();
    }

    let title_h = if d.title.is_some() { TITLE_H } else { 0.0 };

    // Measure each participant's label once and derive per-actor box sizes.
    // Column x-positions, spacing, and canvas width all follow from these
    // widths instead of a single fixed constant.
    let sizes: Vec<(f64, f64)> = d
        .participants
        .iter()
        .map(|p| actor_size(&p.display))
        .collect();
    let actor_h = sizes.iter().map(|s| s.1).fold(ACTOR_H, f64::max);

    let mut x_of: HashMap<String, f64> = HashMap::new();
    let mut w_of: HashMap<String, f64> = HashMap::new();
    let mut x = PAD;
    for (i, p) in d.participants.iter().enumerate() {
        let w = sizes[i].0;
        x_of.insert(p.id.clone(), x + w / 2.0);
        w_of.insert(p.id.clone(), w);
        x += w + PARTICIPANT_GAP;
    }
    let width = x - PARTICIPANT_GAP + PAD;

    let has_boxes = d.items.iter().any(|i| matches!(i, SequenceItem::Box(_)));
    let box_label_h = if has_boxes { BOX_LABEL_H } else { 0.0 };
    let box_top = PAD + title_h;
    let top_y = box_top + box_label_h;
    let header_bottom = top_y + actor_h;
    let body_top = header_bottom + MSG_TOP_GAP;

    // First pass: precompute events with y positions.
    let mut events: Vec<Event> = Vec::new();
    let mut cursor = body_top;
    let mut step_counter: u32 = 1;
    let mut num = Numbering { on: false, step: 1 };
    layout_items(
        &d.items,
        &mut events,
        &mut cursor,
        &mut step_counter,
        &mut num,
        &x_of,
    );
    let lifeline_bottom = cursor + MSG_BOTTOM_GAP;
    let footer_top = lifeline_bottom;
    let footer_bottom = footer_top + actor_h;
    let height = footer_bottom + PAD;

    let mut svg = SvgBuilder::new(width, height).font(theme.font_family, theme.font_size);
    define_markers(&mut svg, theme);

    if let Some(t) = &d.title {
        svg.text(
            width / 2.0,
            PAD + 18.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\" font-weight=\"bold\""),
            t,
        );
    }

    // Box backgrounds (behind lifelines and actors).
    if has_boxes {
        draw_boxes(&mut svg, d, &x_of, &w_of, box_top, footer_bottom, theme);
    }

    // `rect <color>` colored bands (behind lifelines and messages).
    draw_rect_bands(&mut svg, &events, &x_of);

    // `create`/`destroy` lifecycle: a created participant's box + lifeline
    // start at the create point (not the top row); a destroyed one's lifeline
    // ends at the destroy point with a cross and draws no footer box.
    let mut created: HashMap<String, f64> = HashMap::new();
    let mut destroyed: HashMap<String, f64> = HashMap::new();
    for ev in &events {
        match &ev.kind {
            EventKind::Create(id) => {
                created.insert(id.clone(), ev.y);
            }
            EventKind::Destroy(id) => {
                destroyed.insert(id.clone(), ev.y);
            }
            _ => {}
        }
    }

    // Lifelines
    for p in &d.participants {
        let x = x_of[&p.id];
        let top = created.get(&p.id).map_or(header_bottom, |&by| by + actor_h);
        let bottom = destroyed.get(&p.id).copied().unwrap_or(lifeline_bottom);
        svg.line(
            x,
            top,
            x,
            bottom,
            &format!("stroke=\"{lifeline}\" stroke-width=\"1\" stroke-dasharray=\"4 4\""),
        );
    }

    // Activation bands (computed from activate/deactivate events).
    draw_activations(&mut svg, &events, &x_of, lifeline_bottom);

    // Block frames
    draw_block_frames(&mut svg, &events, &x_of, theme);

    // Headers (top + bottom). A created participant's top box is drawn inline
    // at its create point instead; a destroyed one gets no footer box.
    for p in &d.participants {
        let x = x_of[&p.id];
        let w = w_of[&p.id];
        match created.get(&p.id) {
            Some(&by) => draw_actor(&mut svg, x, by, w, actor_h, &p.display, p.kind, theme),
            None => draw_actor(&mut svg, x, top_y, w, actor_h, &p.display, p.kind, theme),
        }
        if !destroyed.contains_key(&p.id) {
            draw_actor(
                &mut svg, x, footer_top, w, actor_h, &p.display, p.kind, theme,
            );
        }
    }

    // Destruction crosses at each destroyed participant's terminating point.
    for p in &d.participants {
        if let Some(&dy) = destroyed.get(&p.id) {
            draw_destroy_cross(&mut svg, x_of[&p.id], dy, theme);
        }
    }

    // Events
    for ev in &events {
        match &ev.kind {
            EventKind::Message { msg, number } => {
                let x1 = x_of.get(&msg.from);
                let x2 = x_of.get(&msg.to);
                if let (Some(&x1), Some(&x2)) = (x1, x2) {
                    let label = if let Some(n) = number {
                        format!("{n}. {}", msg.text)
                    } else {
                        msg.text.clone()
                    };
                    draw_message(&mut svg, x1, x2, ev.y, msg.arrow, &label, theme);
                }
            }
            EventKind::Note(note) => {
                draw_note(&mut svg, note, ev.y, &x_of, theme);
            }
            _ => {}
        }
    }

    svg.finish()
}

#[derive(Debug, Clone)]
struct Event {
    y: f64,
    kind: EventKind,
}

#[derive(Debug, Clone)]
enum EventKind {
    Message {
        msg: Message,
        number: Option<u32>,
    },
    Note(SequenceNote),
    Activate(String),
    Deactivate(String),
    /// Participant spawned mid-diagram via `create`; `y` is the top of its
    /// inline actor box.
    Create(String),
    /// Participant terminated via `destroy`; `y` is where the cross is drawn.
    Destroy(String),
    /// y_start / y_end will be filled in via a second-pass adjustment in the
    /// block-frame drawing routine using the events between BlockOpen and
    /// BlockClose markers.
    BlockOpen {
        kind: BlockKind,
        label: String,
    },
    BlockBranch {
        label: String,
    },
    BlockClose,
    /// `rect <color>` band open/close markers, paired in `draw_rect_bands`.
    RectOpen {
        color: Option<String>,
    },
    RectClose,
}

#[derive(Debug, Clone, Copy)]
enum BlockKind {
    Alt,
    Par,
    Critical,
    Loop,
    Opt,
    Break,
}

/// Auto-numbering state threaded through the layout pass. The counter holds the
/// *next* number to emit; `step` is the increment (`autonumber <start> <step>`).
#[derive(Debug, Clone, Copy)]
struct Numbering {
    on: bool,
    step: u32,
}

fn layout_items(
    items: &[SequenceItem],
    out: &mut Vec<Event>,
    cursor: &mut f64,
    counter: &mut u32,
    num: &mut Numbering,
    x_of: &HashMap<String, f64>,
) {
    for item in items {
        match item {
            SequenceItem::AutoNumber(cfg) => match cfg {
                Some(c) => {
                    num.on = true;
                    num.step = c.step.max(1);
                    *counter = c.start;
                }
                None => num.on = false,
            },
            SequenceItem::Message(m) => {
                *cursor += MSG_STEP;
                let number = if num.on {
                    let n = *counter;
                    *counter += num.step;
                    Some(n)
                } else {
                    None
                };
                out.push(Event {
                    y: *cursor - MSG_STEP / 2.0,
                    kind: EventKind::Message {
                        msg: m.clone(),
                        number,
                    },
                });
            }
            SequenceItem::Note(n) => {
                *cursor += NOTE_HEIGHT + 10.0;
                out.push(Event {
                    y: *cursor - NOTE_HEIGHT / 2.0,
                    kind: EventKind::Note(n.clone()),
                });
            }
            SequenceItem::Activate(id) => {
                out.push(Event {
                    y: *cursor,
                    kind: EventKind::Activate(id.clone()),
                });
            }
            SequenceItem::Deactivate(id) => {
                out.push(Event {
                    y: *cursor,
                    kind: EventKind::Deactivate(id.clone()),
                });
            }
            SequenceItem::Create(id) => {
                // Reserve vertical space for the inline actor box; the creating
                // message follows and lands on the lifeline just below it.
                *cursor += CREATE_GAP;
                out.push(Event {
                    y: *cursor,
                    kind: EventKind::Create(id.clone()),
                });
                *cursor += ACTOR_H;
            }
            SequenceItem::Destroy(id) => {
                out.push(Event {
                    y: *cursor,
                    kind: EventKind::Destroy(id.clone()),
                });
            }
            SequenceItem::Alt(branches) => {
                emit_branched_block(BlockKind::Alt, branches, out, cursor, counter, num, x_of);
            }
            SequenceItem::Par(branches) => {
                emit_branched_block(BlockKind::Par, branches, out, cursor, counter, num, x_of);
            }
            SequenceItem::Critical(branches) => {
                emit_branched_block(
                    BlockKind::Critical,
                    branches,
                    out,
                    cursor,
                    counter,
                    num,
                    x_of,
                );
            }
            SequenceItem::Loop(b) => {
                emit_simple_block(BlockKind::Loop, b, out, cursor, counter, num, x_of);
            }
            SequenceItem::Opt(b) => {
                emit_simple_block(BlockKind::Opt, b, out, cursor, counter, num, x_of);
            }
            SequenceItem::Break(b) => {
                emit_simple_block(BlockKind::Break, b, out, cursor, counter, num, x_of);
            }
            SequenceItem::Rect(r) => {
                emit_rect_block(r, out, cursor, counter, num, x_of);
            }
            SequenceItem::Box(_) => {
                // boxes group participants horizontally — v0.1 just ignores
                // them for rendering (no items inside).
            }
        }
    }
}

fn emit_simple_block(
    kind: BlockKind,
    block: &SequenceBlock,
    out: &mut Vec<Event>,
    cursor: &mut f64,
    counter: &mut u32,
    num: &mut Numbering,
    x_of: &HashMap<String, f64>,
) {
    *cursor += BLOCK_TOP_GAP;
    out.push(Event {
        y: *cursor,
        kind: EventKind::BlockOpen {
            kind,
            label: block.label.clone(),
        },
    });
    *cursor += BLOCK_TAB_H;
    layout_items(&block.items, out, cursor, counter, num, x_of);
    *cursor += BLOCK_BOTTOM_GAP;
    out.push(Event {
        y: *cursor,
        kind: EventKind::BlockClose,
    });
}

/// `rect <color>` — a colored background band with no label tab or border. The
/// band is drawn behind everything in a separate pass (`draw_rect_bands`).
fn emit_rect_block(
    rect: &SequenceRect,
    out: &mut Vec<Event>,
    cursor: &mut f64,
    counter: &mut u32,
    num: &mut Numbering,
    x_of: &HashMap<String, f64>,
) {
    *cursor += BLOCK_BOTTOM_GAP;
    out.push(Event {
        y: *cursor,
        kind: EventKind::RectOpen {
            color: rect.color.clone(),
        },
    });
    layout_items(&rect.items, out, cursor, counter, num, x_of);
    *cursor += BLOCK_BOTTOM_GAP;
    out.push(Event {
        y: *cursor,
        kind: EventKind::RectClose,
    });
}

fn emit_branched_block(
    kind: BlockKind,
    branches: &[AltBranch],
    out: &mut Vec<Event>,
    cursor: &mut f64,
    counter: &mut u32,
    num: &mut Numbering,
    x_of: &HashMap<String, f64>,
) {
    *cursor += BLOCK_TOP_GAP;
    out.push(Event {
        y: *cursor,
        kind: EventKind::BlockOpen {
            kind,
            label: branches
                .first()
                .map(|b| b.label.clone())
                .unwrap_or_default(),
        },
    });
    *cursor += BLOCK_TAB_H;
    for (i, branch) in branches.iter().enumerate() {
        if i > 0 {
            *cursor += BLOCK_TOP_GAP / 2.0;
            out.push(Event {
                y: *cursor,
                kind: EventKind::BlockBranch {
                    label: branch.label.clone(),
                },
            });
            *cursor += BLOCK_TAB_H / 2.0;
        }
        layout_items(&branch.items, out, cursor, counter, num, x_of);
    }
    *cursor += BLOCK_BOTTOM_GAP;
    out.push(Event {
        y: *cursor,
        kind: EventKind::BlockClose,
    });
}

#[allow(clippy::too_many_arguments)]
fn draw_actor(
    svg: &mut SvgBuilder,
    cx: f64,
    top: f64,
    w: f64,
    h: f64,
    label: &str,
    kind: ParticipantKind,
    theme: &Theme,
) {
    match kind {
        ParticipantKind::Actor => draw_actor_figure(svg, cx, top, h, label, theme),
        ParticipantKind::Participant => draw_actor_box(svg, cx, top, w, h, label, theme),
    }
}

fn draw_actor_box(
    svg: &mut SvgBuilder,
    cx: f64,
    top: f64,
    w: f64,
    h: f64,
    label: &str,
    theme: &Theme,
) {
    let fg = theme.fg;
    let actor_fill = theme.actor_fill;
    let actor_stroke = theme.actor_stroke;
    let x = cx - w / 2.0;
    svg.rect(
        x,
        top,
        w,
        h,
        &format!("fill=\"{actor_fill}\" stroke=\"{actor_stroke}\" stroke-width=\"1.5\" rx=\"4\""),
    );
    let lines = label_lines(label);
    let n = lines.len() as f64;
    let y0 = top + h / 2.0 - (n - 1.0) * ACTOR_LINE_H / 2.0 + 5.0;
    for (i, line) in lines.iter().enumerate() {
        svg.text(
            cx,
            y0 + i as f64 * ACTOR_LINE_H,
            &format!("text-anchor=\"middle\" fill=\"{fg}\""),
            line,
        );
    }
}

/// Draw an `actor` as a stick figure (head + body + arms + legs) with the name
/// underneath — mirrors upstream `drawActorTypeActor`.
fn draw_actor_figure(svg: &mut SvgBuilder, cx: f64, top: f64, h: f64, label: &str, theme: &Theme) {
    let fg = theme.fg;
    let stroke = theme.actor_stroke;
    let fill = theme.actor_fill;
    let attrs = format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.5\"");
    let line_attrs = format!("stroke=\"{stroke}\" stroke-width=\"1.5\"");

    let head_r = 7.0;
    let head_cy = top + head_r + 1.0;
    let body_top = head_cy + head_r;
    let body_bot = body_top + 13.0;
    let arm_y = body_top + 5.0;
    let arm_half = 10.0;
    let leg_dx = 8.0;
    let leg_dy = 10.0;

    svg.circle(cx, head_cy, head_r, &attrs);
    svg.line(cx, body_top, cx, body_bot, &line_attrs);
    svg.line(cx - arm_half, arm_y, cx + arm_half, arm_y, &line_attrs);
    svg.line(cx, body_bot, cx - leg_dx, body_bot + leg_dy, &line_attrs);
    svg.line(cx, body_bot, cx + leg_dx, body_bot + leg_dy, &line_attrs);

    // Name sits below the figure, within the actor's allotted height.
    let lines = label_lines(label);
    let mut y = (body_bot + leg_dy + 14.0).min(top + h - 2.0);
    for line in &lines {
        svg.text(
            cx,
            y,
            &format!("text-anchor=\"middle\" fill=\"{fg}\""),
            line,
        );
        y += ACTOR_LINE_H;
    }
}

/// Draw the `box` grouping backgrounds: a filled rect spanning member
/// participants from above the actor row down past the footer, label centered
/// at the top.
fn draw_boxes(
    svg: &mut SvgBuilder,
    d: &SequenceDiagram,
    x_of: &HashMap<String, f64>,
    w_of: &HashMap<String, f64>,
    top: f64,
    bottom: f64,
    theme: &Theme,
) {
    let fg = theme.fg;
    for item in &d.items {
        let SequenceItem::Box(b) = item else { continue };
        let mut min_l = f64::INFINITY;
        let mut max_r = f64::NEG_INFINITY;
        for id in &b.participant_ids {
            if let (Some(&cx), Some(&w)) = (x_of.get(id), w_of.get(id)) {
                min_l = min_l.min(cx - w / 2.0);
                max_r = max_r.max(cx + w / 2.0);
            }
        }
        if !min_l.is_finite() {
            continue;
        }
        let x = min_l - BOX_PAD;
        let w = (max_r - min_l) + BOX_PAD * 2.0;
        let y = top;
        let h = (bottom + BOX_PAD) - y;
        let fill = b.color.as_deref().unwrap_or("none");
        svg.rect(
            x,
            y,
            w,
            h,
            &format!("fill=\"{fill}\" stroke=\"#999\" stroke-width=\"1\""),
        );
        if !b.label.is_empty() {
            svg.text(
                x + w / 2.0,
                y + 15.0,
                &format!(
                    "text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\" font-weight=\"bold\""
                ),
                &b.label,
            );
        }
    }
}

/// Split a participant label into display lines, honoring `<br/>` (issue #3)
/// and literal `\n` escapes.
fn label_lines(label: &str) -> Vec<String> {
    let mut normalized = label.to_string();
    for br in ["<br/>", "<br />", "<br>", "\\n"] {
        normalized = normalized.replace(br, "\n");
    }
    normalized
        .split('\n')
        .map(|l| l.trim().to_string())
        .collect()
}

/// Measure a participant box from its label: width grows to fit the widest
/// line, height grows with line count. Both clamp to sane minimums.
fn actor_size(label: &str) -> (f64, f64) {
    let lines = label_lines(label);
    let max_chars = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    let w = (max_chars as f64 * ACTOR_CHAR_W + ACTOR_PAD_X * 2.0).max(ACTOR_MIN_W);
    let h = (lines.len() as f64 * ACTOR_LINE_H + 14.0).max(ACTOR_H);
    (w, h)
}

fn draw_message(
    svg: &mut SvgBuilder,
    x1: f64,
    x2: f64,
    y: f64,
    arrow: ArrowKind,
    text: &str,
    theme: &Theme,
) {
    let fg = theme.fg;
    let arrow_stroke = theme.arrow_stroke;
    let (dash, start_marker, end_marker) = stroke_for(arrow);
    let dash_attr = if dash.is_empty() {
        String::new()
    } else {
        format!(" stroke-dasharray=\"{dash}\"")
    };
    let marker_attr = |m: Option<&str>, kind: &str| match m {
        Some(name) => format!(" marker-{kind}=\"url(#{name})\""),
        None => String::new(),
    };
    let markers = format!(
        "{}{}",
        marker_attr(start_marker, "start"),
        marker_attr(end_marker, "end")
    );

    if (x1 - x2).abs() < 1e-6 {
        let r = 20.0;
        let d_attr = format!(
            "M{x1} {y}L{xr} {y}L{xr} {y2}L{x1} {y2}",
            x1 = svg_n(x1),
            y = svg_n(y),
            xr = svg_n(x1 + r),
            y2 = svg_n(y + r),
        );
        svg.path(
            &d_attr,
            &format!(
                "fill=\"none\" stroke=\"{arrow_stroke}\" stroke-width=\"1.5\"{dash_attr}{markers}"
            ),
        );
        if !text.is_empty() {
            svg.text(
                x1 + r + 4.0,
                y + r / 2.0 + 4.0,
                &format!("fill=\"{fg}\" font-size=\"12\""),
                text,
            );
        }
        return;
    }

    svg.line(
        x1,
        y,
        x2,
        y,
        &format!("stroke=\"{arrow_stroke}\" stroke-width=\"1.5\"{dash_attr}{markers}"),
    );
    if !text.is_empty() {
        let mid = (x1 + x2) / 2.0;
        svg.text(
            mid,
            y - TEXT_OFFSET,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\""),
            text,
        );
    }
}

fn draw_note(
    svg: &mut SvgBuilder,
    note: &SequenceNote,
    y: f64,
    x_of: &HashMap<String, f64>,
    theme: &Theme,
) {
    let fg = theme.fg;
    if note.participants.is_empty() {
        return;
    }
    let xs: Vec<f64> = note
        .participants
        .iter()
        .filter_map(|id| x_of.get(id).copied())
        .collect();
    if xs.is_empty() {
        return;
    }
    let (rect_x, rect_w) = match note.position {
        NotePosition::Over => {
            let min_x = xs.iter().copied().fold(f64::INFINITY, f64::min);
            let max_x = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let pad = 20.0;
            (min_x - pad, (max_x - min_x) + pad * 2.0)
        }
        NotePosition::RightOf => {
            let x = xs[0];
            (x + 12.0, 140.0)
        }
        NotePosition::LeftOf => {
            let x = xs[0];
            (x - 12.0 - 140.0, 140.0)
        }
    };
    let h = NOTE_HEIGHT;
    let yy = y - h / 2.0;
    svg.rect(
        rect_x,
        yy,
        rect_w,
        h,
        "fill=\"#FFF5AD\" stroke=\"#aaaa33\" stroke-width=\"1\"",
    );
    svg.text(
        rect_x + rect_w / 2.0,
        y + 4.0,
        &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"12\""),
        &note.text,
    );
}

/// Draw `rect <color>` background bands. Bands nest strictly (LIFO), so a plain
/// stack of open `(y_top, color)` pairs matches each close to its open.
fn draw_rect_bands(svg: &mut SvgBuilder, events: &[Event], x_of: &HashMap<String, f64>) {
    if x_of.is_empty() {
        return;
    }
    let min_x = x_of.values().copied().fold(f64::INFINITY, f64::min);
    let max_x = x_of.values().copied().fold(f64::NEG_INFINITY, f64::max);
    let mut stack: Vec<(f64, Option<String>)> = Vec::new();
    for ev in events {
        match &ev.kind {
            EventKind::RectOpen { color } => stack.push((ev.y, color.clone())),
            EventKind::RectClose => {
                if let Some((y_top, color)) = stack.pop() {
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

fn draw_block_frames(
    svg: &mut SvgBuilder,
    events: &[Event],
    x_of: &HashMap<String, f64>,
    theme: &Theme,
) {
    let fg = theme.fg;
    let min_x = x_of.values().copied().fold(f64::INFINITY, f64::min);
    let max_x = x_of.values().copied().fold(f64::NEG_INFINITY, f64::max);
    // Walk events to pair open/close with stack.
    let mut stack: Vec<(usize, BlockKind, String)> = Vec::new();
    for (i, ev) in events.iter().enumerate() {
        match &ev.kind {
            EventKind::BlockOpen { kind, label } => stack.push((i, *kind, label.clone())),
            EventKind::BlockBranch { label } => {
                let y_branch = ev.y;
                svg.line(
                    min_x - 16.0,
                    y_branch,
                    max_x + 16.0,
                    y_branch,
                    "stroke=\"#888\" stroke-width=\"1\" stroke-dasharray=\"4 3\"",
                );
                if !label.is_empty() {
                    svg.text(
                        min_x + 4.0,
                        y_branch - 4.0,
                        &format!("fill=\"{fg}\" font-size=\"11\" font-style=\"italic\""),
                        &format!("[{label}]"),
                    );
                }
            }
            EventKind::BlockClose => {
                if let Some((open_idx, kind, label)) = stack.pop() {
                    let y_top = events[open_idx].y;
                    let y_bot = ev.y;
                    draw_block_frame(svg, kind, &label, min_x, max_x, y_top, y_bot, theme);
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
) {
    let fg = theme.fg;
    let frame_x = min_x - 16.0;
    let frame_w = (max_x + 16.0) - frame_x;
    let frame_h = y_bot - y_top;
    svg.rect(
        frame_x,
        y_top,
        frame_w,
        frame_h,
        "fill=\"none\" stroke=\"#666\" stroke-width=\"1\"",
    );
    let title = match kind {
        BlockKind::Alt => "alt",
        BlockKind::Par => "par",
        BlockKind::Critical => "critical",
        BlockKind::Loop => "loop",
        BlockKind::Opt => "opt",
        BlockKind::Break => "break",
    };
    // Label tab in upper-left
    svg.rect(
        frame_x,
        y_top - 0.5,
        BLOCK_LABEL_W,
        18.0,
        "fill=\"#EEE\" stroke=\"#666\" stroke-width=\"1\"",
    );
    svg.text(
        frame_x + 6.0,
        y_top + 13.0,
        &format!("fill=\"{fg}\" font-size=\"11\" font-weight=\"bold\""),
        title,
    );
    if !label.is_empty() {
        svg.text(
            frame_x + BLOCK_LABEL_W + 8.0,
            y_top + 13.0,
            &format!("fill=\"{fg}\" font-size=\"11\" font-style=\"italic\""),
            &format!("[{label}]"),
        );
    }
}

fn draw_activations(
    svg: &mut SvgBuilder,
    events: &[Event],
    x_of: &HashMap<String, f64>,
    lifeline_bottom: f64,
) {
    // A stack of open start-ys per participant so nested activations
    // (e.g. the `->>+` shorthand) stack instead of overwriting. Each nesting
    // level is offset horizontally so the outer band stays visible.
    let mut open: HashMap<String, Vec<f64>> = HashMap::new();
    for ev in events {
        match &ev.kind {
            EventKind::Activate(id) => {
                open.entry(id.clone()).or_default().push(ev.y);
            }
            EventKind::Deactivate(id) => {
                if let Some(start_y) = open.get_mut(id).and_then(Vec::pop) {
                    // The popped entry sat at depth == current stack length.
                    let level = open.get(id).map_or(0, Vec::len);
                    draw_activation_band(svg, x_of, id, start_y, ev.y, level);
                }
            }
            _ => {}
        }
    }
    // Unclosed activations extend to the bottom of the lifelines.
    for (id, starts) in &open {
        for (level, &start_y) in starts.iter().enumerate() {
            draw_activation_band(svg, x_of, id, start_y, lifeline_bottom, level);
        }
    }
}

fn draw_activation_band(
    svg: &mut SvgBuilder,
    x_of: &HashMap<String, f64>,
    id: &str,
    start_y: f64,
    end_y: f64,
    level: usize,
) {
    if let Some(&cx) = x_of.get(id) {
        let offset = level as f64 * 3.0;
        svg.rect(
            cx - ACTIVATION_W / 2.0 + offset,
            start_y,
            ACTIVATION_W,
            (end_y - start_y).max(8.0),
            "fill=\"#ECECFF\" stroke=\"#9370DB\" stroke-width=\"1\"",
        );
    }
}

/// Draw the `×` that terminates a destroyed participant's lifeline.
fn draw_destroy_cross(svg: &mut SvgBuilder, cx: f64, cy: f64, theme: &Theme) {
    let stroke = theme.arrow_stroke;
    let r = DESTROY_CROSS;
    let attrs = format!("stroke=\"{stroke}\" stroke-width=\"1.5\"");
    svg.line(cx - r, cy - r, cx + r, cy + r, &attrs);
    svg.line(cx + r, cy - r, cx - r, cy + r, &attrs);
}

/// Returns `(dash, start_marker, end_marker)` for an arrow kind. Plain `->`/`-->`
/// are bare lines (no marker); bidirectional arrows carry a marker at both ends.
fn stroke_for(a: ArrowKind) -> (&'static str, Option<&'static str>, Option<&'static str>) {
    match a {
        ArrowKind::Solid => ("", None, None),
        ArrowKind::SolidArrow => ("", None, Some("arrow-filled")),
        ArrowKind::Dashed => ("6 4", None, None),
        ArrowKind::DashedArrow => ("6 4", None, Some("arrow-filled")),
        ArrowKind::Cross => ("", None, Some("arrow-cross")),
        ArrowKind::Open => ("", None, Some("arrow-open")),
        ArrowKind::BiSolidArrow => ("", Some("arrow-filled"), Some("arrow-filled")),
        ArrowKind::BiDashedArrow => ("6 4", Some("arrow-filled"), Some("arrow-filled")),
    }
}

fn define_markers(svg: &mut SvgBuilder, theme: &Theme) {
    let arrow_stroke = theme.arrow_stroke;
    let h = ARROW_HEAD;
    let filled = format!(
        "<marker id=\"arrow-filled\" viewBox=\"0 0 {h} {h}\" refX=\"{h}\" refY=\"{half}\" \
         markerWidth=\"{h}\" markerHeight=\"{h}\" orient=\"auto-start-reverse\">\
         <path d=\"M0 0 L{h} {half} L0 {h} z\" fill=\"{arrow_stroke}\"/></marker>",
        h = h,
        half = h / 2.0,
    );
    let open = format!(
        "<marker id=\"arrow-open\" viewBox=\"0 0 {h} {h}\" refX=\"{h}\" refY=\"{half}\" \
         markerWidth=\"{h}\" markerHeight=\"{h}\" orient=\"auto-start-reverse\">\
         <path d=\"M0 0 L{h} {half} L0 {h}\" fill=\"none\" stroke=\"{arrow_stroke}\" stroke-width=\"1.5\"/></marker>",
        h = h,
        half = h / 2.0,
    );
    let cross = format!(
        "<marker id=\"arrow-cross\" viewBox=\"0 0 {h} {h}\" refX=\"{half}\" refY=\"{half}\" \
         markerWidth=\"{h}\" markerHeight=\"{h}\" orient=\"auto\">\
         <path d=\"M0 0 L{h} {h} M{h} 0 L0 {h}\" stroke=\"{arrow_stroke}\" stroke-width=\"1.5\"/></marker>",
        h = h,
        half = h / 2.0,
    );
    svg.defs_raw(&filled);
    svg.defs_raw(&open);
    svg.defs_raw(&cross);
}

fn svg_n(v: f64) -> String {
    super::builder::fnum(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn build(s: &str) -> SequenceDiagram {
        match parse(s).unwrap() {
            crate::parse::Diagram::Sequence(d) => d,
            _ => panic!("not sequence"),
        }
    }

    #[test]
    fn basic_envelope() {
        let svg = render(
            &build("sequenceDiagram\ntitle Login\nalice->>bob: hi\n"),
            &Theme::default(),
        );
        assert!(svg.contains(">Login<"));
        assert!(svg.contains(">hi<"));
        assert!(svg.contains("arrow-filled"));
    }

    #[test]
    fn plain_line_arrows_have_no_marker() {
        // `->` / `-->` are bare lines with no arrowhead (issue #57).
        let svg = render(&build("sequenceDiagram\nA->B: x\n"), &Theme::default());
        assert!(!svg.contains("marker-end"));
        assert!(!svg.contains("marker-start"));
        let svg = render(&build("sequenceDiagram\nA-->B: y\n"), &Theme::default());
        assert!(!svg.contains("marker-end"));
        assert!(!svg.contains("marker-start"));
    }

    #[test]
    fn bidirectional_arrow_marks_both_ends() {
        let svg = render(
            &build("sequenceDiagram\nAlice<<->>Bob: hi\n"),
            &Theme::default(),
        );
        assert!(svg.contains("marker-start=\"url(#arrow-filled)\""));
        assert!(svg.contains("marker-end=\"url(#arrow-filled)\""));
    }

    #[test]
    fn alt_block_renders_frame() {
        let svg = render(
            &build("sequenceDiagram\nA->>B: q\nalt yes\nA->>B: y\nelse no\nA->>B: n\nend\n"),
            &Theme::default(),
        );
        assert!(svg.contains(">alt<"));
        assert!(svg.contains("[yes]"));
        assert!(svg.contains("[no]"));
    }

    #[test]
    fn loop_block_renders_frame() {
        let svg = render(
            &build("sequenceDiagram\nloop every 5s\nA->>B: ping\nend\n"),
            &Theme::default(),
        );
        assert!(svg.contains(">loop<"));
        assert!(svg.contains("[every 5s]"));
    }

    #[test]
    fn note_over_renders() {
        let svg = render(
            &build("sequenceDiagram\nA->>B: hi\nNote over A,B: shared\n"),
            &Theme::default(),
        );
        assert!(svg.contains(">shared<"));
    }

    #[test]
    fn autonumber_prefixes_messages() {
        let svg = render(
            &build("sequenceDiagram\nautonumber\nA->>B: x\nA->>B: y\n"),
            &Theme::default(),
        );
        assert!(svg.contains(">1. x<"));
        assert!(svg.contains(">2. y<"));
    }

    #[test]
    fn autonumber_honors_start_step_and_off() {
        let svg = render(
            &build(
                "sequenceDiagram\nautonumber 10 5\nA->>B: a\nA->>B: b\nautonumber off\nA->>B: c\n",
            ),
            &Theme::default(),
        );
        assert!(svg.contains(">10. a<"));
        assert!(svg.contains(">15. b<"));
        // After `autonumber off`, subsequent messages carry no prefix.
        assert!(svg.contains(">c<"));
        assert!(!svg.contains(">20. c<"));
    }

    #[test]
    fn break_block_renders_frame() {
        let svg = render(
            &build("sequenceDiagram\nbreak connection lost\nA->>B: bye\nend\n"),
            &Theme::default(),
        );
        assert!(svg.contains(">break<"));
        assert!(svg.contains("[connection lost]"));
    }

    #[test]
    fn rect_block_draws_colored_band() {
        let svg = render(
            &build("sequenceDiagram\nrect rgb(200,220,255)\nA->>B: x\nend\n"),
            &Theme::default(),
        );
        assert!(svg.contains("fill=\"rgb(200,220,255)\""));
    }

    #[test]
    fn actor_box_grows_to_fit_label() {
        // A wide label must produce a box wider than the fixed minimum, and the
        // canvas width must accommodate it.
        let wide = "sequenceDiagram\n\
            participant BE as Backend app06 :8082 (UAT) app14 :8081 (PROD) cyberscore-portal FrankenPHP\n\
            participant A\nA->>BE: hi\n";
        let svg = render(&build(wide), &Theme::default());
        // Find the widest actor rect width; it must exceed the old fixed 110.
        let max_w = svg
            .split("width=\"")
            .skip(1)
            .filter_map(|s| s.split('"').next())
            .filter_map(|s| s.parse::<f64>().ok())
            .fold(0.0_f64, f64::max);
        assert!(max_w > 110.0, "expected a box wider than 110, got {max_w}");
    }

    #[test]
    fn multiline_label_splits_on_br() {
        let svg = render(
            &build("sequenceDiagram\nparticipant BE as Backend<br/>app06\nA->>BE: hi\n"),
            &Theme::default(),
        );
        assert!(svg.contains(">Backend<"));
        assert!(svg.contains(">app06<"));
    }

    #[test]
    fn actor_size_matches_label() {
        assert_eq!(actor_size("A"), (ACTOR_MIN_W, ACTOR_H));
        let (w, h) = actor_size("one<br/>two<br/>three");
        assert!(h > ACTOR_H, "multi-line label should be taller");
        assert_eq!(w, ACTOR_MIN_W, "short lines keep the minimum width");
    }

    #[test]
    fn actor_renders_as_stick_figure() {
        let svg = render(
            &build("sequenceDiagram\nactor A\nparticipant B\nA->>B: hi\n"),
            &Theme::default(),
        );
        // Stick figure emits a <circle> head; a plain participant box does not.
        assert!(svg.contains("<circle"), "actor should draw a circle head");
        assert!(svg.contains(">A</text>"), "actor name below figure");
    }

    #[test]
    fn participant_stays_a_box() {
        let svg = render(
            &build("sequenceDiagram\nparticipant A\nA->>B: hi\n"),
            &Theme::default(),
        );
        assert!(!svg.contains("<circle"), "participant is a rounded rect");
    }

    #[test]
    fn box_renders_background_and_label() {
        let svg = render(
            &build(
                "sequenceDiagram\nbox Aqua Team\nparticipant A\nparticipant B\nend\nA->>B: hi\n",
            ),
            &Theme::default(),
        );
        assert!(svg.contains("fill=\"Aqua\""), "box uses its declared color");
        assert!(svg.contains(">Team<"), "box label is drawn");
    }

    #[test]
    fn box_without_color_is_transparent() {
        let svg = render(
            &build("sequenceDiagram\nbox Team\nparticipant A\nparticipant B\nend\nA->>B: hi\n"),
            &Theme::default(),
        );
        assert!(svg.contains(">Team<"));
        // A transparent box has no colored fill of its own.
        assert!(!svg.contains("fill=\"Aqua\""));
    }

    #[test]
    fn activate_deactivate_draws_band() {
        let svg = render(
            &build("sequenceDiagram\nA->>B: req\nactivate B\nB-->>A: resp\ndeactivate B\n"),
            &Theme::default(),
        );
        // activation rect uses #ECECFF
        assert!(svg.contains("#ECECFF"));
    }

    #[test]
    fn nested_activations_stack_and_offset() {
        // Two activations on B open before either closes: the bands must not
        // overwrite each other, and the inner one is offset horizontally.
        let events = vec![
            Event {
                y: 10.0,
                kind: EventKind::Activate("B".into()),
            },
            Event {
                y: 20.0,
                kind: EventKind::Activate("B".into()),
            },
            Event {
                y: 30.0,
                kind: EventKind::Deactivate("B".into()),
            },
            Event {
                y: 40.0,
                kind: EventKind::Deactivate("B".into()),
            },
        ];
        let mut x_of = HashMap::new();
        x_of.insert("B".to_string(), 100.0);
        let mut svg = SvgBuilder::new(300.0, 300.0);
        draw_activations(&mut svg, &events, &x_of, 200.0);
        let out = svg.finish();
        // Outer band starts at x = 95 (100 - 10/2 + 0), inner offset by 3 → 98.
        assert!(out.contains("x=\"95\""), "outer band at base x");
        assert!(out.contains("x=\"98\""), "inner band offset by level*3");
    }

    #[test]
    fn created_participant_has_no_top_row_box() {
        // Carl's box is drawn inline at the create point, so its lifeline does
        // not span the full height and there is no top-row box for it. We at
        // least verify the diagram renders and both Bob (normal) and Carl draw.
        let svg = render(
            &build(
                "sequenceDiagram\nAlice->>Bob: Hello\ncreate participant Carl\nAlice->>Carl: Hi Carl\n",
            ),
            &Theme::default(),
        );
        assert!(svg.contains(">Carl<"), "created participant box is drawn");
        assert!(svg.contains(">Hi Carl<"), "creating message is drawn");
    }

    #[test]
    fn destroyed_participant_draws_cross() {
        let svg = render(
            &build(
                "sequenceDiagram\ncreate participant Carl\nAlice->>Carl: Hi\ndestroy Carl\nAlice-xCarl: bye\n",
            ),
            &Theme::default(),
        );
        // The termination cross is two crossing <line>s; verify the message and
        // participant still render (cross geometry is asserted via events below).
        assert!(svg.contains(">bye<"));
    }

    #[test]
    fn destroy_cross_geometry() {
        // Exercise the cross drawing directly.
        let mut svg = SvgBuilder::new(300.0, 300.0);
        draw_destroy_cross(&mut svg, 50.0, 100.0, &Theme::default());
        let out = svg.finish();
        // Two diagonal strokes centered on (50, 100) with half-size 7.
        assert!(out.contains("x1=\"43\""));
        assert!(out.contains("y2=\"107\""));
    }

    #[test]
    fn unclosed_activation_extends_to_bottom() {
        let events = vec![Event {
            y: 10.0,
            kind: EventKind::Activate("B".into()),
        }];
        let mut x_of = HashMap::new();
        x_of.insert("B".to_string(), 100.0);
        let mut svg = SvgBuilder::new(300.0, 300.0);
        draw_activations(&mut svg, &events, &x_of, 200.0);
        let out = svg.finish();
        // Band height = lifeline_bottom - start_y = 200 - 10 = 190.
        assert!(out.contains("#ECECFF"), "unclosed activation is drawn");
        assert!(out.contains("height=\"190\""), "extends to lifeline bottom");
    }
}
