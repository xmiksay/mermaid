//! Visual themes. Built-in: `default`, `base`, `dark`, `forest`, `neutral`.
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
    /// Sequence actor/participant name text color (`actorTextColor`). `None`
    /// falls back to [`fg`][Theme::fg].
    pub actor_text_color: Option<Str>,
    pub lifeline: Str,
    pub arrow_stroke: Str,
    /// Sequence message text color (`signalTextColor`). `None` falls back to
    /// [`fg`][Theme::fg].
    pub signal_text_color: Option<Str>,
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
    /// Diagram title text color (`titleColor`). `None` falls back to
    /// [`fg`][Theme::fg].
    pub title_color: Option<Str>,
    pub flow_node_fill: Str,
    pub flow_node_stroke: Str,
    pub flow_edge_stroke: Str,
    pub flow_label_bg: Str,
    /// Background fill of a flowchart/subgraph cluster frame.
    pub flow_cluster_fill: Str,
    /// Border stroke of a flowchart/subgraph cluster frame.
    pub flow_cluster_stroke: Str,
    /// Categorical color palette shared by every diagram kind that cycles
    /// colors (pie slices, sankey/timeline/journey/radar/packet segments and
    /// gitGraph lanes). `themeVariables` overrides (`pie{N}`, `git{N}`,
    /// `cScale{N}`) recolor individual slots via
    /// [`apply_theme_variables`][Theme::apply_theme_variables], so it is an
    /// owned-on-write [`Cow`] slice rather than a `&'static` one.
    pub pie_palette: Cow<'static, [Str]>,
    /// Pie slice/legend stroke (`pieStrokeColor`). `None` falls back to `#fff`.
    pub pie_stroke: Option<Str>,
    /// Pie slice fill-opacity (`pieOpacity`). `None` emits no opacity attribute
    /// (fully opaque), keeping the default render byte-identical.
    pub pie_opacity: Option<Str>,
    /// gitGraph commit-id label color (`commitLabelColor`). `None` falls back
    /// to [`fg_muted`][Theme::fg_muted].
    pub commit_label_color: Option<Str>,
    /// gitGraph tag label color (`tagLabelColor`). `None` falls back to
    /// [`fg`][Theme::fg].
    pub tag_label_color: Option<Str>,
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
            actor_text_color: None,
            lifeline: Cow::Borrowed("#999"),
            arrow_stroke: Cow::Borrowed("#333"),
            signal_text_color: None,
            note_fill: Cow::Borrowed("#FFF5AD"),
            note_stroke: Cow::Borrowed("#aaaa33"),
            activation_fill: Cow::Borrowed("#ECECFF"),
            activation_stroke: Cow::Borrowed("#9370DB"),
            frame_label_fill: Cow::Borrowed("#EEE"),
            title_color: None,
            flow_node_fill: Cow::Borrowed("#ECECFF"),
            flow_node_stroke: Cow::Borrowed("#9370DB"),
            flow_edge_stroke: Cow::Borrowed("#333"),
            flow_label_bg: Cow::Borrowed("#fff"),
            flow_cluster_fill: Cow::Borrowed("#ffffde"),
            flow_cluster_stroke: Cow::Borrowed("#aaaa33"),
            pie_palette: Cow::Borrowed(&PALETTE_DEFAULT),
            pie_stroke: None,
            pie_opacity: None,
            commit_label_color: None,
            tag_label_color: None,
            quadrant_fills: [None, None, None, None],
            font_family: Cow::Borrowed("sans-serif"),
            font_size: 14.0,
            responsive: true,
        }
    }

    /// Upstream's `base` theme: the designated customization palette. Unlike
    /// `default`, its primary is the warm cream `#fff4dd`, so a `theme: base`
    /// render without `themeVariables` is visibly distinct from `default`.
    pub fn base() -> Self {
        Self {
            actor_fill: Cow::Borrowed("#fff4dd"),
            actor_stroke: Cow::Borrowed("#cba15b"),
            activation_fill: Cow::Borrowed("#fff4dd"),
            activation_stroke: Cow::Borrowed("#cba15b"),
            flow_node_fill: Cow::Borrowed("#fff4dd"),
            flow_node_stroke: Cow::Borrowed("#cba15b"),
            ..Self::default_theme()
        }
    }

    pub const fn dark() -> Self {
        Self {
            fg: Cow::Borrowed("#E0E0E0"),
            fg_muted: Cow::Borrowed("#A0A0A0"),
            bg: Cow::Borrowed("#1E1E1E"),
            actor_fill: Cow::Borrowed("#3B3B5B"),
            actor_stroke: Cow::Borrowed("#B58CE0"),
            actor_text_color: None,
            lifeline: Cow::Borrowed("#666"),
            arrow_stroke: Cow::Borrowed("#E0E0E0"),
            signal_text_color: None,
            note_fill: Cow::Borrowed("#3B3B22"),
            note_stroke: Cow::Borrowed("#AAAA55"),
            activation_fill: Cow::Borrowed("#3B3B5B"),
            activation_stroke: Cow::Borrowed("#B58CE0"),
            frame_label_fill: Cow::Borrowed("#2A2A3C"),
            title_color: None,
            flow_node_fill: Cow::Borrowed("#3B3B5B"),
            flow_node_stroke: Cow::Borrowed("#B58CE0"),
            flow_edge_stroke: Cow::Borrowed("#E0E0E0"),
            flow_label_bg: Cow::Borrowed("#1E1E1E"),
            flow_cluster_fill: Cow::Borrowed("#2A2A3C"),
            flow_cluster_stroke: Cow::Borrowed("#9A9ABF"),
            pie_palette: Cow::Borrowed(&PALETTE_DARK),
            pie_stroke: None,
            pie_opacity: None,
            commit_label_color: None,
            tag_label_color: None,
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
            actor_text_color: None,
            lifeline: Cow::Borrowed("#7BAA7B"),
            arrow_stroke: Cow::Borrowed("#1E3A1E"),
            signal_text_color: None,
            note_fill: Cow::Borrowed("#E8F0C8"),
            note_stroke: Cow::Borrowed("#4E8A4E"),
            activation_fill: Cow::Borrowed("#CDE7CD"),
            activation_stroke: Cow::Borrowed("#4E8A4E"),
            frame_label_fill: Cow::Borrowed("#E4F0E4"),
            title_color: None,
            flow_node_fill: Cow::Borrowed("#CDE7CD"),
            flow_node_stroke: Cow::Borrowed("#4E8A4E"),
            flow_edge_stroke: Cow::Borrowed("#1E3A1E"),
            flow_label_bg: Cow::Borrowed("#F0F8F0"),
            flow_cluster_fill: Cow::Borrowed("#E4F0E4"),
            flow_cluster_stroke: Cow::Borrowed("#4E8A4E"),
            pie_palette: Cow::Borrowed(&PALETTE_FOREST),
            pie_stroke: None,
            pie_opacity: None,
            commit_label_color: None,
            tag_label_color: None,
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
            actor_text_color: None,
            lifeline: Cow::Borrowed("#BBB"),
            arrow_stroke: Cow::Borrowed("#222"),
            signal_text_color: None,
            note_fill: Cow::Borrowed("#F0F0D8"),
            note_stroke: Cow::Borrowed("#AAAAAA"),
            activation_fill: Cow::Borrowed("#EEE"),
            activation_stroke: Cow::Borrowed("#777"),
            frame_label_fill: Cow::Borrowed("#F4F4F4"),
            title_color: None,
            flow_node_fill: Cow::Borrowed("#EEE"),
            flow_node_stroke: Cow::Borrowed("#777"),
            flow_edge_stroke: Cow::Borrowed("#222"),
            flow_label_bg: Cow::Borrowed("#fff"),
            flow_cluster_fill: Cow::Borrowed("#F4F4F4"),
            flow_cluster_stroke: Cow::Borrowed("#AAAAAA"),
            pie_palette: Cow::Borrowed(&PALETTE_NEUTRAL),
            pie_stroke: None,
            pie_opacity: None,
            commit_label_color: None,
            tag_label_color: None,
            quadrant_fills: [None, None, None, None],
            font_family: Cow::Borrowed("sans-serif"),
            font_size: 14.0,
            responsive: true,
        }
    }

    pub fn by_name(name: &str) -> Option<Self> {
        match name {
            "default" => Some(Self::default_theme()),
            "base" => Some(Self::base()),
            "dark" => Some(Self::dark()),
            "forest" => Some(Self::forest()),
            "neutral" => Some(Self::neutral()),
            _ => None,
        }
    }

    pub fn pie_color(&self, i: usize) -> &str {
        &self.pie_palette[i % self.pie_palette.len()]
    }

    /// Pie slice/legend stroke (`pieStrokeColor`), defaulting to white.
    pub fn pie_stroke(&self) -> &str {
        self.pie_stroke.as_deref().unwrap_or("#fff")
    }

    /// Sequence actor/participant name text color (`actorTextColor`).
    pub fn actor_text(&self) -> &str {
        self.actor_text_color.as_deref().unwrap_or(&self.fg)
    }

    /// Sequence message text color (`signalTextColor`).
    pub fn signal_text(&self) -> &str {
        self.signal_text_color.as_deref().unwrap_or(&self.fg)
    }

    /// Diagram title text color (`titleColor`).
    pub fn title(&self) -> &str {
        self.title_color.as_deref().unwrap_or(&self.fg)
    }

    /// gitGraph commit-id label color (`commitLabelColor`).
    pub fn commit_label(&self) -> &str {
        self.commit_label_color.as_deref().unwrap_or(&self.fg_muted)
    }

    /// gitGraph tag label color (`tagLabelColor`).
    pub fn tag_label(&self) -> &str {
        self.tag_label_color.as_deref().unwrap_or(&self.fg)
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

    /// Set categorical-palette slot `i` to `c`, growing the palette (cloning it
    /// out of the `'static` slice on first write) and back-filling any gap with
    /// the wrapped built-in colors so untouched slots keep their defaults.
    fn set_palette(&mut self, i: usize, c: Str) {
        let base: Vec<Str> = self.pie_palette.to_vec();
        let pal = self.pie_palette.to_mut();
        while pal.len() <= i {
            let fill = base[pal.len() % base.len()].clone();
            pal.push(fill);
        }
        pal[i] = c;
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
        // Titles (`titleColor` is the general title color).
        if let Some(v) = get("titleColor") {
            self.title_color = Some(own(v));
        }
        // Edge (link) label background (flowchart).
        if let Some(v) = get("edgeLabelBackground") {
            self.flow_label_bg = own(v);
        }

        self.apply_sequence_variables(vars);
        self.apply_pie_variables(vars);
        self.apply_git_variables(vars);
        self.apply_palette_variables(vars);

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

    /// Sequence-diagram `themeVariables` (upstream's `actor*`/`signal*`/
    /// activation/label-box names) onto the shared sequence fields.
    fn apply_sequence_variables(&mut self, vars: &BTreeMap<String, String>) {
        let get = |k: &str| vars.get(k).map(String::as_str);
        if let Some(v) = get("actorBkg") {
            self.actor_fill = own(v);
        }
        if let Some(v) = get("actorBorder") {
            self.actor_stroke = own(v);
        }
        if let Some(v) = get("actorTextColor") {
            self.actor_text_color = Some(own(v));
        }
        if let Some(v) = get("actorLineColor") {
            self.lifeline = own(v);
        }
        if let Some(v) = get("signalColor") {
            self.arrow_stroke = own(v);
        }
        if let Some(v) = get("signalTextColor") {
            self.signal_text_color = Some(own(v));
        }
        if let Some(v) = get("labelBoxBkgColor") {
            self.frame_label_fill = own(v);
        }
        if let Some(v) = get("activationBkgColor") {
            self.activation_fill = own(v);
        }
        if let Some(v) = get("activationBorderColor") {
            self.activation_stroke = own(v);
        }
    }

    /// Pie-chart `themeVariables`: title text, slice stroke/opacity, and the
    /// `pie{1..12}` per-slice palette overrides.
    fn apply_pie_variables(&mut self, vars: &BTreeMap<String, String>) {
        let get = |k: &str| vars.get(k).map(String::as_str);
        if let Some(v) = get("pieTitleTextColor") {
            self.title_color = Some(own(v));
        }
        if let Some(v) = get("pieStrokeColor") {
            self.pie_stroke = Some(own(v));
        }
        if let Some(v) = get("pieOpacity") {
            self.pie_opacity = Some(own(v));
        }
        // `pie{N}` is 1-based upstream; slot N-1 in the shared palette.
        for n in 1..=12 {
            if let Some(v) = get(&format!("pie{n}")) {
                self.set_palette(n - 1, own(v));
            }
        }
    }

    /// gitGraph `themeVariables`: the `git{0..7}` lane palette plus the commit
    /// and tag label colors.
    fn apply_git_variables(&mut self, vars: &BTreeMap<String, String>) {
        let get = |k: &str| vars.get(k).map(String::as_str);
        for n in 0..8 {
            if let Some(v) = get(&format!("git{n}")) {
                self.set_palette(n, own(v));
            }
        }
        if let Some(v) = get("commitLabelColor") {
            self.commit_label_color = Some(own(v));
        }
        if let Some(v) = get("tagLabelColor") {
            self.tag_label_color = Some(own(v));
        }
    }

    /// The generic `cScale{0..11}` categorical scale, which this crate models
    /// as the same shared palette every color-cycling diagram reads.
    fn apply_palette_variables(&mut self, vars: &BTreeMap<String, String>) {
        let get = |k: &str| vars.get(k).map(String::as_str);
        for n in 0..12 {
            if let Some(v) = get(&format!("cScale{n}")) {
                self.set_palette(n, own(v));
            }
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

const PALETTE_DEFAULT: [Str; 10] = [
    Cow::Borrowed("#5470C6"),
    Cow::Borrowed("#91CC75"),
    Cow::Borrowed("#FAC858"),
    Cow::Borrowed("#EE6666"),
    Cow::Borrowed("#73C0DE"),
    Cow::Borrowed("#3BA272"),
    Cow::Borrowed("#FC8452"),
    Cow::Borrowed("#9A60B4"),
    Cow::Borrowed("#EA7CCC"),
    Cow::Borrowed("#7BCBA5"),
];

const PALETTE_DARK: [Str; 10] = [
    Cow::Borrowed("#7CB5FF"),
    Cow::Borrowed("#A6D88A"),
    Cow::Borrowed("#FFD980"),
    Cow::Borrowed("#FF8888"),
    Cow::Borrowed("#8FD8F2"),
    Cow::Borrowed("#5BC09A"),
    Cow::Borrowed("#FF9B6E"),
    Cow::Borrowed("#B58CE0"),
    Cow::Borrowed("#FF9CDA"),
    Cow::Borrowed("#8FE0BA"),
];

const PALETTE_FOREST: [Str; 10] = [
    Cow::Borrowed("#4E8A4E"),
    Cow::Borrowed("#7BAA5A"),
    Cow::Borrowed("#A8C870"),
    Cow::Borrowed("#D7E0A0"),
    Cow::Borrowed("#A8C8A8"),
    Cow::Borrowed("#3A6B3A"),
    Cow::Borrowed("#6BA66B"),
    Cow::Borrowed("#C0D8A0"),
    Cow::Borrowed("#7AA070"),
    Cow::Borrowed("#5C8C5C"),
];

const PALETTE_NEUTRAL: [Str; 10] = [
    Cow::Borrowed("#444"),
    Cow::Borrowed("#666"),
    Cow::Borrowed("#888"),
    Cow::Borrowed("#AAA"),
    Cow::Borrowed("#555"),
    Cow::Borrowed("#777"),
    Cow::Borrowed("#999"),
    Cow::Borrowed("#BBB"),
    Cow::Borrowed("#5E5E5E"),
    Cow::Borrowed("#7E7E7E"),
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

    #[test]
    fn base_theme_differs_from_default() {
        // `base` is the customization palette: a warm cream primary, visibly
        // distinct from default's lavender without any overrides.
        assert_eq!(Theme::default_theme().flow_node_fill, "#ECECFF");
        assert_eq!(Theme::base().flow_node_fill, "#fff4dd");
        assert_eq!(Theme::by_name("base").unwrap().actor_fill, "#fff4dd");
    }

    #[test]
    fn pie_variables_override_palette_slots() {
        let mut t = Theme::default_theme();
        let mut vars = BTreeMap::new();
        vars.insert("pie1".to_string(), "#111111".to_string());
        vars.insert("pie3".to_string(), "#333333".to_string());
        t.apply_theme_variables(&vars);
        assert_eq!(t.pie_color(0), "#111111");
        assert_eq!(t.pie_color(2), "#333333");
        // Untouched slots keep their defaults.
        assert_eq!(t.pie_color(1), "#91CC75");
    }

    #[test]
    fn pie_variable_grows_palette_beyond_default_len() {
        let mut t = Theme::default_theme();
        let mut vars = BTreeMap::new();
        vars.insert("pie12".to_string(), "#abcdef".to_string());
        t.apply_theme_variables(&vars);
        // Slot 11 is past the built-in 10-color palette; it must exist now.
        assert_eq!(t.pie_color(11), "#abcdef");
        // The back-filled slot 10 wraps the built-in palette (index 0).
        assert_eq!(t.pie_color(10), "#5470C6");
    }

    #[test]
    fn git_variables_override_lane_palette() {
        let mut t = Theme::default_theme();
        let mut vars = BTreeMap::new();
        vars.insert("git0".to_string(), "#0a0a0a".to_string());
        vars.insert("commitLabelColor".to_string(), "#c0ffee".to_string());
        vars.insert("tagLabelColor".to_string(), "#facade".to_string());
        t.apply_theme_variables(&vars);
        assert_eq!(t.pie_color(0), "#0a0a0a");
        assert_eq!(t.commit_label(), "#c0ffee");
        assert_eq!(t.tag_label(), "#facade");
    }

    #[test]
    fn sequence_and_pie_text_variables() {
        let mut t = Theme::default_theme();
        let mut vars = BTreeMap::new();
        vars.insert("actorTextColor".to_string(), "#a11".to_string());
        vars.insert("signalTextColor".to_string(), "#5163".to_string());
        vars.insert("actorBkg".to_string(), "#eee".to_string());
        vars.insert("pieStrokeColor".to_string(), "#000".to_string());
        vars.insert("pieOpacity".to_string(), "0.7".to_string());
        t.apply_theme_variables(&vars);
        assert_eq!(t.actor_text(), "#a11");
        assert_eq!(t.signal_text(), "#5163");
        assert_eq!(t.actor_fill, "#eee");
        assert_eq!(t.pie_stroke(), "#000");
        assert_eq!(t.pie_opacity.as_deref(), Some("0.7"));
    }

    #[test]
    fn text_color_helpers_fall_back() {
        let t = Theme::default_theme();
        assert_eq!(t.actor_text(), t.fg);
        assert_eq!(t.signal_text(), t.fg);
        assert_eq!(t.title(), t.fg);
        assert_eq!(t.commit_label(), t.fg_muted);
        assert_eq!(t.tag_label(), t.fg);
        assert_eq!(t.pie_stroke(), "#fff");
    }
}
