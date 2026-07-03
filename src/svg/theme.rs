//! Visual themes. Built-in: `default`, `dark`, `forest`, `neutral`.
//!
//! Custom themes: construct a [`Theme`] directly (typically via struct-update
//! syntax from a built-in) and pass it to
//! [`render_with`][crate::render_with].
//!
//! Color/font fields are [`Cow<'static, str>`] so the built-ins stay
//! zero-allocation `const` values while source-level `themeVariables` /
//! `fontFamily` config can override them with owned runtime strings
//! ([`apply_theme_variables`][Theme::apply_theme_variables]).

use std::borrow::Cow;
use std::collections::BTreeMap;

/// A borrowed-or-owned color/font string. Built-ins use `Cow::Borrowed`;
/// `themeVariables` overrides use `Cow::Owned`.
type Str = Cow<'static, str>;

#[derive(Debug, Clone)]
pub struct Theme {
    pub fg: Str,
    pub fg_muted: Str,
    pub bg: Str,
    pub actor_fill: Str,
    pub actor_stroke: Str,
    pub lifeline: Str,
    pub arrow_stroke: Str,
    /// Fill of a sequence `note` box.
    pub note_fill: Str,
    /// Border stroke of a sequence `note` box.
    pub note_stroke: Str,
    /// Fill of a sequence activation band.
    pub activation_fill: Str,
    /// Border stroke of a sequence activation band.
    pub activation_stroke: Str,
    /// Fill of an alt/loop/par block-frame label tab.
    pub frame_label_fill: Str,
    pub flow_node_fill: Str,
    pub flow_node_stroke: Str,
    pub flow_edge_stroke: Str,
    pub flow_label_bg: Str,
    /// Background fill of a flowchart/subgraph cluster frame.
    pub flow_cluster_fill: Str,
    /// Border stroke of a flowchart/subgraph cluster frame.
    pub flow_cluster_stroke: Str,
    pub pie_palette: &'static [&'static str],
    /// Optional `quadrant{1..4}Fill` overrides (`themeVariables`), indexed by
    /// quadrant number minus one. `None` falls back to the pie palette.
    pub quadrant_fills: [Option<Str>; 4],
    /// CSS `font-family` applied to the root `<svg>`; cascades to all text.
    pub font_family: Str,
    /// Base `font-size` (px) on the root `<svg>`; individual labels may
    /// override it with their own `font-size`.
    pub font_size: f64,
    /// Emit the responsive `width="100%"` + `max-width` envelope (upstream
    /// default). `config.useMaxWidth: false` clears this so the SVG builder
    /// emits a fixed pixel `width`/`height` instead.
    pub responsive: bool,
}

impl Theme {
    pub const fn default_theme() -> Self {
        Self {
            fg: Cow::Borrowed("#333"),
            fg_muted: Cow::Borrowed("#666"),
            bg: Cow::Borrowed("#fff"),
            actor_fill: Cow::Borrowed("#ECECFF"),
            actor_stroke: Cow::Borrowed("#9370DB"),
            lifeline: Cow::Borrowed("#999"),
            arrow_stroke: Cow::Borrowed("#333"),
            note_fill: Cow::Borrowed("#FFF5AD"),
            note_stroke: Cow::Borrowed("#aaaa33"),
            activation_fill: Cow::Borrowed("#ECECFF"),
            activation_stroke: Cow::Borrowed("#9370DB"),
            frame_label_fill: Cow::Borrowed("#EEE"),
            flow_node_fill: Cow::Borrowed("#ECECFF"),
            flow_node_stroke: Cow::Borrowed("#9370DB"),
            flow_edge_stroke: Cow::Borrowed("#333"),
            flow_label_bg: Cow::Borrowed("#fff"),
            flow_cluster_fill: Cow::Borrowed("#ffffde"),
            flow_cluster_stroke: Cow::Borrowed("#aaaa33"),
            pie_palette: &PALETTE_DEFAULT,
            quadrant_fills: [None, None, None, None],
            font_family: Cow::Borrowed("sans-serif"),
            font_size: 14.0,
            responsive: true,
        }
    }

    pub const fn dark() -> Self {
        Self {
            fg: Cow::Borrowed("#E0E0E0"),
            fg_muted: Cow::Borrowed("#A0A0A0"),
            bg: Cow::Borrowed("#1E1E1E"),
            actor_fill: Cow::Borrowed("#3B3B5B"),
            actor_stroke: Cow::Borrowed("#B58CE0"),
            lifeline: Cow::Borrowed("#666"),
            arrow_stroke: Cow::Borrowed("#E0E0E0"),
            note_fill: Cow::Borrowed("#3B3B22"),
            note_stroke: Cow::Borrowed("#AAAA55"),
            activation_fill: Cow::Borrowed("#3B3B5B"),
            activation_stroke: Cow::Borrowed("#B58CE0"),
            frame_label_fill: Cow::Borrowed("#2A2A3C"),
            flow_node_fill: Cow::Borrowed("#3B3B5B"),
            flow_node_stroke: Cow::Borrowed("#B58CE0"),
            flow_edge_stroke: Cow::Borrowed("#E0E0E0"),
            flow_label_bg: Cow::Borrowed("#1E1E1E"),
            flow_cluster_fill: Cow::Borrowed("#2A2A3C"),
            flow_cluster_stroke: Cow::Borrowed("#9A9ABF"),
            pie_palette: &PALETTE_DARK,
            quadrant_fills: [None, None, None, None],
            font_family: Cow::Borrowed("sans-serif"),
            font_size: 14.0,
            responsive: true,
        }
    }

    pub const fn forest() -> Self {
        Self {
            fg: Cow::Borrowed("#1E3A1E"),
            fg_muted: Cow::Borrowed("#5A7A5A"),
            bg: Cow::Borrowed("#F0F8F0"),
            actor_fill: Cow::Borrowed("#CDE7CD"),
            actor_stroke: Cow::Borrowed("#4E8A4E"),
            lifeline: Cow::Borrowed("#7BAA7B"),
            arrow_stroke: Cow::Borrowed("#1E3A1E"),
            note_fill: Cow::Borrowed("#E8F0C8"),
            note_stroke: Cow::Borrowed("#4E8A4E"),
            activation_fill: Cow::Borrowed("#CDE7CD"),
            activation_stroke: Cow::Borrowed("#4E8A4E"),
            frame_label_fill: Cow::Borrowed("#E4F0E4"),
            flow_node_fill: Cow::Borrowed("#CDE7CD"),
            flow_node_stroke: Cow::Borrowed("#4E8A4E"),
            flow_edge_stroke: Cow::Borrowed("#1E3A1E"),
            flow_label_bg: Cow::Borrowed("#F0F8F0"),
            flow_cluster_fill: Cow::Borrowed("#E4F0E4"),
            flow_cluster_stroke: Cow::Borrowed("#4E8A4E"),
            pie_palette: &PALETTE_FOREST,
            quadrant_fills: [None, None, None, None],
            font_family: Cow::Borrowed("sans-serif"),
            font_size: 14.0,
            responsive: true,
        }
    }

    pub const fn neutral() -> Self {
        Self {
            fg: Cow::Borrowed("#222"),
            fg_muted: Cow::Borrowed("#777"),
            bg: Cow::Borrowed("#fff"),
            actor_fill: Cow::Borrowed("#EEE"),
            actor_stroke: Cow::Borrowed("#777"),
            lifeline: Cow::Borrowed("#BBB"),
            arrow_stroke: Cow::Borrowed("#222"),
            note_fill: Cow::Borrowed("#F0F0D8"),
            note_stroke: Cow::Borrowed("#AAAAAA"),
            activation_fill: Cow::Borrowed("#EEE"),
            activation_stroke: Cow::Borrowed("#777"),
            frame_label_fill: Cow::Borrowed("#F4F4F4"),
            flow_node_fill: Cow::Borrowed("#EEE"),
            flow_node_stroke: Cow::Borrowed("#777"),
            flow_edge_stroke: Cow::Borrowed("#222"),
            flow_label_bg: Cow::Borrowed("#fff"),
            flow_cluster_fill: Cow::Borrowed("#F4F4F4"),
            flow_cluster_stroke: Cow::Borrowed("#AAAAAA"),
            pie_palette: &PALETTE_NEUTRAL,
            quadrant_fills: [None, None, None, None],
            font_family: Cow::Borrowed("sans-serif"),
            font_size: 14.0,
            responsive: true,
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

    /// Fill for quadrant `quadrant` (1-based). Returns the `quadrant{N}Fill`
    /// `themeVariables` override if set, else the palette color at
    /// `palette_index` (the two differ because the quadrant-to-palette mapping
    /// is not 1:1).
    pub fn quadrant_fill(&self, quadrant: usize, palette_index: usize) -> &str {
        match self
            .quadrant_fills
            .get(quadrant - 1)
            .and_then(Option::as_deref)
        {
            Some(c) => c,
            None => self.pie_color(palette_index),
        }
    }

    /// Override the root `font-family` (e.g. `"Inter, sans-serif"`).
    pub fn with_font(mut self, family: impl Into<Str>) -> Self {
        self.font_family = family.into();
        self
    }

    /// Override the base `font-size` in pixels.
    pub const fn with_font_size(mut self, size: f64) -> Self {
        self.font_size = size;
        self
    }

    /// Recolor `self` from upstream `themeVariables` (the documented
    /// `theme: base` customization path). `vars` maps the upstream variable
    /// name (e.g. `primaryColor`, `lineColor`, `fontFamily`) to its value.
    /// Each recognized variable overrides the derived diagram-specific fields;
    /// unknown variables are ignored.
    pub fn apply_theme_variables(&mut self, vars: &BTreeMap<String, String>) {
        let get = |k: &str| vars.get(k).map(String::as_str);

        if let Some(v) = get("primaryColor").or_else(|| get("mainBkg")) {
            self.flow_node_fill = own(v);
            self.actor_fill = own(v);
            self.activation_fill = own(v);
        }
        if let Some(v) = get("primaryBorderColor").or_else(|| get("nodeBorder")) {
            self.flow_node_stroke = own(v);
            self.actor_stroke = own(v);
            self.activation_stroke = own(v);
        }
        if let Some(v) = get("primaryTextColor") {
            self.fg = own(v);
        }
        if let Some(v) = get("lineColor") {
            self.arrow_stroke = own(v);
            self.flow_edge_stroke = own(v);
            self.lifeline = own(v);
        }
        if let Some(v) = get("textColor") {
            self.fg = own(v);
        }
        if let Some(v) = get("secondaryColor") {
            self.flow_cluster_fill = own(v);
        }
        if let Some(v) = get("tertiaryColor") {
            self.frame_label_fill = own(v);
        }
        if let Some(v) = get("background").or_else(|| get("bg")) {
            self.bg = own(v);
        }
        if let Some(v) = get("noteBkgColor") {
            self.note_fill = own(v);
        }
        if let Some(v) = get("noteBorderColor") {
            self.note_stroke = own(v);
        }
        if let Some(v) = get("clusterBkg") {
            self.flow_cluster_fill = own(v);
        }
        if let Some(v) = get("clusterBorder") {
            self.flow_cluster_stroke = own(v);
        }
        for (i, key) in [
            "quadrant1Fill",
            "quadrant2Fill",
            "quadrant3Fill",
            "quadrant4Fill",
        ]
        .iter()
        .enumerate()
        {
            if let Some(v) = get(key) {
                self.quadrant_fills[i] = Some(own(v));
            }
        }
        if let Some(v) = get("fontFamily") {
            self.font_family = own(v);
        }
        if let Some(v) = get("fontSize").and_then(parse_font_size_px) {
            self.font_size = v;
        }
    }
}

/// Own a `themeVariables` value into the theme's `Cow` field.
fn own(v: &str) -> Str {
    Cow::Owned(v.to_string())
}

/// Parse a `fontSize` value that may carry a `px` suffix (`"16px"` / `"16"`).
fn parse_font_size_px(v: &str) -> Option<f64> {
    let v = v.trim();
    let num = v.strip_suffix("px").unwrap_or(v).trim();
    num.parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && *n > 0.0)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_variables_recolor_base() {
        let mut t = Theme::default_theme();
        let mut vars = BTreeMap::new();
        vars.insert("primaryColor".to_string(), "#ff0000".to_string());
        vars.insert("lineColor".to_string(), "#00ff00".to_string());
        vars.insert("fontFamily".to_string(), "Courier".to_string());
        vars.insert("fontSize".to_string(), "20px".to_string());
        t.apply_theme_variables(&vars);
        assert_eq!(t.flow_node_fill, "#ff0000");
        assert_eq!(t.actor_fill, "#ff0000");
        assert_eq!(t.flow_edge_stroke, "#00ff00");
        assert_eq!(t.arrow_stroke, "#00ff00");
        assert_eq!(t.font_family, "Courier");
        assert_eq!(t.font_size, 20.0);
    }

    #[test]
    fn quadrant_fill_variables_override_palette() {
        let mut t = Theme::default_theme();
        // Unset quadrants fall back to their palette index.
        assert_eq!(t.quadrant_fill(1, 1), t.pie_color(1));
        let mut vars = BTreeMap::new();
        vars.insert("quadrant1Fill".to_string(), "#ff0000".to_string());
        vars.insert("quadrant3Fill".to_string(), "#00ff00".to_string());
        t.apply_theme_variables(&vars);
        assert_eq!(t.quadrant_fill(1, 1), "#ff0000");
        assert_eq!(t.quadrant_fill(3, 2), "#00ff00");
        assert_eq!(t.quadrant_fill(2, 0), t.pie_color(0));
    }

    #[test]
    fn unknown_variables_ignored() {
        let mut t = Theme::default_theme();
        let mut vars = BTreeMap::new();
        vars.insert("nonsense".to_string(), "x".to_string());
        t.apply_theme_variables(&vars);
        assert_eq!(t.flow_node_fill, "#ECECFF");
    }

    #[test]
    fn font_size_rejects_garbage() {
        assert_eq!(parse_font_size_px("16px"), Some(16.0));
        assert_eq!(parse_font_size_px("16"), Some(16.0));
        assert_eq!(parse_font_size_px("-4"), None);
        assert_eq!(parse_font_size_px("big"), None);
    }
}
