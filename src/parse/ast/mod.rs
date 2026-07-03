//! AST types shared by all diagram kinds.
//!
//! # API stability
//!
//! These types are the *output* of [`parse`](crate::parse): downstream code is
//! expected to read them (field access, pattern matching against the
//! `#[non_exhaustive]` enums) rather than construct them. Constructing an AST
//! struct with a literal is **not** part of the stable API — new `pub` fields
//! may be added in a minor release, which would break a downstream struct
//! literal but not field access. Build diagrams by parsing Mermaid source.
//!
//! The concrete per-diagram types live in domain submodules and are re-exported
//! flat here, so every type stays reachable as `ast::<Type>`.

mod block;
mod c4;
mod charts;
mod class;
mod er;
mod flowchart;
mod gantt;
mod sequence;
mod state;
mod structure;

pub use block::*;
pub use c4::*;
pub use charts::*;
pub use class::*;
pub use er::*;
pub use flowchart::*;
pub use gantt::*;
pub use sequence::*;
pub use state::*;
pub use structure::*;

/// CSS-ish `key:value` pairs from a `style`/`classDef`/`linkStyle` directive,
/// kept in source order. Resolved to SVG attributes at render time.
pub type Style = Vec<(String, String)>;

/// Shared, diagram-agnostic metadata extracted from the source *preamble*
/// (YAML frontmatter, `%%{init}%%` directives, and `accTitle`/`accDescr`
/// statements) before dispatching to a per-diagram parser.
///
/// `title` is also copied onto the concrete diagram's own `title` field when it
/// has one; the accessibility fields and `theme` only live here.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DiagramMeta {
    /// `title:` from YAML frontmatter (diagram title).
    pub title: Option<String>,
    /// `accTitle:` — an accessible short title emitted as `<title>`.
    pub acc_title: Option<String>,
    /// `accDescr:` (single line or `accDescr { … }` block) — emitted as `<desc>`.
    pub acc_descr: Option<String>,
    /// Theme name from `%%{init: {theme: …}}%%` or frontmatter `config.theme`.
    pub theme: Option<String>,
    /// `config.kanban.ticketBaseUrl` from frontmatter — copied onto a
    /// [`KanbanDiagram`] to build per-card ticket links.
    pub ticket_base_url: Option<String>,
}

#[derive(Debug, Clone)]
#[non_exhaustive]
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
