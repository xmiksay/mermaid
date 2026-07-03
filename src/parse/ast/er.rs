//! Entity-relationship (ER) AST types.

use super::*;
use std::collections::HashMap;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ErDiagram {
    pub entities: Vec<Entity>,
    pub relations: Vec<ErRelation>,
    /// `direction TB/BT/LR/RL`; drives the same layout transpose as flowchart.
    pub direction: FlowDirection,
    /// `classDef <name> <props>` style classes, resolved per entity.
    pub class_defs: HashMap<String, Style>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    /// Stable identifier used by relations.
    pub name: String,
    /// Display text; equals `name` unless an `id[Alias]` form set an alias.
    pub label: String,
    pub attributes: Vec<EntityAttribute>,
    /// CSS classes attached via `class <id> <name>` or the `id:::name` shorthand.
    pub classes: Vec<String>,
    /// Inline `style <id> <props>` declarations.
    pub style: Style,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityAttribute {
    pub type_: String,
    pub name: String,
    pub key: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ErRelation {
    pub left: String,
    pub right: String,
    pub left_card: Cardinality,
    pub right_card: Cardinality,
    pub identifying: bool,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Cardinality {
    /// `||` exactly one
    ExactlyOne,
    /// `o|` zero or one
    ZeroOrOne,
    /// `|{` one or more
    OneOrMore,
    /// `o{` zero or more
    ZeroOrMore,
}
