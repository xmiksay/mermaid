//! Class-diagram AST types.

use super::*;
use std::collections::HashMap;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClassDiagram {
    pub direction: FlowDirection,
    pub classes: Vec<UmlClass>,
    pub relations: Vec<ClassRelation>,
    pub namespaces: Vec<Namespace>,
    /// `classDef <name> …` definitions, keyed by class name.
    pub class_defs: HashMap<String, Style>,
    /// `note "…"` (free) and `note for <Class> "…"` (attached) annotations.
    pub notes: Vec<ClassNote>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UmlClass {
    pub name: String,
    /// Display label from `class Name["label"]`; falls back to `name`.
    pub label: Option<String>,
    pub stereotype: Option<String>,
    pub members: Vec<ClassMember>,
    /// Style class names applied via `cssClass`/`:::`.
    pub classes: Vec<String>,
    /// Inline `style <Name> …` properties.
    pub style: Style,
    /// Interaction bound via `click`/`link`/`callback`, if any. Reuses the
    /// flowchart [`ClickAction`] model.
    pub click: Option<ClickAction>,
}

/// A `note "text"` (free-floating) or `note for <Class> "text"` (attached to a
/// class) annotation — a yellow sticky box in the rendered diagram.
#[derive(Debug, Clone, PartialEq)]
pub struct ClassNote {
    /// Class the note is attached to (`note for X …`); `None` is a free note.
    pub target: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassMember {
    pub visibility: Visibility,
    pub text: String,
    pub kind: MemberKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Visibility {
    #[default]
    Default,
    Public,
    Private,
    Protected,
    Package,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MemberKind {
    Attribute,
    Method,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassRelation {
    pub from: String,
    pub to: String,
    pub kind: ClassRelationKind,
    pub label: Option<String>,
    /// Multiplicity on the `from` end, e.g. `"1"` in `A "1" --> "*" B`.
    pub from_card: Option<String>,
    /// Multiplicity on the `to` end, e.g. `"*"` in `A "1" --> "*" B`.
    pub to_card: Option<String>,
    /// True when the relation token's decorated end (triangle/diamond/circle/
    /// arrow) sits on the `from` class rather than `to` — e.g. `<|--`, `*--`,
    /// `o--`, `<--`, `<..`. The renderer then draws the marker at the `from`
    /// end. Layout order (`from` → `to`) is preserved either way.
    pub reversed: bool,
    /// A second decorated marker on the `to` end for two-way relations
    /// (`relationType lineType relationType`, e.g. `<|--|>`, `*--*`, `o--o`,
    /// `<-->`, `<..>`). `kind`/`reversed` mark the `from` end; this marks the
    /// `to` end. `None` for the common single-ended relations.
    pub to_kind: Option<ClassRelationKind>,
    /// A lollipop-interface socket circle sits on the `from` end — the `()`
    /// preceded the relation token (`bar ()-- foo`).
    pub lollipop_from: bool,
    /// A lollipop-interface socket circle sits on the `to` end — the `()`
    /// followed the relation token (`foo --() bar`).
    pub lollipop_to: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ClassRelationKind {
    /// `<|--` inheritance
    Inheritance,
    /// `*--` composition
    Composition,
    /// `o--` aggregation
    Aggregation,
    /// `-->` association with arrow
    Association,
    /// `--` plain link
    Link,
    /// `..` dashed link
    LinkDashed,
    /// `..|>` realization (dashed)
    Realization,
    /// `..>` dependency (dashed)
    Dependency,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Namespace {
    pub name: String,
    /// Display label from `namespace Name["label"]`; falls back to `name`.
    pub label: Option<String>,
    /// Nesting level, 0 for an outermost namespace. Deeper namespaces draw a
    /// smaller frame so an outer namespace visibly encloses its nested ones.
    pub depth: usize,
    pub class_names: Vec<String>,
}
