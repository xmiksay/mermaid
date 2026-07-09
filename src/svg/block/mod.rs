//! block-beta renderer. Grid layout: items flow into cells by column count;
//! a composite `block:id … end` is a solid container that hugs its children at
//! their natural size (never shrinking them below text size) and claims the
//! whole grid columns it needs (#310, correcting #259).

mod draw;
mod edges;
mod layout;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use crate::parse::{BlockDiagram, BlockItem, BlockShape};

use super::builder::SvgBuilder;
use super::theme::Theme;

use draw::draw;
use edges::draw_edge;
use layout::{cell_dims, layout_items};

/// Center + bounding box + shape of a laid-out node, keyed by id for edge
/// routing. `shape` is `None` for a composite group (clipped as a rectangle).
pub(super) struct Geom {
    pub(super) cx: f64,
    pub(super) cy: f64,
    pub(super) w: f64,
    pub(super) h: f64,
    pub(super) shape: Option<BlockShape>,
}

/// Outer canvas margin — upstream's small `diagramPadding`, not the old 30px.
const PAD: f64 = 8.0;
/// Per-glyph width for block labels (shared with the flowchart estimate).
const CHAR_W: f64 = 7.5;
/// Horizontal text padding each side — boxes hug their label (#259).
const PAD_X: f64 = 14.0;
/// Uniform row height; tuned to upstream's compact hug, not the old 60px.
const CELL_H: f64 = 38.0;
/// Floor for the content-derived uniform column width.
const MIN_CELL_W: f64 = 40.0;
const GAP: f64 = 8.0;
/// Inner padding between a composite container and its natural-size children.
const GROUP_PAD: f64 = 8.0;

/// Maps a laid-out child frame (local coords) onto its parent frame:
/// `parent = (tx, ty) + s · local`. Composite groups scale their children
/// into a single grid slot, so nested geometry composes these.
#[derive(Clone, Copy)]
struct Transform {
    tx: f64,
    ty: f64,
    s: f64,
}

impl Transform {
    const IDENTITY: Transform = Transform {
        tx: 0.0,
        ty: 0.0,
        s: 1.0,
    };

    /// `self ∘ inner` — apply `inner` (child frame) then `self`.
    fn compose(&self, inner: &Transform) -> Transform {
        Transform {
            tx: self.tx + self.s * inner.tx,
            ty: self.ty + self.s * inner.ty,
            s: self.s * inner.s,
        }
    }
}

#[derive(Clone)]
struct Laid {
    item: BlockItem,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    /// Children in this node's own (local, pre-scale) coordinate frame.
    children: Vec<Laid>,
    /// For a composite group: maps `children` onto this frame (translate to the
    /// padded container; scale stays 1). `None` for leaf blocks and edges.
    child_tf: Option<Transform>,
}

pub(crate) fn render(d: &BlockDiagram, theme: &Theme) -> String {
    let (cw, ch) = cell_dims(&d.items, theme.font_size);
    let (laid, total_w, total_h) = layout_items(&d.items, d.columns.unwrap_or(3), PAD, PAD, cw, ch);
    let width = PAD * 2.0 + total_w;
    let height = PAD * 2.0 + total_h;
    let mut svg = SvgBuilder::new(width.max(120.0), height.max(60.0)).theme(theme);

    // Resolve node geometry (recursively) for edges — leaf blocks *and*
    // composite groups, so an edge can target a `block:ID … end` group.
    // `tf` maps each frame onto the absolute canvas so scaled children of a
    // composite group report their on-screen box, not their pre-scale one.
    let mut nodes: BTreeMap<String, Geom> = BTreeMap::new();
    fn collect(laid: &[Laid], out: &mut BTreeMap<String, Geom>, tf: &Transform) {
        for l in laid {
            let cx = tf.tx + tf.s * (l.x + l.w / 2.0);
            let cy = tf.ty + tf.s * (l.y + l.h / 2.0);
            let w = l.w * tf.s;
            let h = l.h * tf.s;
            match &l.item {
                BlockItem::Block(b) => {
                    out.insert(
                        b.id.clone(),
                        Geom {
                            cx,
                            cy,
                            w,
                            h,
                            shape: Some(b.shape),
                        },
                    );
                }
                BlockItem::Group(g) => {
                    if !g.id.is_empty() {
                        out.insert(
                            g.id.clone(),
                            Geom {
                                cx,
                                cy,
                                w,
                                h,
                                shape: None,
                            },
                        );
                    }
                    if let Some(inner) = &l.child_tf {
                        collect(&l.children, out, &tf.compose(inner));
                    }
                }
                _ => {}
            }
        }
    }
    collect(&laid, &mut nodes, &Transform::IDENTITY);

    // Draw items.
    for l in &laid {
        draw(l, &mut svg, theme, &d.class_defs);
    }
    // Draw edges last so they sit on top.
    for l in &laid {
        if let BlockItem::Edge(e) = &l.item {
            draw_edge(e, &nodes, &mut svg, theme);
        }
    }

    svg.finish()
}
