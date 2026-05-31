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
    pub items: Vec<SequenceItem>,
    pub autonumber: bool,
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
    /// `box label ... end` — wraps participants
    Box(SequenceBox),
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
    pub subgraphs: Vec<Subgraph>,
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
    /// `(((text)))` — double circle
    DoubleCircle,
    /// `{text}` — rhombus (decision)
    Rhombus,
    /// `{{text}}` — hexagon
    Hexagon,
    /// `[/text/]` — parallelogram (input/output)
    Parallelogram,
    /// `[\text\]` — parallelogram opposite
    ParallelogramAlt,
    /// `[/text\]` — trapezoid (manual input)
    Trapezoid,
    /// `[\text/]` — trapezoid alt (manual output)
    TrapezoidAlt,
    /// `>text]` — asymmetric flag
    Asymmetric,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlowEdge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub line: EdgeLine,
    pub head: EdgeHead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeLine {
    /// `-` solid
    Solid,
    /// `.` dotted
    Dotted,
    /// `=` thick
    Thick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeHead {
    /// `---` no head
    None,
    /// `-->` filled arrow head
    Arrow,
    /// `--o` open circle head
    Circle,
    /// `--x` cross head
    Cross,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Subgraph {
    pub id: String,
    pub label: String,
    pub direction: Option<FlowDirection>,
    pub node_ids: Vec<String>,
    pub child_subgraph_ids: Vec<String>,
}

// ---- state diagram ---------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct StateDiagram {
    pub direction: FlowDirection,
    pub states: Vec<State>,
    pub transitions: Vec<StateTransition>,
    pub composites: Vec<CompositeState>,
    pub notes: Vec<StateNote>,
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

#[derive(Debug, Clone, PartialEq)]
pub struct CompositeState {
    pub id: String,
    /// Each region is an ordered list of child state ids (multi-region
    /// composites use the `--` separator to split into parallel regions).
    pub regions: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StateNote {
    pub target: String,
    pub position: NotePosition,
    pub text: String,
}

// ---- class diagram ---------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClassDiagram {
    pub direction: FlowDirection,
    pub classes: Vec<UmlClass>,
    pub relations: Vec<ClassRelation>,
    pub namespaces: Vec<Namespace>,
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
    pub class_names: Vec<String>,
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
    pub excludes: Vec<String>,
    pub today_marker: Option<String>,
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
