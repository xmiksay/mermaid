//! `themeVariables` overrides: the documented `theme: base` customization path
//! that recolors a [`Theme`] from upstream variable names (`primaryColor`,
//! `lineColor`, the `pie{N}`/`git{N}`/`cScale{N}` palette slots, `fontFamily`,
//! …). Split out of `theme/mod.rs` to keep each file under the size cap.

use std::collections::BTreeMap;

use super::{Str, Theme};

impl Theme {
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
    Str::Owned(v.to_string())
}

/// Parse a `fontSize` value that may carry a `px` suffix (`"16px"` / `"16"`).
fn parse_font_size_px(v: &str) -> Option<f64> {
    let v = v.trim();
    let num = v.strip_suffix("px").unwrap_or(v).trim();
    num.parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && *n > 0.0)
}

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
}
