//! Visual themes. Built-in: `default`, `dark`, `forest`, `neutral`.
//!
//! Custom themes: construct a [`Theme`] directly (typically via struct-update
//! syntax from a built-in) and pass it to
//! [`render_with`][crate::render_with].

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub fg: &'static str,
    pub fg_muted: &'static str,
    pub bg: &'static str,
    pub actor_fill: &'static str,
    pub actor_stroke: &'static str,
    pub lifeline: &'static str,
    pub arrow_stroke: &'static str,
    /// Fill of a sequence `note` box.
    pub note_fill: &'static str,
    /// Border stroke of a sequence `note` box.
    pub note_stroke: &'static str,
    /// Fill of a sequence activation band.
    pub activation_fill: &'static str,
    /// Border stroke of a sequence activation band.
    pub activation_stroke: &'static str,
    /// Fill of an alt/loop/par block-frame label tab.
    pub frame_label_fill: &'static str,
    pub flow_node_fill: &'static str,
    pub flow_node_stroke: &'static str,
    pub flow_edge_stroke: &'static str,
    pub flow_label_bg: &'static str,
    /// Background fill of a flowchart/subgraph cluster frame.
    pub flow_cluster_fill: &'static str,
    /// Border stroke of a flowchart/subgraph cluster frame.
    pub flow_cluster_stroke: &'static str,
    pub pie_palette: &'static [&'static str],
    /// CSS `font-family` applied to the root `<svg>`; cascades to all text.
    pub font_family: &'static str,
    /// Base `font-size` (px) on the root `<svg>`; individual labels may
    /// override it with their own `font-size`.
    pub font_size: f64,
}

impl Theme {
    pub const fn default_theme() -> Self {
        Self {
            fg: "#333",
            fg_muted: "#666",
            bg: "#fff",
            actor_fill: "#ECECFF",
            actor_stroke: "#9370DB",
            lifeline: "#999",
            arrow_stroke: "#333",
            note_fill: "#FFF5AD",
            note_stroke: "#aaaa33",
            activation_fill: "#ECECFF",
            activation_stroke: "#9370DB",
            frame_label_fill: "#EEE",
            flow_node_fill: "#ECECFF",
            flow_node_stroke: "#9370DB",
            flow_edge_stroke: "#333",
            flow_label_bg: "#fff",
            flow_cluster_fill: "#ffffde",
            flow_cluster_stroke: "#aaaa33",
            pie_palette: &PALETTE_DEFAULT,
            font_family: "sans-serif",
            font_size: 14.0,
        }
    }

    pub const fn dark() -> Self {
        Self {
            fg: "#E0E0E0",
            fg_muted: "#A0A0A0",
            bg: "#1E1E1E",
            actor_fill: "#3B3B5B",
            actor_stroke: "#B58CE0",
            lifeline: "#666",
            arrow_stroke: "#E0E0E0",
            note_fill: "#3B3B22",
            note_stroke: "#AAAA55",
            activation_fill: "#3B3B5B",
            activation_stroke: "#B58CE0",
            frame_label_fill: "#2A2A3C",
            flow_node_fill: "#3B3B5B",
            flow_node_stroke: "#B58CE0",
            flow_edge_stroke: "#E0E0E0",
            flow_label_bg: "#1E1E1E",
            flow_cluster_fill: "#2A2A3C",
            flow_cluster_stroke: "#9A9ABF",
            pie_palette: &PALETTE_DARK,
            font_family: "sans-serif",
            font_size: 14.0,
        }
    }

    pub const fn forest() -> Self {
        Self {
            fg: "#1E3A1E",
            fg_muted: "#5A7A5A",
            bg: "#F0F8F0",
            actor_fill: "#CDE7CD",
            actor_stroke: "#4E8A4E",
            lifeline: "#7BAA7B",
            arrow_stroke: "#1E3A1E",
            note_fill: "#E8F0C8",
            note_stroke: "#4E8A4E",
            activation_fill: "#CDE7CD",
            activation_stroke: "#4E8A4E",
            frame_label_fill: "#E4F0E4",
            flow_node_fill: "#CDE7CD",
            flow_node_stroke: "#4E8A4E",
            flow_edge_stroke: "#1E3A1E",
            flow_label_bg: "#F0F8F0",
            flow_cluster_fill: "#E4F0E4",
            flow_cluster_stroke: "#4E8A4E",
            pie_palette: &PALETTE_FOREST,
            font_family: "sans-serif",
            font_size: 14.0,
        }
    }

    pub const fn neutral() -> Self {
        Self {
            fg: "#222",
            fg_muted: "#777",
            bg: "#fff",
            actor_fill: "#EEE",
            actor_stroke: "#777",
            lifeline: "#BBB",
            arrow_stroke: "#222",
            note_fill: "#F0F0D8",
            note_stroke: "#AAAAAA",
            activation_fill: "#EEE",
            activation_stroke: "#777",
            frame_label_fill: "#F4F4F4",
            flow_node_fill: "#EEE",
            flow_node_stroke: "#777",
            flow_edge_stroke: "#222",
            flow_label_bg: "#fff",
            flow_cluster_fill: "#F4F4F4",
            flow_cluster_stroke: "#AAAAAA",
            pie_palette: &PALETTE_NEUTRAL,
            font_family: "sans-serif",
            font_size: 14.0,
        }
    }

    pub fn by_name(name: &str) -> Option<Self> {
        match name {
            "default" | "base" => Some(Self::default_theme()),
            "dark" => Some(Self::dark()),
            "forest" => Some(Self::forest()),
            "neutral" => Some(Self::neutral()),
            _ => None,
        }
    }

    pub fn pie_color(&self, i: usize) -> &'static str {
        self.pie_palette[i % self.pie_palette.len()]
    }

    /// Override the root `font-family` (e.g. `"Inter, sans-serif"`).
    pub const fn with_font(mut self, family: &'static str) -> Self {
        self.font_family = family;
        self
    }

    /// Override the base `font-size` in pixels.
    pub const fn with_font_size(mut self, size: f64) -> Self {
        self.font_size = size;
        self
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::default_theme()
    }
}

const PALETTE_DEFAULT: [&str; 10] = [
    "#5470C6", "#91CC75", "#FAC858", "#EE6666", "#73C0DE", "#3BA272", "#FC8452", "#9A60B4",
    "#EA7CCC", "#7BCBA5",
];

const PALETTE_DARK: [&str; 10] = [
    "#7CB5FF", "#A6D88A", "#FFD980", "#FF8888", "#8FD8F2", "#5BC09A", "#FF9B6E", "#B58CE0",
    "#FF9CDA", "#8FE0BA",
];

const PALETTE_FOREST: [&str; 10] = [
    "#4E8A4E", "#7BAA5A", "#A8C870", "#D7E0A0", "#A8C8A8", "#3A6B3A", "#6BA66B", "#C0D8A0",
    "#7AA070", "#5C8C5C",
];

const PALETTE_NEUTRAL: [&str; 10] = [
    "#444", "#666", "#888", "#AAA", "#555", "#777", "#999", "#BBB", "#5E5E5E", "#7E7E7E",
];
