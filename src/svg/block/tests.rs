use super::layout::{cell_dims, layout_items};
use super::{render, CELL_H, GROUP_PAD, PAD};
use crate::parse::{Block, BlockDiagram, BlockItem, BlockShape};
use crate::svg::theme::Theme;

#[test]
fn produces_svg() {
    let d = BlockDiagram {
        columns: Some(2),
        items: vec![
            BlockItem::Block(Block {
                id: "a".into(),
                label: "A".into(),
                shape: BlockShape::Rect,
                span: 1,
                ..Block::default()
            }),
            BlockItem::Block(Block {
                id: "b".into(),
                label: "B".into(),
                shape: BlockShape::Circle,
                span: 1,
                ..Block::default()
            }),
        ],
        ..BlockDiagram::default()
    };
    let svg = render(&d, &Theme::default());
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains(">A<"));
    assert!(svg.contains(">B<"));
}

#[test]
fn classdef_style_and_edge_label() {
    let src = "block-beta\n  columns 2\n  a b\n  classDef hot fill:#f00,stroke:#900\n  class a hot\n  a -- \"link\" --> b\n";
    let svg = render_from(src);
    assert!(svg.contains("#f00"));
    assert!(svg.contains(">link<"));
    // #260: the edge label sits on an opaque background rect so it stays
    // legible where the edge crosses a node.
    let rect = svg.find("fill=\"#e8e8e8\" stroke=\"none\"");
    let label = svg.find(">link<");
    assert!(rect.is_some() && rect < label);
    // no ghost blocks for the classDef/class keywords
    assert!(!svg.contains(">classDef<"));
    assert!(!svg.contains(">hot<"));
}

/// Bounds `(x0, y0, x1, y1)` of the opaque background rect drawn directly
/// before the `text-anchor=middle` label for `id`.
fn label_bg_box(svg: &str, id: &str) -> (f64, f64, f64, f64) {
    let text = svg.find(&format!(">{id}</text>")).unwrap();
    let rect_open = svg[..text].rfind("<rect ").unwrap();
    let rect_end = rect_open + svg[rect_open..].find("/>").unwrap();
    let tag = &svg[rect_open..rect_end];
    let grab = |attr: &str| {
        let s = tag.find(attr).unwrap() + attr.len();
        let e = s + tag[s..].find('"').unwrap();
        tag[s..e].parse::<f64>().unwrap()
    };
    let (x, y, w, h) = (
        grab("x=\""),
        grab("y=\""),
        grab("width=\""),
        grab("height=\""),
    );
    (x, y, x + w, y + h)
}

#[test]
fn non_opposite_pair_labels_do_not_overlap() {
    // Two unrelated edges (a->d, b->c — not a reversed pair) whose natural
    // midpoints land close together; without the general overlap-avoidance
    // pass their opaque label backgrounds would occlude one another.
    let src = "block-beta\ncolumns 5\na space space b\nspace c d space\na -- \"path one\" --> d\nb -- \"path two\" --> c\n";
    let svg = render_from(src);
    let (ax0, ay0, ax1, ay1) = label_bg_box(&svg, "path one");
    let (bx0, by0, bx1, by1) = label_bg_box(&svg, "path two");
    let overlap = ax0 < bx1 && bx0 < ax1 && ay0 < by1 && by0 < ay1;
    assert!(
        !overlap,
        "label backgrounds overlap: path one ({ax0},{ay0},{ax1},{ay1}) vs path two ({bx0},{by0},{bx1},{by1})",
    );
}

#[test]
fn adjacent_cell_edge_labels_clear_neighboring_boxes_and_each_other() {
    // block/07 sample repro: two labeled edges back-to-back in the same row
    // (a--b, b--c) plus a labeled vertical edge (c-f) in a tightly packed
    // grid — the default 8px cell gap leaves no room for a label between
    // adjacent cells, so the grid gap must widen to fit the widest label.
    let src = "block-beta\n  columns 3\n  a b c\n  d e f\n  a -- \"solid label\" --> b\n  b -. \"dotted label\" -.-> c\n  c == \"thick label\" ==> f\n";
    let svg = render_from(src);
    let a = label_bg_box(&svg, "solid label");
    let b = label_bg_box(&svg, "dotted label");
    let c = label_bg_box(&svg, "thick label");
    let overlaps = |r1: (f64, f64, f64, f64), r2: (f64, f64, f64, f64)| {
        r1.0 < r2.2 && r2.0 < r1.2 && r1.1 < r2.3 && r2.1 < r1.3
    };
    assert!(!overlaps(a, b), "solid/dotted labels overlap: {a:?} {b:?}");
    assert!(!overlaps(a, c), "solid/thick labels overlap: {a:?} {c:?}");
    assert!(!overlaps(b, c), "dotted/thick labels overlap: {b:?} {c:?}");
}

#[test]
fn tail_only_reversed_arrow_has_no_ghost_blocks() {
    // Bug #1 downstream check: a bare `<--` (two dashes, no head char) isn't
    // valid upstream syntax and correctly falls through to block tokenizing,
    // but the valid three-dash form `<---` must parse as a real edge with a
    // tail marker and no head, not spawn ghost blocks for its own tokens.
    let src = "block-beta\n  columns 3\n  a b c\n  d e f\n  g h i\n  d <--- e\n";
    let svg = render_from(src);
    // No ghost blocks for the connector tokens.
    assert!(!svg.contains(">&lt;---<"));
    assert!(!svg.contains(">---<"));
    // marker-start (tail arrowhead), no marker-end (headless).
    assert!(svg.contains("marker-start=\"url(#blockarrow)\""));
    let d = crate::parse::parse(src).unwrap();
    let crate::parse::Diagram::Block(d) = d else {
        panic!("expected block");
    };
    let edges: Vec<_> = d
        .items
        .iter()
        .filter_map(|i| match i {
            crate::parse::BlockItem::Edge(e) => Some(e),
            _ => None,
        })
        .collect();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].from, "d");
    assert_eq!(edges[0].to, "e");
    assert_eq!(edges[0].head, crate::parse::EdgeHead::None);
    assert_eq!(edges[0].tail, crate::parse::EdgeHead::Arrow);
    // Still a clean 3x3 grid — 9 leaf blocks, no extras from the edge line.
    let blocks = d
        .items
        .iter()
        .filter(|i| matches!(i, BlockItem::Block(_)))
        .count();
    assert_eq!(blocks, 9);
}

#[test]
fn block_arrow_renders_path() {
    let svg = render_from("block-beta\n  a<[\"go\"]>(right)\n");
    assert!(svg.contains("<path"));
    assert!(svg.contains(">go<"));
}

#[test]
fn composite_group_is_solid_untitled_and_scaled() {
    // #259: a composite block draws a solid pale container (theme cluster
    // fill), no dashed frame, no title text, with its children inside a
    // group transform.
    let src = "block-beta\n  columns 3\n  a b c\n  block:group1\n    x y z\n  end\n";
    let svg = render_from(src);
    let t = Theme::default();
    assert!(svg.contains(&format!("fill=\"{}\"", t.flow_cluster_fill)));
    assert!(!svg.contains("stroke-dasharray=\"5 4\""));
    // no bold title label for the group id
    assert!(!svg.contains(">group1<"));
    // children still rendered, inside a group transform
    assert!(svg.contains(">x<") && svg.contains(">z<"));
    assert!(svg.contains("<g transform=\"translate("));
}

#[test]
fn composite_children_keep_natural_size() {
    // #310: the #259 compaction over-shrank children into ~10px dots. The
    // container must hug them at natural scale (no shrink) so labels stay
    // legible, and it must be at least as tall as one cell plus its pad.
    let src = "block-beta\n  columns 3\n  a b c\n  block:group1\n    x y z\n  end\n";
    let svg = render_from(src);
    // children rendered at scale 1 — never scaled down.
    assert!(svg.contains("scale(1)"));
    assert!(!svg.contains("scale(0"));
    // container tall enough for a full cell + inner pad on both sides.
    let d = match crate::parse::parse(src).unwrap() {
        crate::parse::Diagram::Block(d) => d,
        _ => unreachable!(),
    };
    let (cw, ch) = cell_dims(&d.items, Theme::default().font_size, super::GAP);
    let (laid, _, _) = layout_items(
        &d.items,
        3,
        PAD,
        PAD,
        cw,
        ch,
        Theme::default().font_size,
        super::GAP,
    );
    let group = laid
        .iter()
        .find(|l| matches!(l.item, BlockItem::Group(_)))
        .unwrap();
    assert!(group.h >= CELL_H + GROUP_PAD * 2.0);
    assert_eq!(group.child_tf.unwrap().s, 1.0);
}

#[test]
fn composite_group_occupies_one_slot_not_full_row() {
    // The container hugs a single grid slot, so a sibling that follows it
    // shares the row instead of being pushed below a full-width group.
    let src = "block-beta\n  columns 3\n  block:g\n    x\n  end\n  sib\n";
    let svg = render_from(src);
    // The whole canvas is ~3 columns wide, well under the old full-row size.
    let width = svg
        .split("viewBox=\"0 0 ")
        .nth(1)
        .and_then(|s| s.split_whitespace().next())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap();
    assert!(width < 200.0, "canvas unexpectedly wide: {width}");
}

#[test]
fn edge_to_composite_group() {
    let src = "block-beta\n  block:G\n    x\n  end\n  y\n  G --> y\n";
    let svg = render_from(src);
    // one edge line drawn (marker present) — group id resolves as a node
    assert!(svg.contains("marker-end=\"url(#blockarrow)\""));
}

#[test]
fn cross_and_circle_head_markers() {
    let svg = render_from("block-beta\n  a b c\n  a --x b\n  b --o c\n");
    assert!(svg.contains("marker-end=\"url(#blockcross)\""));
    assert!(svg.contains("marker-end=\"url(#blockcircle)\""));
    assert!(svg.contains("id=\"blockcross\""));
    assert!(svg.contains("id=\"blockcircle\""));
}

#[test]
fn bidirectional_link_marks_both_ends() {
    let svg = render_from("block-beta\n  a b\n  a <--> b\n");
    assert!(svg.contains("marker-start=\"url(#blockarrow)\""));
    assert!(svg.contains("marker-end=\"url(#blockarrow)\""));
}

fn render_from(src: &str) -> String {
    match crate::parse::parse(src).unwrap() {
        crate::parse::Diagram::Block(d) => render(&d, &Theme::default()),
        _ => panic!("expected block"),
    }
}
