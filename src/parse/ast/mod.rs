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
///
/// [`config`](Self::config) is the generic, flattened view of the whole
/// `config:` tree (dotted keys, e.g. `flowchart.htmlLabels`); the typed fields
/// below are the subset the renderer currently honors, derived from it.
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
    /// `config.themeVariables.*` — upstream's `theme: base` recoloring path
    /// (`primaryColor`, `lineColor`, `fontFamily`, …), keyed by variable name.
    pub theme_variables: std::collections::BTreeMap<String, String>,
    /// `config.fontFamily` — CSS `font-family` for all text.
    pub font_family: Option<String>,
    /// `config.fontSize` — base font size in px (a `px` suffix is stripped).
    pub font_size: Option<f64>,
    /// `config.useMaxWidth` — `false` emits a fixed-size SVG instead of the
    /// responsive `width="100%"` envelope.
    pub use_max_width: Option<bool>,
    /// `config.look` (`classic`/`handDrawn`/…) — parsed, not yet honored.
    pub look: Option<String>,
    /// `config.layout` (`dagre`/`elk`/…) — parsed, not yet honored.
    pub layout: Option<String>,
    /// `config.securityLevel` — parsed, not yet honored.
    pub security_level: Option<String>,
    /// `config.kanban.ticketBaseUrl` from frontmatter — copied onto a
    /// [`KanbanDiagram`] to build per-card ticket links.
    pub ticket_base_url: Option<String>,
    /// `config.treemap.valueFormat` from frontmatter — copied onto a
    /// [`TreemapDiagram`] to format leaf values.
    pub value_format: Option<String>,
    /// `config.treemap.showValues` from frontmatter — copied onto a
    /// [`TreemapDiagram`]; `Some(false)` hides leaf value text.
    pub show_values: Option<bool>,
    /// `config.sankey.linkColor` from frontmatter — copied onto a
    /// [`SankeyDiagram`] to tint links.
    pub sankey_link_color: Option<String>,
    /// `config.sankey.nodeAlignment` from frontmatter — copied onto a
    /// [`SankeyDiagram`] to drive column alignment.
    pub sankey_node_alignment: Option<String>,
    /// `config.timeline.disableMulticolor` from frontmatter — copied onto a
    /// [`TimelineDiagram`] to keep a sectionless timeline one flat color.
    pub timeline_disable_multicolor: Option<bool>,
    /// `config.gitGraph.*` keys — copied onto a [`GitGraphDiagram`]'s config.
    pub git_graph: GitGraphMeta,
    /// The whole `config:` tree flattened to dotted `key → value` entries
    /// (both frontmatter and `%%{init}%%`). The typed fields above are derived
    /// from this; per-diagram renderers can read further keys as needed.
    pub config: std::collections::BTreeMap<String, String>,
}

/// `config.gitGraph.*` keys pulled from the preamble; each is `None` when the
/// source did not set it, leaving upstream's own default in place.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GitGraphMeta {
    pub main_branch_name: Option<String>,
    pub show_branches: Option<bool>,
    pub show_commit_label: Option<bool>,
    pub rotate_commit_label: Option<bool>,
    pub parallel_commits: Option<bool>,
    pub main_branch_order: Option<usize>,
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
