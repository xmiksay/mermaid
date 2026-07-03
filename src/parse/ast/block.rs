//! block-beta AST types.

use super::*;
use std::collections::HashMap;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BlockDiagram {
    pub columns: Option<usize>,
    pub items: Vec<BlockItem>,
    /// `classDef <name> <props>` style classes, keyed by class name.
    pub class_defs: HashMap<String, Style>,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum BlockItem {
    Block(Block),
    Group(BlockGroup),
    Space(usize),
    Edge(BlockEdge),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Block {
    pub id: String,
    pub label: String,
    pub shape: BlockShape,
    /// Optional column-span like `a["wide"]:2`.
    pub span: usize,
    /// `:::className` refs plus any `class a,b name` assignments.
    pub classes: Vec<String>,
    /// Inline `style <id> <props>` declarations.
    pub style: Style,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum BlockShape {
    #[default]
    Rect,
    Round,
    Stadium,
    Cylinder,
    Circle,
    Rhombus,
    Hexagon,
    /// Block arrow `id<["label"]>(dir)`, pointing along the set directions.
    Arrow(BlockArrow),
}

/// Directions a block arrow (`<[…]>(right)`) points. Multiple may be set,
/// e.g. `(x)` sets both `left` and `right`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlockArrow {
    pub right: bool,
    pub left: bool,
    pub up: bool,
    pub down: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockGroup {
    pub id: String,
    pub label: Option<String>,
    pub columns: Option<usize>,
    pub items: Vec<BlockItem>,
    /// Optional column-span from `block:id:span`.
    pub span: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockEdge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub arrow: bool,
}
