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
    Journey(JourneyDiagram),
    Timeline(TimelineDiagram),
    Sankey(SankeyDiagram),
    Quadrant(QuadrantDiagram),
    XyChart(XyChartDiagram),
    Radar(RadarDiagram),
    Packet(PacketDiagram),
    Mindmap(MindmapDiagram),
    GitGraph(GitGraphDiagram),
    Requirement(RequirementDiagram),
    C4(C4Diagram),
    Block(BlockDiagram),
    Architecture(ArchitectureDiagram),
    Kanban(KanbanDiagram),
    Treemap(TreemapDiagram),
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

// ---- journey ---------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct JourneyDiagram {
    pub title: Option<String>,
    pub sections: Vec<JourneySection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JourneySection {
    pub name: String,
    pub tasks: Vec<JourneyTask>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct JourneyTask {
    pub name: String,
    pub score: i32,
    pub actors: Vec<String>,
}

// ---- timeline --------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TimelineDiagram {
    pub title: Option<String>,
    pub sections: Vec<TimelineSection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimelineSection {
    /// `None` for events that appear before any explicit `section` block.
    pub name: Option<String>,
    pub periods: Vec<TimelinePeriod>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimelinePeriod {
    pub label: String,
    pub events: Vec<String>,
}

// ---- sankey ----------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SankeyDiagram {
    pub links: Vec<SankeyLink>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SankeyLink {
    pub source: String,
    pub target: String,
    pub value: f64,
}

// ---- quadrant --------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct QuadrantDiagram {
    pub title: Option<String>,
    pub x_axis_left: Option<String>,
    pub x_axis_right: Option<String>,
    pub y_axis_bottom: Option<String>,
    pub y_axis_top: Option<String>,
    pub q1: Option<String>,
    pub q2: Option<String>,
    pub q3: Option<String>,
    pub q4: Option<String>,
    pub points: Vec<QuadrantPoint>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QuadrantPoint {
    pub label: String,
    pub x: f64,
    pub y: f64,
}

// ---- xychart ---------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct XyChartDiagram {
    pub horizontal: bool,
    pub title: Option<String>,
    pub x_axis: Option<XyAxis>,
    pub y_axis: Option<XyAxis>,
    pub series: Vec<XySeries>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct XyAxis {
    pub title: Option<String>,
    pub kind: XyAxisKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum XyAxisKind {
    /// Categorical labels (e.g. month names).
    Categories(Vec<String>),
    /// Numeric range `min --> max`.
    Range { min: f64, max: f64 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct XySeries {
    pub kind: XySeriesKind,
    pub values: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XySeriesKind {
    Bar,
    Line,
}

// ---- radar -----------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RadarDiagram {
    pub title: Option<String>,
    pub axes: Vec<RadarAxis>,
    pub curves: Vec<RadarCurve>,
    /// Optional explicit max value; defaults to max observed.
    pub max: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadarAxis {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadarCurve {
    pub id: String,
    pub label: String,
    pub values: Vec<f64>,
}

// ---- packet ----------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PacketDiagram {
    pub title: Option<String>,
    pub fields: Vec<PacketField>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PacketField {
    pub start: u32,
    pub end: u32,
    pub label: String,
}

// ---- mindmap ---------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MindmapDiagram {
    pub root: Option<MindmapNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MindmapNode {
    pub text: String,
    pub shape: MindmapShape,
    pub icon: Option<String>,
    pub children: Vec<MindmapNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MindmapShape {
    /// Default — no explicit delimiters.
    Default,
    /// `[text]` — square
    Square,
    /// `(text)` — rounded square
    Rounded,
    /// `((text))` — circle
    Circle,
    /// `))text((` — bang / explosion
    Bang,
    /// `)text(` — cloud
    Cloud,
    /// `{{text}}` — hexagon
    Hexagon,
}

// ---- gitGraph --------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GitGraphDiagram {
    pub title: Option<String>,
    pub direction: GitDirection,
    pub events: Vec<GitEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GitDirection {
    #[default]
    LeftRight,
    TopDown,
    BottomTop,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GitEvent {
    Commit {
        id: Option<String>,
        tag: Option<String>,
        kind: CommitKind,
    },
    Branch {
        name: String,
    },
    Checkout {
        name: String,
    },
    Merge {
        from: String,
        id: Option<String>,
        tag: Option<String>,
    },
    CherryPick {
        commit_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CommitKind {
    #[default]
    Normal,
    Highlight,
    Reverse,
}

// ---- requirement -----------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RequirementDiagram {
    pub requirements: Vec<Requirement>,
    pub elements: Vec<ReqElement>,
    pub relations: Vec<ReqRelation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Requirement {
    pub kind: RequirementKind,
    pub name: String,
    pub id: Option<String>,
    pub text: Option<String>,
    pub risk: Option<String>,
    pub verifymethod: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RequirementKind {
    #[default]
    Requirement,
    Functional,
    Interface,
    Performance,
    Physical,
    DesignConstraint,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReqElement {
    pub name: String,
    pub type_: Option<String>,
    pub docref: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReqRelation {
    pub from: String,
    pub to: String,
    pub kind: ReqRelationKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReqRelationKind {
    Contains,
    Copies,
    Derives,
    Satisfies,
    Verifies,
    Refines,
    Traces,
}

// ---- C4 --------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct C4Diagram {
    pub kind: C4Kind,
    pub title: Option<String>,
    pub elements: Vec<C4Element>,
    pub relations: Vec<C4Relation>,
}

impl Default for C4Diagram {
    fn default() -> Self {
        Self {
            kind: C4Kind::Context,
            title: None,
            elements: Vec::new(),
            relations: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum C4Kind {
    Context,
    Container,
    Component,
    Dynamic,
    Deployment,
}

#[derive(Debug, Clone, PartialEq)]
pub struct C4Element {
    pub kind: C4ElementKind,
    pub alias: String,
    pub label: String,
    pub descr: Option<String>,
    pub technology: Option<String>,
    pub external: bool,
    pub boundary_alias: Option<String>,
    pub boundary_label: Option<String>,
    pub boundary_kind: Option<C4BoundaryKind>,
    pub members: Vec<C4Element>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum C4ElementKind {
    Person,
    System,
    SystemDb,
    SystemQueue,
    Container,
    ContainerDb,
    ContainerQueue,
    Component,
    ComponentDb,
    ComponentQueue,
    Node,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum C4BoundaryKind {
    System,
    Container,
    Enterprise,
    Generic,
    Deployment,
}

#[derive(Debug, Clone, PartialEq)]
pub struct C4Relation {
    pub from: String,
    pub to: String,
    pub label: String,
    pub technology: Option<String>,
    pub direction: C4RelDirection,
    pub bidirectional: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum C4RelDirection {
    #[default]
    Default,
    Up,
    Down,
    Left,
    Right,
}

// ---- block-beta ------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct BlockDiagram {
    pub columns: Option<usize>,
    pub items: Vec<BlockItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlockItem {
    Block(Block),
    Group(BlockGroup),
    Space(usize),
    Edge(BlockEdge),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub id: String,
    pub label: String,
    pub shape: BlockShape,
    /// Optional column-span like `a["wide"]:2`.
    pub span: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockShape {
    #[default]
    Rect,
    Round,
    Stadium,
    Cylinder,
    Circle,
    Rhombus,
    Hexagon,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockGroup {
    pub id: String,
    pub label: Option<String>,
    pub columns: Option<usize>,
    pub items: Vec<BlockItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockEdge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub arrow: bool,
}

// ---- architecture-beta -----------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ArchitectureDiagram {
    pub groups: Vec<ArchGroup>,
    pub services: Vec<ArchService>,
    pub junctions: Vec<ArchJunction>,
    pub edges: Vec<ArchEdge>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArchGroup {
    pub id: String,
    pub icon: Option<String>,
    pub label: Option<String>,
    pub parent: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArchService {
    pub id: String,
    pub icon: Option<String>,
    pub label: Option<String>,
    pub parent: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArchJunction {
    pub id: String,
    pub parent: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArchEdge {
    pub from: String,
    pub from_side: ArchSide,
    pub from_arrow: bool,
    pub to: String,
    pub to_side: ArchSide,
    pub to_arrow: bool,
    pub label: Option<String>,
    pub group: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchSide {
    Top,
    Bottom,
    Left,
    Right,
}

// ---- kanban ----------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct KanbanDiagram {
    pub columns: Vec<KanbanColumn>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KanbanColumn {
    pub id: String,
    pub label: String,
    pub tasks: Vec<KanbanTask>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KanbanTask {
    pub id: String,
    pub text: String,
    pub assigned: Option<String>,
    pub priority: Option<String>,
}

// ---- treemap ---------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TreemapDiagram {
    pub title: Option<String>,
    pub root: Vec<TreemapNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TreemapNode {
    pub label: String,
    pub value: Option<f64>,
    pub children: Vec<TreemapNode>,
}
