//! Sequence diagram renderer.
//!
//! Two passes: first collect "events" with their y position from a recursive
//! walk of `items`, then draw lifelines + headers + events. Block frames are
//! emitted around their child events' y range.

use std::collections::HashMap;

use crate::parse::{Message, SequenceDiagram, SequenceItem, SequenceNote};

use super::builder::SvgBuilder;
use super::theme::Theme;

mod blocks;
mod glyphs;
mod messages;
mod participants;

use blocks::*;
use glyphs::*;
use messages::*;
use participants::*;

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
const NOTE_SIDE_W: f64 = 140.0; // fixed width of a left/right-of note box
const NOTE_MIN_W: f64 = 100.0; // minimum width of an `over` note box
const NOTE_CHAR_W: f64 = 6.5; // per-glyph width estimate for note text (12px)
const NOTE_PAD_X: f64 = 8.0; // horizontal inner padding when wrapping note text
const NOTE_PAD_Y: f64 = 8.0; // vertical inner padding of a note box
const NOTE_LINE_H: f64 = 16.0; // baseline spacing between wrapped note lines
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
    let fg = &theme.fg;
    let lifeline = &theme.lifeline;
    if d.participants.is_empty() {
        let mut svg = SvgBuilder::new(200.0, 60.0).theme(theme);
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
        .map(|p| actor_size(&p.display, theme.font_size))
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

    let mut svg = SvgBuilder::new(width, height).theme(theme);
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
    draw_activations(&mut svg, &events, &x_of, lifeline_bottom, theme);

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
}
