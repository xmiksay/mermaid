use super::super::*;
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
    // Operator label is regular weight (upstream `.labelText`), not bold
    // (#329).
    assert!(!svg.contains("font-weight=\"bold\">alt<"));
    // The pentagon tab fills with the lavender label-box color, not gray
    // (#329).
    let t = Theme::default();
    assert!(svg.contains(&format!("fill=\"{}\"", t.frame_label_fill)));
    assert_eq!(t.frame_label_fill, "#ECECFF");
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
    // Badge fill is the near-black signal color, not the purple actor
    // stroke of #268 (#329).
    let t = Theme::default();
    assert!(
        svg.contains(&format!("fill=\"{}\" stroke=\"none\"", t.arrow_stroke)),
        "badge fills with the near-black arrow stroke"
    );
    assert!(
        !svg.contains(&format!("fill=\"{}\" stroke=\"none\"", t.actor_stroke)),
        "badge no longer fills purple"
    );
}

#[test]
fn autonumber_honors_start_step_and_off() {
    let svg = render(
        &build("sequenceDiagram\nautonumber 10 5\nA->>B: a\nA->>B: b\nautonumber off\nA->>B: c\n"),
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
