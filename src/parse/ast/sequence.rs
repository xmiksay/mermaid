//! Sequence-diagram AST types.

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SequenceDiagram {
    pub title: Option<String>,
    pub participants: Vec<Participant>,
    pub items: Vec<SequenceItem>,
    /// True if any `autonumber` on-directive was seen. Message numbering itself
    /// is positional — see [`SequenceItem::AutoNumber`].
    pub autonumber: bool,
    /// True when this diagram came from the `zenuml` parser. ZenUML reuses the
    /// sequence AST but renders with its own chrome: activation bars from call
    /// nesting, hierarchical `1.1.1` numbering, top-only boxed participants,
    /// suppressed synthesized returns, and a title frame.
    pub zenuml: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Participant {
    pub id: String,
    pub display: String,
    pub kind: ParticipantKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParticipantKind {
    Participant,
    Actor,
    /// ZenUML `@Boundary` — UML boundary stereotype (circle with a bar).
    Boundary,
    /// ZenUML `@Control` — UML control stereotype (circle with an arrow).
    Control,
    /// ZenUML `@Entity` — UML entity stereotype (underlined circle).
    Entity,
    /// ZenUML `@Database` — persistence stereotype (a cylinder).
    Database,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum SequenceItem {
    Message(Message),
    Note(SequenceNote),
    Activate(String),
    Deactivate(String),
    /// `alt label / else label / end`
    Alt(Vec<AltBranch>),
    /// `loop label ... end`
    Loop(SequenceBlock),
    /// `par label / and label / end`
    Par(Vec<AltBranch>),
    /// `opt label ... end`
    Opt(SequenceBlock),
    /// `critical label / option label / end`
    Critical(Vec<AltBranch>),
    /// `break label ... end` — draws a labeled frame breaking out of a loop
    Break(SequenceBlock),
    /// `rect <color> ... end` — colored background band behind the items
    Rect(SequenceRect),
    /// `box label ... end` — wraps participants
    Box(SequenceBox),
    /// `autonumber [start [step]]` / `autonumber off` — toggles message
    /// numbering positionally. `Some` turns it on (resetting the counter to
    /// `start`), `None` turns it off for subsequent messages.
    AutoNumber(Option<AutoNumberConfig>),
    /// `create [participant|actor] X` — the named participant's lifeline (and
    /// box) is spawned at this point instead of the top actor row. The
    /// participant itself is registered in [`SequenceDiagram::participants`].
    Create(String),
    /// `destroy X` — the named participant's lifeline terminates here with a
    /// cross glyph, and no bottom actor box is drawn.
    Destroy(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutoNumberConfig {
    /// Starting number. Fractional since v11.15 (`autonumber 1.5 0.5`), so it is
    /// an `f64` — an integral value still renders without a decimal point.
    pub start: f64,
    pub step: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SequenceRect {
    /// Background fill from `rect <color>` (hex, `rgb()/rgba()`, or a named
    /// CSS color). `None` renders a light default band.
    pub color: Option<String>,
    pub items: Vec<SequenceItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AltBranch {
    pub label: String,
    pub items: Vec<SequenceItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SequenceBlock {
    pub label: String,
    pub items: Vec<SequenceItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SequenceBox {
    /// Background fill from the `box <color> <label>` syntax; `None` renders
    /// transparent (upstream default).
    pub color: Option<String>,
    pub label: String,
    pub participant_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SequenceNote {
    pub position: NotePosition,
    pub participants: Vec<String>,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum NotePosition {
    Over,
    LeftOf,
    RightOf,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub from: String,
    pub to: String,
    pub text: String,
    pub arrow: ArrowKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ArrowKind {
    /// `->`  solid line, no arrowhead
    Solid,
    /// `->>` solid line, arrowhead
    SolidArrow,
    /// `-->` dashed line, no arrowhead
    Dashed,
    /// `-->>` dashed line, arrowhead
    DashedArrow,
    /// `-x` cross terminator, solid line
    Cross,
    /// `--x` cross terminator, dashed line
    DashedCross,
    /// `-)` open arrow (async), solid line
    Open,
    /// `--)` open arrow (async), dashed line
    DashedOpen,
    /// `<<->>` solid line, filled arrowhead at both ends (bidirectional)
    BiSolidArrow,
    /// `<<-->>` dashed line, filled arrowhead at both ends (bidirectional)
    BiDashedArrow,
    /// `-\\` / `-|\` solid line, upper-barb half arrowhead at the head (v11.12.3+)
    HalfArrowTop,
    /// `-//` / `-|/` solid line, lower-barb half arrowhead at the head
    HalfArrowBottom,
    /// `--\\` / `--|\` dashed line, upper-barb half arrowhead at the head
    DashedHalfArrowTop,
    /// `--//` / `--|/` dashed line, lower-barb half arrowhead at the head
    DashedHalfArrowBottom,
    /// `\\-` / `\|-` solid line, upper-barb half arrowhead at the tail (reverse)
    HalfArrowStartTop,
    /// `//-` / `/|-` solid line, lower-barb half arrowhead at the tail (reverse)
    HalfArrowStartBottom,
    /// `\\--` / `\|--` dashed line, upper-barb half arrowhead at the tail (reverse)
    DashedHalfArrowStartTop,
    /// `//--` / `/|--` dashed line, lower-barb half arrowhead at the tail (reverse)
    DashedHalfArrowStartBottom,
}
