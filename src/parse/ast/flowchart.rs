//! Flowchart AST types.

use super::*;
use std::collections::HashMap;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlowchartDiagram {
    /// Diagram title from YAML frontmatter (`--- title: … ---`).
    pub title: Option<String>,
    pub direction: FlowDirection,
    pub nodes: Vec<FlowNode>,
    pub edges: Vec<FlowEdge>,
    pub subgraphs: Vec<Subgraph>,
    /// `classDef <name> …` definitions, keyed by class name.
    pub class_defs: HashMap<String, Style>,
    /// `linkStyle <idx> …` overrides, keyed by edge definition index.
    pub edge_styles: HashMap<usize, Style>,
    /// `linkStyle default …` applied to all edges.
    pub link_style_default: Style,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
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
    /// Class names applied via `class`/`:::` (resolution order preserved).
    pub classes: Vec<String>,
    /// Inline `style <id> …` properties (highest priority).
    pub style: Style,
    /// Interaction bound via a `click` directive, if any.
    pub click: Option<ClickAction>,
}

/// A `click <id> …` interaction. Either turns the node into a hyperlink or
/// binds a JavaScript callback fired on click.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ClickAction {
    /// `click A "url" "tooltip"` / `click A href "url" "tooltip" _blank` —
    /// wraps the node in an SVG `<a>` link.
    Href {
        url: String,
        tooltip: Option<String>,
        /// Link target such as `_blank`; `None` renders no `target` attribute.
        target: Option<String>,
    },
    /// `click A callback "tooltip"` / `click A call callback() "tooltip"` —
    /// binds an `onclick` handler invoking the named function.
    Callback {
        function: String,
        tooltip: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
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
    /// Start-side head for bidirectional edges (`<-->`, `o--o`, `x--x`);
    /// `EdgeHead::None` for the common one-directional edge.
    pub tail: EdgeHead,
    pub head: EdgeHead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EdgeLine {
    /// `-` solid
    Solid,
    /// `.` dotted
    Dotted,
    /// `=` thick
    Thick,
    /// `~~~` invisible — participates in layout but is not drawn
    Invisible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
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
    /// Class names applied to the cluster via `class <id> …`/`:::` — resolved
    /// against `FlowchartDiagram::class_defs` when styling the frame.
    pub classes: Vec<String>,
    /// Inline `style <id> …` properties applied to the cluster frame.
    pub style: Style,
}
