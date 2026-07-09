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

mod palette;
mod variables;

use palette::{
    CSCALE_DEFAULT, GIT_DEFAULT, PALETTE_DARK, PALETTE_FOREST, PALETTE_NEUTRAL, PIE_DEFAULT,
    QUADRANT_FILLS_DARK, QUADRANT_FILLS_DEFAULT, QUADRANT_FILLS_FOREST, QUADRANT_FILLS_NEUTRAL,
    XYCHART_BASE, XYCHART_DARK, XYCHART_DEFAULT, XYCHART_FOREST, XYCHART_NEUTRAL,
};

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
    /// Generic categorical scale (upstream `cScale0..11`) read by the diagram
    /// kinds that cycle a color per series/section without a scale of their own:
    /// journey, timeline, sankey, radar, packet, kanban, quadrant and
    /// treemap. `cScale{N}` `themeVariables` recolor individual slots, so this
    /// is an owned-on-write [`Cow`] slice rather than a `&'static` one.
    pub cscale_palette: Cow<'static, [Str]>,
    /// Pie-slice scale (upstream `pie1..12`), distinct from
    /// [`cscale_palette`][Theme::cscale_palette]. `pie{N}` `themeVariables`
    /// recolor individual slots.
    pub pie_palette: Cow<'static, [Str]>,
    /// gitGraph lane scale (upstream `git0..7`), distinct from
    /// [`cscale_palette`][Theme::cscale_palette]. `git{N}` `themeVariables`
    /// recolor individual slots.
    pub git_palette: Cow<'static, [Str]>,
    /// xychart series scale (upstream `xyChart.plotColorPalette`), distinct from
    /// [`cscale_palette`][Theme::cscale_palette] — upstream hardcodes a pale
    /// lavender / gray-blue palette per theme rather than reusing `cScale`
    /// (#319). Overridden per-diagram by `themeVariables.xyChart.plotColorPalette`.
    pub xychart_palette: Cow<'static, [Str]>,
    /// Pie slice/outer-circle stroke (`pieStrokeColor`). `None` falls back to
    /// `black`.
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
    /// quadrant number minus one. `None` falls back to
    /// [`quadrant_default_fills`][Theme::quadrant_default_fills].
    pub quadrant_fills: [Option<Str>; 4],
    /// Per-theme default quadrant background tints (quadrant 1..4). Upstream
    /// fills all four quadrants with lightened tints of the single primary-color
    /// family rather than distinct categorical hues (#316).
    pub quadrant_default_fills: [Str; 4],
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
            lifeline: Cow::Borrowed("#9370DB"),
            arrow_stroke: Cow::Borrowed("#333"),
            signal_text_color: None,
            note_fill: Cow::Borrowed("#FFF5AD"),
            note_stroke: Cow::Borrowed("#aaaa33"),
            activation_fill: Cow::Borrowed("#ECECFF"),
            activation_stroke: Cow::Borrowed("#9370DB"),
            // Upstream's `labelBoxBkgColor` defaults to `actorBkg` — a lavender
            // frame-label tab, not the old gray `#EEE` (#329).
            frame_label_fill: Cow::Borrowed("#ECECFF"),
            title_color: None,
            flow_node_fill: Cow::Borrowed("#ECECFF"),
            flow_node_stroke: Cow::Borrowed("#9370DB"),
            flow_edge_stroke: Cow::Borrowed("#333"),
            flow_label_bg: Cow::Borrowed("#fff"),
            flow_cluster_fill: Cow::Borrowed("#ffffde"),
            flow_cluster_stroke: Cow::Borrowed("#aaaa33"),
            cscale_palette: Cow::Borrowed(&CSCALE_DEFAULT),
            pie_palette: Cow::Borrowed(&PIE_DEFAULT),
            git_palette: Cow::Borrowed(&GIT_DEFAULT),
            xychart_palette: Cow::Borrowed(&XYCHART_DEFAULT),
            pie_stroke: None,
            pie_opacity: None,
            commit_label_color: None,
            tag_label_color: None,
            quadrant_fills: [None, None, None, None],
            quadrant_default_fills: QUADRANT_FILLS_DEFAULT,
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
            frame_label_fill: Cow::Borrowed("#fff4dd"),
            flow_node_fill: Cow::Borrowed("#fff4dd"),
            flow_node_stroke: Cow::Borrowed("#cba15b"),
            xychart_palette: Cow::Borrowed(&XYCHART_BASE),
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
            frame_label_fill: Cow::Borrowed("#3B3B5B"),
            title_color: None,
            flow_node_fill: Cow::Borrowed("#3B3B5B"),
            flow_node_stroke: Cow::Borrowed("#B58CE0"),
            flow_edge_stroke: Cow::Borrowed("#E0E0E0"),
            flow_label_bg: Cow::Borrowed("#1E1E1E"),
            flow_cluster_fill: Cow::Borrowed("#2A2A3C"),
            flow_cluster_stroke: Cow::Borrowed("#9A9ABF"),
            cscale_palette: Cow::Borrowed(&PALETTE_DARK),
            pie_palette: Cow::Borrowed(&PALETTE_DARK),
            git_palette: Cow::Borrowed(&PALETTE_DARK),
            xychart_palette: Cow::Borrowed(&XYCHART_DARK),
            pie_stroke: None,
            pie_opacity: None,
            commit_label_color: None,
            tag_label_color: None,
            quadrant_fills: [None, None, None, None],
            quadrant_default_fills: QUADRANT_FILLS_DARK,
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
            frame_label_fill: Cow::Borrowed("#CDE7CD"),
            title_color: None,
            flow_node_fill: Cow::Borrowed("#CDE7CD"),
            flow_node_stroke: Cow::Borrowed("#4E8A4E"),
            flow_edge_stroke: Cow::Borrowed("#1E3A1E"),
            flow_label_bg: Cow::Borrowed("#F0F8F0"),
            flow_cluster_fill: Cow::Borrowed("#E4F0E4"),
            flow_cluster_stroke: Cow::Borrowed("#4E8A4E"),
            cscale_palette: Cow::Borrowed(&PALETTE_FOREST),
            pie_palette: Cow::Borrowed(&PALETTE_FOREST),
            git_palette: Cow::Borrowed(&PALETTE_FOREST),
            xychart_palette: Cow::Borrowed(&XYCHART_FOREST),
            pie_stroke: None,
            pie_opacity: None,
            commit_label_color: None,
            tag_label_color: None,
            quadrant_fills: [None, None, None, None],
            quadrant_default_fills: QUADRANT_FILLS_FOREST,
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
            frame_label_fill: Cow::Borrowed("#EEE"),
            title_color: None,
            flow_node_fill: Cow::Borrowed("#EEE"),
            flow_node_stroke: Cow::Borrowed("#777"),
            flow_edge_stroke: Cow::Borrowed("#222"),
            flow_label_bg: Cow::Borrowed("#fff"),
            flow_cluster_fill: Cow::Borrowed("#F4F4F4"),
            flow_cluster_stroke: Cow::Borrowed("#AAAAAA"),
            cscale_palette: Cow::Borrowed(&PALETTE_NEUTRAL),
            pie_palette: Cow::Borrowed(&PALETTE_NEUTRAL),
            git_palette: Cow::Borrowed(&PALETTE_NEUTRAL),
            xychart_palette: Cow::Borrowed(&XYCHART_NEUTRAL),
            pie_stroke: None,
            pie_opacity: None,
            commit_label_color: None,
            tag_label_color: None,
            quadrant_fills: [None, None, None, None],
            quadrant_default_fills: QUADRANT_FILLS_NEUTRAL,
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

    /// Pie-slice color at index `i` (upstream `pie{i+1}`), wrapping the scale.
    pub fn pie_color(&self, i: usize) -> &str {
        &self.pie_palette[i % self.pie_palette.len()]
    }

    /// Generic categorical color at index `i` (upstream `cScale{i}`), wrapping
    /// the scale. Read by every color-cycling diagram without a scale of its
    /// own.
    pub fn cscale_color(&self, i: usize) -> &str {
        &self.cscale_palette[i % self.cscale_palette.len()]
    }

    /// gitGraph lane color at index `i` (upstream `git{i}`), wrapping the scale.
    pub fn git_color(&self, i: usize) -> &str {
        &self.git_palette[i % self.git_palette.len()]
    }

    /// xychart series color at index `i` (upstream `xyChart.plotColorPalette`),
    /// wrapping the scale.
    pub fn xychart_color(&self, i: usize) -> &str {
        &self.xychart_palette[i % self.xychart_palette.len()]
    }

    /// Pie slice/outer-circle stroke (`pieStrokeColor`), defaulting to black —
    /// upstream outlines the circumference and slice edges in black.
    pub fn pie_stroke(&self) -> &str {
        self.pie_stroke.as_deref().unwrap_or("black")
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
    /// `themeVariables` override if set, else the per-theme default tint — all
    /// four are lightened tints of the single primary-color family (#316).
    pub fn quadrant_fill(&self, quadrant: usize) -> &str {
        match self
            .quadrant_fills
            .get(quadrant - 1)
            .and_then(Option::as_deref)
        {
            Some(c) => c,
            None => &self.quadrant_default_fills[quadrant - 1],
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

    /// Set `pie{i+1}` slot `i` to `c` (see [`set_slot`]).
    fn set_pie_palette(&mut self, i: usize, c: Str) {
        set_slot(&mut self.pie_palette, i, c);
    }

    /// Set `git{i}` slot `i` to `c` (see [`set_slot`]).
    fn set_git_palette(&mut self, i: usize, c: Str) {
        set_slot(&mut self.git_palette, i, c);
    }

    /// Set `cScale{i}` slot `i` to `c` (see [`set_slot`]).
    fn set_cscale_palette(&mut self, i: usize, c: Str) {
        set_slot(&mut self.cscale_palette, i, c);
    }
}

/// Set palette slot `i` to `c`, growing the palette (cloning it out of the
/// `'static` slice on first write) and back-filling any gap with the wrapped
/// built-in colors so untouched slots keep their defaults.
fn set_slot(pal: &mut Cow<'static, [Str]>, i: usize, c: Str) {
    let base: Vec<Str> = pal.to_vec();
    let pal = pal.to_mut();
    while pal.len() <= i {
        let fill = base[pal.len() % base.len()].clone();
        pal.push(fill);
    }
    pal[i] = c;
}

impl Default for Theme {
    fn default() -> Self {
        Self::default_theme()
    }
}

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
    fn default_palettes_are_authentic_mermaid_scales() {
        let t = Theme::default_theme();
        // Pale-lavender / pale-yellow primaries, not the old saturated palette.
        assert_eq!(t.cscale_color(0), "#B9B9FF");
        assert_eq!(t.pie_color(0), "#ECECFF");
        // git0..7 are the raw base colors darkened 25% (issue #309), so main
        // renders a saturated blue rather than pale lavender.
        assert_eq!(t.git_color(0), "#6D6DFF");
        // The three upstream scales genuinely differ past the shared primaries.
        assert_eq!(t.cscale_color(2), "#E8FFB9");
        assert_eq!(t.pie_color(2), "#B5FF20");
        assert_eq!(t.git_color(2), "#D1FF6D");
        // xychart has its own scale: pale-lavender bars, dark gray-blue line —
        // not the saturated cScale colors (#319).
        assert_eq!(t.xychart_color(0), "#ECECFF");
        assert_eq!(t.xychart_color(1), "#8493A6");
    }

    #[test]
    fn text_color_helpers_fall_back() {
        let t = Theme::default_theme();
        assert_eq!(t.actor_text(), t.fg);
        assert_eq!(t.signal_text(), t.fg);
        assert_eq!(t.title(), t.fg);
        assert_eq!(t.commit_label(), t.fg_muted);
        assert_eq!(t.tag_label(), t.fg);
        assert_eq!(t.pie_stroke(), "black");
    }
}
