//! AST types shared by all diagram kinds.

#[derive(Debug, Clone)]
pub enum Diagram {
    Pie(PieDiagram),
    Sequence(SequenceDiagram),
    Flowchart(FlowchartDiagram),
    State(StateDiagram),
    Class(ClassDiagram),
    Er(ErDiagram),
    Gantt(GanttDiagram),
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

// ---- state diagram ---------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct StateDiagram {
    pub direction: FlowDirection,
    pub states: Vec<State>,
    pub transitions: Vec<StateTransition>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub id: String,
    pub label: String,
    pub kind: StateKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateKind {
    Normal,
    Start,
    End,
    Choice,
    Fork,
    Join,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StateTransition {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
}

// ---- class diagram ---------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClassDiagram {
    pub classes: Vec<UmlClass>,
    pub relations: Vec<ClassRelation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UmlClass {
    pub name: String,
    pub stereotype: Option<String>,
    pub members: Vec<ClassMember>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassMember {
    pub visibility: Visibility,
    pub text: String,
    pub kind: MemberKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Visibility {
    #[default]
    Default,
    Public,
    Private,
    Protected,
    Package,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassRelationKind {
    /// `<|--` inheritance (from extends to)
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

// ---- ER diagram ------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ErDiagram {
    pub entities: Vec<Entity>,
    pub relations: Vec<ErRelation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    pub name: String,
    pub attributes: Vec<EntityAttribute>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntityAttribute {
    pub type_: String,
    pub name: String,
    pub key: Option<String>,
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

// ---- gantt -----------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GanttDiagram {
    pub title: Option<String>,
    pub date_format: Option<String>,
    pub axis_format: Option<String>,
    pub sections: Vec<GanttSection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GanttSection {
    pub name: String,
    pub tasks: Vec<GanttTask>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GanttTask {
    pub name: String,
    pub id: Option<String>,
    pub start: TaskStart,
    pub duration_days: f64,
    pub status: TaskStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStart {
    Date(String),
    AfterId(String),
    AfterPrevious,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TaskStatus {
    #[default]
    Normal,
    Active,
    Done,
    Crit,
}
