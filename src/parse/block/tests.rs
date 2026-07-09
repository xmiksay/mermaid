use super::*;
use crate::parse::ast::{Block, BlockLinkStyle, BlockShape, EdgeHead};

#[test]
fn basic_grid() {
    let d = parse("block-beta\ncolumns 3\na b c\nd[\"wide\"]:2 e\n").unwrap();
    assert_eq!(d.columns, Some(3));
    assert_eq!(d.items.len(), 5);
    match &d.items[3] {
        BlockItem::Block(b) => {
            assert_eq!(b.id, "d");
            assert_eq!(b.label, "wide");
            assert_eq!(b.span, 2);
        }
        _ => panic!(),
    }
}

#[test]
fn group() {
    let d = parse("block-beta\nblock:g\n  x y\nend\n").unwrap();
    assert_eq!(d.items.len(), 1);
    match &d.items[0] {
        BlockItem::Group(g) => {
            assert_eq!(g.id, "g");
            assert_eq!(g.items.len(), 2);
        }
        _ => panic!(),
    }
}

#[test]
fn classdef_class_and_style_not_ghost_blocks() {
    let d = parse(
        "block-beta\n  a b\n  classDef blue fill:#66f,stroke:#333\n  class a blue\n  style b fill:#0f0\n",
    )
    .unwrap();
    // Only the two real blocks survive — no ghost blocks for the keywords.
    assert_eq!(d.items.len(), 2);
    assert!(d.class_defs.contains_key("blue"));
    match &d.items[0] {
        BlockItem::Block(b) => assert_eq!(b.classes, vec!["blue".to_string()]),
        _ => panic!(),
    }
    match &d.items[1] {
        BlockItem::Block(b) => assert_eq!(b.style, vec![("fill".into(), "#0f0".into())]),
        _ => panic!(),
    }
}

#[test]
fn inline_class_shorthand() {
    let d = parse("block-beta\n  a[\"A\"]:::warn\n").unwrap();
    match &d.items[0] {
        BlockItem::Block(b) => {
            assert_eq!(b.id, "a");
            assert_eq!(b.label, "A");
            assert_eq!(b.classes, vec!["warn".to_string()]);
        }
        _ => panic!(),
    }
}

#[test]
fn edge_label_keeps_edge() {
    let d = parse("block-beta\n  a b\n  a -- \"hello\" --> b\n").unwrap();
    let edges: Vec<_> = d
        .items
        .iter()
        .filter_map(|i| match i {
            BlockItem::Edge(e) => Some(e),
            _ => None,
        })
        .collect();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].from, "a");
    assert_eq!(edges[0].to, "b");
    assert_eq!(edges[0].label.as_deref(), Some("hello"));
    assert_eq!(edges[0].head, EdgeHead::Arrow);
    assert_eq!(edges[0].tail, EdgeHead::None);
}

#[test]
fn block_arrow_parses() {
    let d = parse("block-beta\n  blockArrowId<[\"label\"]>(right)\n").unwrap();
    match &d.items[0] {
        BlockItem::Block(b) => {
            assert_eq!(b.id, "blockArrowId");
            assert_eq!(b.label, "label");
            assert!(matches!(
                b.shape,
                BlockShape::Arrow(a) if a.right && !a.left && !a.up && !a.down
            ));
        }
        _ => panic!(),
    }
}

fn blocks(d: &BlockDiagram) -> Vec<&Block> {
    d.items
        .iter()
        .filter_map(|i| match i {
            BlockItem::Block(b) => Some(b),
            _ => None,
        })
        .collect()
}

#[test]
fn subroutine_and_double_circle_shapes() {
    let d = parse("block-beta\n  a[[\"subroutine\"]] b(((\"double\")))\n").unwrap();
    let bs = blocks(&d);
    assert_eq!(bs[0].label, "subroutine");
    assert_eq!(bs[0].shape, BlockShape::Subroutine);
    assert_eq!(bs[1].label, "double");
    assert_eq!(bs[1].shape, BlockShape::DoubleCircle);
}

#[test]
fn asymmetric_and_lean_shapes_dont_mangle_line() {
    let d =
        parse("block-beta\n  a>\"asym\"] b[/\"lr\"/] c[\\\"ll\"\\] d[/\"tz\"\\] e[\\\"tza\"/]\n")
            .unwrap();
    let bs = blocks(&d);
    assert_eq!(bs.len(), 5);
    assert_eq!(bs[0].shape, BlockShape::Odd);
    assert_eq!(bs[0].label, "asym");
    assert_eq!(bs[1].shape, BlockShape::LeanRight);
    assert_eq!(bs[2].shape, BlockShape::LeanLeft);
    assert_eq!(bs[3].shape, BlockShape::Trapezoid);
    assert_eq!(bs[4].shape, BlockShape::TrapezoidAlt);
}

#[test]
fn dotted_thick_invisible_links() {
    let d = parse("block-beta\n  a b c d\n  a -.-> b\n  b ==> c\n  c ~~~ d\n").unwrap();
    let edges: Vec<_> = d
        .items
        .iter()
        .filter_map(|i| match i {
            BlockItem::Edge(e) => Some(e),
            _ => None,
        })
        .collect();
    assert_eq!(edges.len(), 3);
    assert_eq!(edges[0].style, BlockLinkStyle::Dotted);
    assert_eq!(edges[0].head, EdgeHead::Arrow);
    assert_eq!(edges[1].style, BlockLinkStyle::Thick);
    assert_eq!(edges[2].style, BlockLinkStyle::Invisible);
}

#[test]
fn cross_and_circle_headed_links() {
    // Formerly a #169 residual: `--x`/`--o`/`==x`/`==o` fell through to block
    // parsing and produced ghost blocks. They now parse as headed edges.
    let d =
        parse("block-beta\n  a b c d e f\n  a --x b\n  b --o c\n  c ==x d\n  d ==o e\n").unwrap();
    let edges: Vec<_> = d
        .items
        .iter()
        .filter_map(|i| match i {
            BlockItem::Edge(e) => Some(e),
            _ => None,
        })
        .collect();
    assert_eq!(edges.len(), 4);
    assert_eq!(
        (edges[0].style, edges[0].head),
        (BlockLinkStyle::Solid, EdgeHead::Cross)
    );
    assert_eq!(
        (edges[1].style, edges[1].head),
        (BlockLinkStyle::Solid, EdgeHead::Circle)
    );
    assert_eq!(
        (edges[2].style, edges[2].head),
        (BlockLinkStyle::Thick, EdgeHead::Cross)
    );
    assert_eq!(
        (edges[3].style, edges[3].head),
        (BlockLinkStyle::Thick, EdgeHead::Circle)
    );
    // No ghost blocks were emitted for the connectors.
    let ids: Vec<&str> = blocks(&d).iter().map(|b| b.id.as_str()).collect();
    assert_eq!(ids, vec!["a", "b", "c", "d", "e", "f"]);
}

#[test]
fn tail_marked_links() {
    // `<-->`/`x--x`/`o--o` carry a marker on the `from` end too. A node id
    // ending in `o` (`foo`) must not be misread as a circle tail.
    let d = parse("block-beta\n  a b foo c\n  a <--> b\n  b x--x foo\n  foo o--o c\n").unwrap();
    let edges: Vec<_> = d
        .items
        .iter()
        .filter_map(|i| match i {
            BlockItem::Edge(e) => Some(e),
            _ => None,
        })
        .collect();
    assert_eq!(edges.len(), 3);
    assert_eq!(
        (edges[0].tail, edges[0].head),
        (EdgeHead::Arrow, EdgeHead::Arrow)
    );
    assert_eq!(
        (edges[1].tail, edges[1].head),
        (EdgeHead::Cross, EdgeHead::Cross)
    );
    assert_eq!(
        (edges[2].from.as_str(), edges[2].tail),
        ("foo", EdgeHead::Circle)
    );
}

#[test]
fn columns_auto_packs_one_row() {
    let d = parse("block-beta\n  columns auto\n  a b c d\n").unwrap();
    assert_eq!(d.columns, Some(4));
}

#[test]
fn space_prefixed_id_is_not_a_space() {
    let d = parse("block-beta\n  spaceship[\"Ship\"] space\n").unwrap();
    let bs = blocks(&d);
    assert_eq!(bs.len(), 1);
    assert_eq!(bs[0].id, "spaceship");
    assert_eq!(bs[0].label, "Ship");
    assert!(matches!(d.items[1], BlockItem::Space(1)));
}

#[test]
fn style_multi_id_list() {
    let d = parse("block-beta\n  a b\n  style a,b fill:#f00\n").unwrap();
    let bs = blocks(&d);
    assert_eq!(bs[0].style, vec![("fill".into(), "#f00".into())]);
    assert_eq!(bs[1].style, vec![("fill".into(), "#f00".into())]);
}

#[test]
fn composite_block_span_kept() {
    let d = parse("block-beta\n  block:wide:2\n    x\n  end\n").unwrap();
    match &d.items[0] {
        BlockItem::Group(g) => {
            assert_eq!(g.id, "wide");
            assert_eq!(g.span, 2);
        }
        _ => panic!(),
    }
}
