//! C4-diagram AST types.

#[derive(Debug, Clone, PartialEq)]
pub struct C4Diagram {
    pub kind: C4Kind,
    pub title: Option<String>,
    pub elements: Vec<C4Element>,
    pub relations: Vec<C4Relation>,
    /// `UpdateElementStyle` color overrides, keyed by element alias.
    pub element_styles: Vec<C4ElementStyle>,
    /// `UpdateRelStyle` color/offset overrides, keyed by the (from, to) pair.
    pub rel_styles: Vec<C4RelStyle>,
    /// `UpdateBoundaryStyle` color overrides, keyed by boundary alias.
    pub boundary_styles: Vec<C4ElementStyle>,
    /// `UpdateLayoutConfig` row hints.
    pub layout: C4LayoutConfig,
    /// `SHOW_LEGEND()` was requested (legend rendering deferred).
    pub show_legend: bool,
}

impl Default for C4Diagram {
    fn default() -> Self {
        Self {
            kind: C4Kind::Context,
            title: None,
            elements: Vec::new(),
            relations: Vec::new(),
            element_styles: Vec::new(),
            rel_styles: Vec::new(),
            boundary_styles: Vec::new(),
            layout: C4LayoutConfig::default(),
            show_legend: false,
        }
    }
}

/// `UpdateElementStyle(alias, $bgColor="…", $fontColor="…", $borderColor="…")`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct C4ElementStyle {
    pub alias: String,
    pub bg_color: Option<String>,
    pub font_color: Option<String>,
    pub border_color: Option<String>,
}

/// `UpdateRelStyle(from, to, $textColor="…", $lineColor="…", $offsetX="…", $offsetY="…")`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct C4RelStyle {
    pub from: String,
    pub to: String,
    pub text_color: Option<String>,
    pub line_color: Option<String>,
    pub offset_x: Option<f64>,
    pub offset_y: Option<f64>,
}

/// `UpdateLayoutConfig($c4ShapeInRow="…", $c4BoundaryInRow="…")`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct C4LayoutConfig {
    pub shape_in_row: Option<usize>,
    pub boundary_in_row: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
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
    /// `$sprite=` keyword arg (icon name — rendering deferred).
    pub sprite: Option<String>,
    /// `$tags=` keyword arg.
    pub tags: Option<String>,
    /// `$link=` keyword arg (URL — rendering deferred).
    pub link: Option<String>,
    pub external: bool,
    pub boundary_alias: Option<String>,
    pub boundary_label: Option<String>,
    pub boundary_kind: Option<C4BoundaryKind>,
    pub members: Vec<C4Element>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
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
#[non_exhaustive]
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
    /// `$sprite=` keyword arg (icon name — rendering deferred).
    pub sprite: Option<String>,
    /// `$tags=` keyword arg.
    pub tags: Option<String>,
    /// `$link=` keyword arg (URL — rendering deferred).
    pub link: Option<String>,
    pub direction: C4RelDirection,
    pub bidirectional: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum C4RelDirection {
    #[default]
    Default,
    Up,
    Down,
    Left,
    Right,
}
