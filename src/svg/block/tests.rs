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
    let (cw, ch) = cell_dims(&d.items, Theme::default().font_size);
    let (laid, _, _) = layout_items(&d.items, 3, PAD, PAD, cw, ch);
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
