//! Sequence-diagram AST types.

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SequenceDiagram {
    pub title: Option<String>,
    pub participants: Vec<Participant>,
    pub items: Vec<SequenceItem>,
    /// True if any `autonumber` on-directive was seen. Message numbering itself
    /// is positional — see [`SequenceItem::AutoNumber`].
    pub autonumber: bool,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoNumberConfig {
    pub start: u32,
    pub step: u32,
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
    /// `-x` / `--x` cross terminator
    Cross,
    /// `-)` / `--)` open arrow (async)
    Open,
    /// `<<->>` solid line, filled arrowhead at both ends (bidirectional)
    BiSolidArrow,
    /// `<<-->>` dashed line, filled arrowhead at both ends (bidirectional)
    BiDashedArrow,
}
