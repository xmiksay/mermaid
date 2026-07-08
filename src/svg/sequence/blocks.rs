//! Layout pass (event collection) and block-frame drawing.

use std::collections::HashMap;

use crate::parse::{AltBranch, SequenceBlock, SequenceItem, SequenceRect};

use crate::svg::builder::fnum;

use super::*;

pub(super) fn layout_items(
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
pub(super) fn draw_rect_bands(svg: &mut SvgBuilder, events: &[Event], x_of: &HashMap<String, f64>) {
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

pub(super) fn draw_block_frames(
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
    svg.text(
        frame_x + 6.0,
        y_top + 13.0,
        &format!("fill=\"{fg}\" font-size=\"11\" font-weight=\"bold\""),
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
    fn autonumber_draws_circle_badges() {
        // Numbered messages carry a filled circle badge on the arrow origin, not
        // a `"1. "` text prefix (#268).
        let svg = render(
            &build("sequenceDiagram\nautonumber\nA->>B: x\nA->>B: y\n"),
            &Theme::default(),
        );
        assert!(svg.contains("<circle"), "badge is a filled circle");
        assert!(
            svg.contains(">1<") && svg.contains(">2<"),
            "badge numbers drawn"
        );
        // Message text keeps its own label with no numeric prefix.
        assert!(svg.contains(">x<") && svg.contains(">y<"));
        assert!(!svg.contains(">1. x<"), "no legacy text prefix");
    }

    #[test]
    fn autonumber_honors_start_step_and_off() {
        let svg = render(
            &build(
                "sequenceDiagram\nautonumber 10 5\nA->>B: a\nA->>B: b\nautonumber off\nA->>B: c\n",
            ),
            &Theme::default(),
        );
        assert!(svg.contains(">10<"));
        assert!(svg.contains(">15<"));
        // After `autonumber off`, subsequent messages carry no badge.
        assert!(svg.contains(">c<"));
        assert!(!svg.contains(">20<"));
    }

    #[test]
    fn autonumber_decimal_numbers_render() {
        // `autonumber 1.5 0.5` → 1.5, 2, 2.5 — integral values drop the decimal
        // point (#176).
        let svg = render(
            &build("sequenceDiagram\nautonumber 1.5 0.5\nA->>B: a\nA->>B: b\nA->>B: c\n"),
            &Theme::default(),
        );
        assert!(svg.contains(">1.5<"));
        assert!(svg.contains(">2<"));
        assert!(svg.contains(">2.5<"));
    }

    #[test]
    fn half_arrow_uses_half_marker() {
        // `A-\\B` (upstream doubled barb) → upper-barb half marker at the head.
        let svg = render(&build("sequenceDiagram\nA-\\\\B: x\n"), &Theme::default());
        assert!(svg.contains("id=\"arrow-half-top\""));
        assert!(svg.contains("marker-end=\"url(#arrow-half-top)\""));
    }

    #[test]
    fn reverse_half_arrow_marks_the_tail() {
        // `A//-B` (reverse lower barb) → lower-barb half marker at the tail.
        let svg = render(&build("sequenceDiagram\nA//-B: x\n"), &Theme::default());
        assert!(svg.contains("id=\"arrow-half-bottom\""));
        assert!(svg.contains("marker-start=\"url(#arrow-half-bottom)\""));
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
    fn block_frame_bounds_to_involved_participants() {
        // A is leftmost but the loop only involves B and C: the frame must start
        // to the right of A instead of spanning the whole diagram (#123).
        let svg = render(
            &build(
                "sequenceDiagram\nparticipant A\nparticipant B\nparticipant C\n\
                 A->>B: setup\nloop retry\nB->>C: ping\nend\n",
            ),
            &Theme::default(),
        );
        assert!(svg.contains(">loop<"));
        // B's column left edge (223) bounds the frame; a full-span frame would
        // start at A's column (63).
        assert!(svg.contains("x=\"223\""), "loop frame starts right of A");
        assert!(
            !svg.contains("x=\"63\""),
            "loop frame must not span down to A's lifeline"
        );
    }

    #[test]
    fn block_frame_uses_theme_label_fill() {
        let svg = render(
            &build("sequenceDiagram\nA->>B: q\nloop retry\nA->>B: y\nend\n"),
            &Theme::dark(),
        );
        assert!(!svg.contains("fill=\"#EEE\""));
        assert!(svg.contains(Theme::dark().frame_label_fill.as_ref()));
    }

    #[test]
    fn alt_frame_is_dotted_themed_with_centered_guards() {
        // Frame chrome: dotted theme-colored border + centered guard text, not
        // the old solid-gray border with left-italic labels (#268).
        let svg = render(
            &build("sequenceDiagram\nA->>B: q\nalt cached\nA->>B: y\nelse miss\nA->>B: n\nend\n"),
            &Theme::default(),
        );
        assert!(
            !svg.contains("stroke=\"#666\""),
            "no solid gray frame border"
        );
        // Dotted border/divider in the actor-stroke color (lifelines are solid).
        assert!(svg.contains("stroke-dasharray=\"2 2\""));
        assert!(svg.contains(&format!("stroke=\"{}\"", Theme::default().actor_stroke)));
        // Guard text is centered (text-anchor middle), no longer italic.
        assert!(svg.contains("[cached]") && svg.contains("[miss]"));
        assert!(!svg.contains("font-style=\"italic\""));
    }

    #[test]
    fn activation_band_starts_at_activating_arrow() {
        // `->>+`/`-->>-` shorthand: the band top aligns to the request arrow and
        // its bottom to the response arrow, not half a row below them (#227).
        let d = build("sequenceDiagram\nA->>+B: req\nB-->>-A: resp\n");
        let mut events = Vec::new();
        let mut cursor = 0.0;
        let mut counter = 1.0;
        let mut num = Numbering {
            on: false,
            step: 1.0,
        };
        layout_items(
            &d.items,
            &mut events,
            &mut cursor,
            &mut counter,
            &mut num,
            &HashMap::new(),
        );
        let msg_ys: Vec<f64> = events
            .iter()
            .filter_map(|e| match e.kind {
                EventKind::Message { .. } => Some(e.y),
                _ => None,
            })
            .collect();
        let act_y = events
            .iter()
            .find_map(|e| match &e.kind {
                EventKind::Activate(_) => Some(e.y),
                _ => None,
            })
            .unwrap();
        let deact_y = events
            .iter()
            .find_map(|e| match &e.kind {
                EventKind::Deactivate(_) => Some(e.y),
                _ => None,
            })
            .unwrap();
        assert_eq!(act_y, msg_ys[0], "band top sits on the request arrow");
        assert_eq!(deact_y, msg_ys[1], "band bottom sits on the response arrow");
    }

    #[test]
    fn standalone_activate_stays_on_cursor() {
        // An `activate` not directly following a message arrow (here separated by
        // a note) is unaffected: it lands on the running cursor, below the arrow.
        let d = build("sequenceDiagram\nA->>B: req\nNote right of B: wait\nactivate B\n");
        let mut events = Vec::new();
        let mut cursor = 0.0;
        let mut counter = 1.0;
        let mut num = Numbering {
            on: false,
            step: 1.0,
        };
        layout_items(
            &d.items,
            &mut events,
            &mut cursor,
            &mut counter,
            &mut num,
            &HashMap::new(),
        );
        let msg_y = events
            .iter()
            .find_map(|e| match e.kind {
                EventKind::Message { .. } => Some(e.y),
                _ => None,
            })
            .unwrap();
        let act_y = events
            .iter()
            .find_map(|e| match &e.kind {
                EventKind::Activate(_) => Some(e.y),
                _ => None,
            })
            .unwrap();
        assert!(act_y > msg_y, "standalone activate stays on the cursor");
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
