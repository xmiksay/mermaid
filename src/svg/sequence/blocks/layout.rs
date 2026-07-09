//! First pass: collect `Event`s with their y positions from a recursive walk
//! of the sequence `items`.

use std::collections::HashMap;

use crate::parse::{AltBranch, SequenceBlock, SequenceItem, SequenceRect};

use super::super::*;

pub(in crate::svg::sequence) fn layout_items(
    items: &[SequenceItem],
    out: &mut Vec<Event>,
    cursor: &mut f64,
    counter: &mut f64,
    num: &mut Numbering,
    x_of: &HashMap<String, f64>,
) {
    // Arrow y of the message immediately preceding an activation, so a `->>+`/
    // `-->>-` shorthand starts (or ends) its band at the arrow rather than half a
    // row below it, matching upstream. Cleared every iteration; only re-set by a
    // message, so a standalone `activate`/`deactivate` still lands on the cursor.
    let mut prev_msg_arrow_y: Option<f64> = None;
    for item in items {
        let attach_y = prev_msg_arrow_y.take();
        match item {
            SequenceItem::AutoNumber(cfg) => match cfg {
                Some(c) => {
                    num.on = true;
                    // A non-positive step would never advance; fall back to 1.
                    num.step = if c.step > 0.0 { c.step } else { 1.0 };
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
                let arrow_y = *cursor - MSG_STEP / 2.0;
                out.push(Event {
                    y: arrow_y,
                    kind: EventKind::Message {
                        msg: m.clone(),
                        number,
                    },
                });
                prev_msg_arrow_y = Some(arrow_y);
            }
            SequenceItem::Note(n) => {
                let h = note_geometry(n, x_of).map_or(NOTE_HEIGHT, |g| g.height);
                *cursor += h + 10.0;
                out.push(Event {
                    y: *cursor - h / 2.0,
                    kind: EventKind::Note(n.clone()),
                });
            }
            SequenceItem::Activate(id) => {
                out.push(Event {
                    y: attach_y.unwrap_or(*cursor),
                    kind: EventKind::Activate(id.clone()),
                });
            }
            SequenceItem::Deactivate(id) => {
                out.push(Event {
                    y: attach_y.unwrap_or(*cursor),
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
    counter: &mut f64,
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
    counter: &mut f64,
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
    counter: &mut f64,
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
