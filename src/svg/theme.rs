//! Visual constants. A single default theme for v0.1.

pub const FG: &str = "#333";
pub const FG_MUTED: &str = "#666";
pub const ACTOR_FILL: &str = "#ECECFF";
pub const ACTOR_STROKE: &str = "#9370DB";
pub const LIFELINE: &str = "#999";
pub const ARROW_STROKE: &str = "#333";

pub const FLOW_NODE_FILL: &str = "#ECECFF";
pub const FLOW_NODE_STROKE: &str = "#9370DB";
pub const FLOW_EDGE_STROKE: &str = "#333";
pub const FLOW_LABEL_BG: &str = "#fff";

pub const PIE_PALETTE: &[&str] = &[
    "#5470C6", "#91CC75", "#FAC858", "#EE6666", "#73C0DE",
    "#3BA272", "#FC8452", "#9A60B4", "#EA7CCC", "#7BCBA5",
];

pub fn pie_color(i: usize) -> &'static str {
    PIE_PALETTE[i % PIE_PALETTE.len()]
}
