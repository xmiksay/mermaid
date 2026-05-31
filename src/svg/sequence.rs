//! Sequence diagram renderer.
//!
//! Two passes: first collect "events" with their y position from a recursive
//! walk of `items`, then draw lifelines + headers + events. Block frames are
//! emitted around their child events' y range.

use std::collections::HashMap;

use crate::parse::{
    AltBranch, ArrowKind, Message, NotePosition, SequenceBlock, SequenceDiagram, SequenceItem,
    SequenceNote,
};

use super::builder::SvgBuilder;
use super::theme::Theme;

const ACTOR_W: f64 = 110.0;
const ACTOR_H: f64 = 40.0;
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

    let mut x_of: HashMap<String, f64> = HashMap::new();
    for (i, p) in d.participants.iter().enumerate() {
        let x = PAD + ACTOR_W / 2.0 + (i as f64) * (ACTOR_W + PARTICIPANT_GAP);
        x_of.insert(p.id.clone(), x);
    }
    let last_x = PAD
        + (d.participants.len() as f64) * ACTOR_W
        + (d.participants.len().saturating_sub(1) as f64) * PARTICIPANT_GAP;
    let width = last_x + PAD;

    let top_y = PAD + title_h;
    let header_bottom = top_y + ACTOR_H;
    let body_top = header_bottom + MSG_TOP_GAP;

    // First pass: precompute events with y positions.
    let mut events: Vec<Event> = Vec::new();
    let mut cursor = body_top;
    let mut step_counter: u32 = 0;
    layout_items(
        &d.items,
        &mut events,
        &mut cursor,
        &mut step_counter,
        d.autonumber,
        &x_of,
    );
    let lifeline_bottom = cursor + MSG_BOTTOM_GAP;
    let footer_top = lifeline_bottom;
    let footer_bottom = footer_top + ACTOR_H;
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

    // Lifelines
    for p in &d.participants {
        let x = x_of[&p.id];
        svg.line(
            x,
            header_bottom,
            x,
            lifeline_bottom,
            &format!("stroke=\"{lifeline}\" stroke-width=\"1\" stroke-dasharray=\"4 4\""),
        );
    }

    // Activation bands (computed from activate/deactivate events).
    draw_activations(&mut svg, &events, &x_of);

    // Block frames
    draw_block_frames(&mut svg, &events, &x_of, theme);

    // Headers (top + bottom)
    for p in &d.participants {
        let x = x_of[&p.id];
        draw_actor(&mut svg, x, top_y, &p.display, theme);
        draw_actor(&mut svg, x, footer_top, &p.display, theme);
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
}

#[derive(Debug, Clone, Copy)]
enum BlockKind {
    Alt,
    Par,
    Critical,
    Loop,
    Opt,
}

fn layout_items(
    items: &[SequenceItem],
    out: &mut Vec<Event>,
    cursor: &mut f64,
    counter: &mut u32,
    autonumber: bool,
    x_of: &HashMap<String, f64>,
) {
    for item in items {
        match item {
            SequenceItem::Message(m) => {
                *cursor += MSG_STEP;
                let number = if autonumber {
                    *counter += 1;
                    Some(*counter)
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
            SequenceItem::Alt(branches) => {
                emit_branched_block(
                    BlockKind::Alt,
                    branches,
                    out,
                    cursor,
                    counter,
                    autonumber,
                    x_of,
                );
            }
            SequenceItem::Par(branches) => {
                emit_branched_block(
                    BlockKind::Par,
                    branches,
                    out,
                    cursor,
                    counter,
                    autonumber,
                    x_of,
                );
            }
            SequenceItem::Critical(branches) => {
                emit_branched_block(
                    BlockKind::Critical,
                    branches,
                    out,
                    cursor,
                    counter,
                    autonumber,
                    x_of,
                );
            }
            SequenceItem::Loop(b) => {
                emit_simple_block(BlockKind::Loop, b, out, cursor, counter, autonumber, x_of);
            }
            SequenceItem::Opt(b) => {
                emit_simple_block(BlockKind::Opt, b, out, cursor, counter, autonumber, x_of);
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
    autonumber: bool,
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
    layout_items(&block.items, out, cursor, counter, autonumber, x_of);
    *cursor += BLOCK_BOTTOM_GAP;
    out.push(Event {
        y: *cursor,
        kind: EventKind::BlockClose,
    });
}

fn emit_branched_block(
    kind: BlockKind,
    branches: &[AltBranch],
    out: &mut Vec<Event>,
    cursor: &mut f64,
    counter: &mut u32,
    autonumber: bool,
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
        layout_items(&branch.items, out, cursor, counter, autonumber, x_of);
    }
    *cursor += BLOCK_BOTTOM_GAP;
    out.push(Event {
        y: *cursor,
        kind: EventKind::BlockClose,
    });
}

fn draw_actor(svg: &mut SvgBuilder, cx: f64, top: f64, label: &str, theme: &Theme) {
    let fg = theme.fg;
    let actor_fill = theme.actor_fill;
    let actor_stroke = theme.actor_stroke;
    let x = cx - ACTOR_W / 2.0;
    svg.rect(
        x,
        top,
        ACTOR_W,
        ACTOR_H,
        &format!("fill=\"{actor_fill}\" stroke=\"{actor_stroke}\" stroke-width=\"1.5\" rx=\"4\""),
    );
    svg.text(
        cx,
        top + ACTOR_H / 2.0 + 5.0,
        &format!("text-anchor=\"middle\" fill=\"{fg}\""),
        label,
    );
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
    let (dash, marker) = stroke_for(arrow);
    let dash_attr = if dash.is_empty() {
        String::new()
    } else {
        format!(" stroke-dasharray=\"{dash}\"")
    };

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
                "fill=\"none\" stroke=\"{arrow_stroke}\" stroke-width=\"1.5\"{dash_attr} marker-end=\"url(#{marker})\""
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
        &format!(
            "stroke=\"{arrow_stroke}\" stroke-width=\"1.5\"{dash_attr} marker-end=\"url(#{marker})\""
        ),
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

fn draw_activations(svg: &mut SvgBuilder, events: &[Event], x_of: &HashMap<String, f64>) {
    // Find paired activate/deactivate per participant id.
    let mut open: HashMap<String, f64> = HashMap::new();
    for ev in events {
        match &ev.kind {
            EventKind::Activate(id) => {
                open.insert(id.clone(), ev.y);
            }
            EventKind::Deactivate(id) => {
                if let Some(start_y) = open.remove(id) {
                    if let Some(&cx) = x_of.get(id) {
                        svg.rect(
                            cx - ACTIVATION_W / 2.0,
                            start_y,
                            ACTIVATION_W,
                            (ev.y - start_y).max(8.0),
                            "fill=\"#ECECFF\" stroke=\"#9370DB\" stroke-width=\"1\"",
                        );
                    }
                }
            }
            _ => {}
        }
    }
    // Any still-open activations — leave them open until lifeline_bottom.
}

fn stroke_for(a: ArrowKind) -> (&'static str, &'static str) {
    match a {
        ArrowKind::Solid => ("", "arrow-open"),
        ArrowKind::SolidArrow => ("", "arrow-filled"),
        ArrowKind::Dashed => ("6 4", "arrow-open"),
        ArrowKind::DashedArrow => ("6 4", "arrow-filled"),
        ArrowKind::Cross => ("", "arrow-cross"),
        ArrowKind::Open => ("", "arrow-open"),
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
    fn activate_deactivate_draws_band() {
        let svg = render(
            &build("sequenceDiagram\nA->>B: req\nactivate B\nB-->>A: resp\ndeactivate B\n"),
            &Theme::default(),
        );
        // activation rect uses #ECECFF
        assert!(svg.contains("#ECECFF"));
    }
}
