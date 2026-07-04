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

mod variables;

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
    fn base_theme_differs_from_default() {
        // `base` is the customization palette: a warm cream primary, visibly
        // distinct from default's lavender without any overrides.
        assert_eq!(Theme::default_theme().flow_node_fill, "#ECECFF");
        assert_eq!(Theme::base().flow_node_fill, "#fff4dd");
        assert_eq!(Theme::by_name("base").unwrap().actor_fill, "#fff4dd");
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
