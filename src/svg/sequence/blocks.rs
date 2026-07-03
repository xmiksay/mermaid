//! Layout pass (event collection) and block-frame drawing.

use std::collections::HashMap;

use crate::parse::{AltBranch, SequenceBlock, SequenceItem, SequenceRect};

use super::*;

pub(super) fn layout_items(
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

/// Draw `rect <color>` background bands. Bands nest strictly (LIFO), so a plain
/// stack of open `(y_top, color)` pairs matches each close to its open.
pub(super) fn draw_rect_bands(svg: &mut SvgBuilder, events: &[Event], x_of: &HashMap<String, f64>) {
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

pub(super) fn draw_block_frames(
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
}
