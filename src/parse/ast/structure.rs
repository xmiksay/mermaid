//! AST types for the structural diagrams: mindmap, gitGraph, requirement,
//! architecture-beta, kanban, and treemap.

use super::*;
use std::collections::HashMap;

// ---- mindmap ---------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MindmapDiagram {
    pub root: Option<MindmapNode>,
    /// `classDef <name> <props>` style classes, referenced by node `:::` classes.
    pub class_defs: HashMap<String, Style>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MindmapNode {
    pub text: String,
    pub shape: MindmapShape,
    pub icon: Option<String>,
    /// CSS classes attached via a `:::class1 class2` line.
    pub classes: Vec<String>,
    pub children: Vec<MindmapNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
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
    /// `gitGraph` config keys from `%%{init}%%` / frontmatter `config`.
    pub config: GitGraphConfig,
}

/// gitGraph configuration (upstream `config.gitGraph.*`), filled from the
/// source preamble. Non-`Default` fields keep upstream's own defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct GitGraphConfig {
    /// `mainBranchName` — the initial/default branch (upstream default `main`).
    pub main_branch_name: String,
    /// `showBranches` — draw branch labels and lane lines.
    pub show_branches: bool,
    /// `showCommitLabel` — draw the per-commit id label.
    pub show_commit_label: bool,
    /// `rotateCommitLabel` — rotate the commit id label (horizontal layout).
    pub rotate_commit_label: bool,
    /// `parallelCommits` — position commits by parent depth so independent
    /// branches can share a column instead of always advancing time.
    pub parallel_commits: bool,
}

impl Default for GitGraphConfig {
    fn default() -> Self {
        Self {
            main_branch_name: "main".into(),
            show_branches: true,
            show_commit_label: true,
            rotate_commit_label: true,
            parallel_commits: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum GitDirection {
    #[default]
    LeftRight,
    TopDown,
    BottomTop,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum GitEvent {
    Commit {
        id: Option<String>,
        tag: Option<String>,
        kind: CommitKind,
    },
    Branch {
        name: String,
        /// Explicit lane ordering from `branch <name> order: <n>`.
        order: Option<usize>,
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
        /// `parent:"…"` — required by upstream when cherry-picking a merge
        /// commit to pick which parent lineage.
        parent: Option<String>,
        /// `tag:"…"` — label drawn on the cherry-pick node.
        tag: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum CommitKind {
    #[default]
    Normal,
    Highlight,
    Reverse,
    /// A merge commit — drawn as a double concentric circle.
    Merge,
    /// A cherry-pick commit — drawn with the dedicated cherry glyph.
    CherryPick,
}

// ---- requirement -----------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RequirementDiagram {
    pub requirements: Vec<Requirement>,
    pub elements: Vec<ReqElement>,
    pub relations: Vec<ReqRelation>,
    /// v11 `direction TB/BT/LR/RL`; drives the layout transpose.
    pub direction: FlowDirection,
    /// `classDef <name> …` style definitions, referenced by `class`.
    pub class_defs: HashMap<String, Style>,
    /// `class <a>,<b> <name>` assignments, keyed by requirement/element name.
    pub node_classes: HashMap<String, Vec<String>>,
    /// `style <id> …` inline styles, keyed by requirement/element name.
    pub node_styles: HashMap<String, Style>,
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
#[non_exhaustive]
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
#[non_exhaustive]
pub enum ReqRelationKind {
    Contains,
    Copies,
    Derives,
    Satisfies,
    Verifies,
    Refines,
    Traces,
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
    pub group: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
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
    /// `kanban.ticketBaseUrl` from frontmatter config. `#TICKET#` in it is
    /// replaced by each task's `ticket` id to build the card link.
    pub ticket_base_url: Option<String>,
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
    pub ticket: Option<String>,
}

// ---- treemap ---------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TreemapDiagram {
    pub title: Option<String>,
    pub root: Vec<TreemapNode>,
    /// `classDef <name> <props>` definitions, referenced by a node's `:::name`.
    pub class_defs: HashMap<String, Style>,
    /// `config.treemap.valueFormat` (d3-format subset) applied to leaf values.
    pub value_format: Option<String>,
    /// `config.treemap.showValues` — `Some(false)` suppresses leaf value text
    /// (upstream gates on `showValues !== false`, so `None`/`Some(true)` show).
    pub show_values: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TreemapNode {
    pub label: String,
    pub value: Option<f64>,
    pub children: Vec<TreemapNode>,
    /// `:::name` class reference into [`TreemapDiagram::class_defs`].
    pub class_name: Option<String>,
}
