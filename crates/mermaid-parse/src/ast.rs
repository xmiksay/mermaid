//! AST types shared by all diagram kinds.

#[derive(Debug, Clone)]
pub enum Diagram {
    Pie(PieDiagram),
    Sequence(SequenceDiagram),
    Flowchart(FlowchartDiagram),
}

// ---- pie -------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PieDiagram {
    pub title: Option<String>,
    pub show_data: bool,
    pub entries: Vec<PieEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PieEntry {
    pub label: String,
    pub value: f64,
}

// ---- sequence --------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SequenceDiagram {
    pub title: Option<String>,
    pub participants: Vec<Participant>,
    pub messages: Vec<Message>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Participant {
    pub id: String,
    pub display: String,
    pub kind: ParticipantKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticipantKind {
    Participant,
    Actor,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub from: String,
    pub to: String,
    pub text: String,
    pub arrow: ArrowKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

// ---- flowchart -------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlowchartDiagram {
    pub direction: FlowDirection,
    pub nodes: Vec<FlowNode>,
    pub edges: Vec<FlowEdge>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FlowDirection {
    #[default]
    TopDown,
    BottomTop,
    LeftRight,
    RightLeft,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlowNode {
    pub id: String,
    pub text: String,
    pub shape: NodeShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeShape {
    /// `[text]` — rectangle (default)
    Rect,
    /// `(text)` — rounded rectangle
    Round,
    /// `([text])` — stadium
    Stadium,
    /// `[[text]]` — subroutine (double-line rect)
    Subroutine,
    /// `[(text)]` — cylinder (database)
    Cylinder,
    /// `((text))` — circle
    Circle,
    /// `{text}` — rhombus (decision)
    Rhombus,
    /// `{{text}}` — hexagon
    Hexagon,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlowEdge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
    /// `-->` solid line with arrow
    Solid,
    /// `---` solid line, no arrow
    SolidNoArrow,
    /// `-.->` dotted line with arrow
    Dotted,
    /// `==>` thick line with arrow
    Thick,
}
