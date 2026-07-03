//! AST types for the data-oriented chart diagrams: journey, timeline, sankey,
//! quadrant, xychart, radar, and packet.

use std::collections::HashMap;

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
    /// `timeline <dir>` header direction (v11.14+, e.g. `LR`/`TD`). Parsed and
    /// validated; the horizontal renderer treats it as a no-op.
    pub direction: Option<String>,
    /// `config.timeline.disableMulticolor` — when `true`, a sectionless
    /// timeline stays one flat color instead of advancing per time-period.
    pub disable_multicolor: bool,
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
    /// `config.sankey.linkColor` — how each link's stroke color is derived:
    /// `source`/`target` (the node's palette color), `gradient` (source→target
    /// gradient), or a literal hex. `None` defaults to `source`.
    pub link_color: Option<String>,
    /// `config.sankey.nodeAlignment` — `justify`/`center`/`left`/`right`.
    /// `None` defaults to `justify`.
    pub node_alignment: Option<String>,
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
    /// `classDef <name> …` style definitions, referenced by `:::name`.
    pub classes: HashMap<String, QuadrantStyle>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QuadrantPoint {
    pub label: String,
    pub x: f64,
    pub y: f64,
    /// Third array value `[x, y, r]` or inline `radius:` — the bubble radius.
    pub radius: Option<f64>,
    pub color: Option<String>,
    pub stroke_color: Option<String>,
    pub stroke_width: Option<String>,
    /// `:::name` reference into `QuadrantDiagram::classes`.
    pub class_name: Option<String>,
}

/// Per-point styling shared by inline attributes and `classDef` definitions.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct QuadrantStyle {
    pub radius: Option<f64>,
    pub color: Option<String>,
    pub stroke_color: Option<String>,
    pub stroke_width: Option<String>,
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
#[non_exhaustive]
pub enum XyAxisKind {
    /// Categorical labels (e.g. month names).
    Categories(Vec<String>),
    /// Numeric range `min --> max`.
    Range { min: f64, max: f64 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct XySeries {
    pub kind: XySeriesKind,
    /// Optional quoted series title (`bar "Revenue" [..]`), shown in a legend.
    pub title: Option<String>,
    pub values: Vec<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
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
    /// Optional explicit min value; defaults to 0.
    pub min: Option<f64>,
    /// Optional explicit max value; defaults to max observed.
    pub max: Option<f64>,
    /// Number of graticule rings; defaults to 5.
    pub ticks: Option<u32>,
    /// Graticule shape (concentric circles vs polygon rings).
    pub graticule: RadarGraticule,
    /// Whether to draw the curve legend; `None` defaults to true.
    pub show_legend: Option<bool>,
}

/// Shape of the radar graticule (background grid rings).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum RadarGraticule {
    /// Concentric circles (upstream default).
    #[default]
    Circle,
    /// Polygon rings following the axis vertices.
    Polygon,
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
    /// `config.packet.*` rendering knobs (frontmatter / `%%{init}%%`).
    pub config: PacketConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PacketField {
    pub start: u32,
    pub end: u32,
    pub label: String,
}

/// `config.packet.*` layout knobs. Defaults match the renderer's built-in
/// constants, so a diagram with no config renders byte-identically.
#[derive(Debug, Clone, PartialEq)]
pub struct PacketConfig {
    /// `packet.bitsPerRow` — bits drawn per row before wrapping.
    pub bits_per_row: u32,
    /// `packet.bitWidth` — pixel width of one bit cell.
    pub bit_width: f64,
    /// `packet.rowHeight` — pixel height of one row.
    pub row_height: f64,
    /// `packet.showBits` — draw the per-bit ruler above the block.
    pub show_bits: bool,
    /// `packet.paddingX` — horizontal margin around the block.
    pub padding_x: f64,
    /// `packet.paddingY` — vertical margin around the block.
    pub padding_y: f64,
}

impl Default for PacketConfig {
    fn default() -> Self {
        Self {
            bits_per_row: 32,
            bit_width: 16.0,
            row_height: 40.0,
            show_bits: true,
            padding_x: 30.0,
            padding_y: 30.0,
        }
    }
}
