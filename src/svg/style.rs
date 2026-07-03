//! Resolve `style`/`classDef`/`linkStyle` declarations into SVG attributes.
//!
//! A node's effective style is layered: `default` classDef → each named class in
//! order → the node's inline `style` (later layers win per property). Only the
//! handful of CSS properties that map onto our SVG primitives are translated;
//! anything else is ignored.

use std::collections::HashMap;

use crate::parse::ast::Style;

pub(crate) struct ResolvedStyle {
    pub fill: Option<String>,
    pub stroke: Option<String>,
    /// `stroke-width`, with any trailing `px` stripped.
    pub stroke_width: Option<String>,
    pub stroke_dasharray: Option<String>,
    /// `color` — used for label text fill.
    pub color: Option<String>,
    /// `font-size`, with any trailing `px` stripped.
    pub font_size: Option<String>,
    /// `font-weight` (e.g. `bold`, `600`).
    pub font_weight: Option<String>,
    /// `font-style` (e.g. `italic`).
    pub font_style: Option<String>,
    /// `opacity` — applied to the shape.
    pub opacity: Option<String>,
}

impl ResolvedStyle {
    /// Shape presentation attributes, falling back to the supplied theme
    /// defaults where the style is silent.
    pub(crate) fn shape_attrs(&self, def_fill: &str, def_stroke: &str, def_sw: &str) -> String {
        let fill = self.fill.as_deref().unwrap_or(def_fill);
        let stroke = self.stroke.as_deref().unwrap_or(def_stroke);
        let sw = self.stroke_width.as_deref().unwrap_or(def_sw);
        let mut s = format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"{sw}\"");
        if let Some(d) = &self.stroke_dasharray {
            s.push_str(&format!(" stroke-dasharray=\"{d}\""));
        }
        if let Some(o) = &self.opacity {
            s.push_str(&format!(" opacity=\"{o}\""));
        }
        s
    }

    /// Label text colour, falling back to the theme foreground.
    pub(crate) fn label_fill<'a>(&'a self, def_fg: &'a str) -> &'a str {
        self.color.as_deref().unwrap_or(def_fg)
    }

    /// Extra text-presentation attributes (`font-weight`/`font-style`)
    /// contributed by the style, as a space-prefixed attribute string (empty
    /// when the style is silent). Appended after a renderer's own `<text>` attrs.
    pub(crate) fn text_attrs(&self) -> String {
        let mut s = String::new();
        if let Some(w) = &self.font_weight {
            s.push_str(&format!(" font-weight=\"{w}\""));
        }
        if let Some(st) = &self.font_style {
            s.push_str(&format!(" font-style=\"{st}\""));
        }
        s
    }

    /// Stroke colour for inner separators, falling back to the theme stroke.
    pub(crate) fn stroke_or<'a>(&'a self, def: &'a str) -> &'a str {
        self.stroke.as_deref().unwrap_or(def)
    }
}

/// Resolve a node/state/class style: `default` classDef → named classes → inline.
pub(crate) fn resolve_style(
    class_defs: &HashMap<String, Style>,
    classes: &[String],
    inline: &Style,
) -> ResolvedStyle {
    let mut acc: Style = Vec::new();
    if let Some(d) = class_defs.get("default") {
        merge(&mut acc, d);
    }
    for c in classes {
        if let Some(cd) = class_defs.get(c) {
            merge(&mut acc, cd);
        }
    }
    merge(&mut acc, inline);
    pick(&acc)
}

/// Resolve an edge style: `linkStyle default` → the per-index `linkStyle`.
pub(crate) fn resolve_edge_style(default: &Style, indexed: Option<&Style>) -> ResolvedStyle {
    let mut acc: Style = Vec::new();
    merge(&mut acc, default);
    if let Some(s) = indexed {
        merge(&mut acc, s);
    }
    pick(&acc)
}

fn merge(acc: &mut Style, add: &Style) {
    for (k, v) in add {
        if let Some(slot) = acc.iter_mut().find(|(ek, _)| ek == k) {
            slot.1 = v.clone();
        } else {
            acc.push((k.clone(), v.clone()));
        }
    }
}

fn pick(acc: &Style) -> ResolvedStyle {
    let get = |key: &str| {
        acc.iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    };
    let strip_px = |o: Option<String>| o.map(|v| v.trim_end_matches("px").trim().to_string());
    ResolvedStyle {
        fill: get("fill"),
        stroke: get("stroke"),
        stroke_width: strip_px(get("stroke-width")),
        stroke_dasharray: get("stroke-dasharray"),
        color: get("color"),
        font_size: strip_px(get("font-size")),
        font_weight: get("font-weight"),
        font_style: get("font-style"),
        opacity: get("opacity"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(pairs: &[(&str, &str)]) -> Style {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn default_classdef_applies_to_unclassed() {
        let mut defs = HashMap::new();
        defs.insert("default".to_string(), s(&[("fill", "#eee")]));
        let r = resolve_style(&defs, &[], &Style::new());
        assert_eq!(r.fill.as_deref(), Some("#eee"));
    }

    #[test]
    fn precedence_default_named_inline() {
        let mut defs = HashMap::new();
        defs.insert("default".to_string(), s(&[("fill", "#111")]));
        defs.insert("foo".to_string(), s(&[("fill", "#222")]));
        let inline = s(&[("fill", "#333")]);
        // default < named < inline
        let r = resolve_style(&defs, &["foo".to_string()], &inline);
        assert_eq!(r.fill.as_deref(), Some("#333"));
        // named overrides default when no inline
        let r2 = resolve_style(&defs, &["foo".to_string()], &Style::new());
        assert_eq!(r2.fill.as_deref(), Some("#222"));
    }

    #[test]
    fn later_class_wins_on_conflict() {
        let mut defs = HashMap::new();
        defs.insert("a".to_string(), s(&[("stroke", "#a00")]));
        defs.insert("b".to_string(), s(&[("stroke", "#0b0")]));
        let r = resolve_style(&defs, &["a".to_string(), "b".to_string()], &Style::new());
        assert_eq!(r.stroke.as_deref(), Some("#0b0"));
    }

    #[test]
    fn strips_px_units() {
        let r = pick(&s(&[("stroke-width", "4px"), ("font-size", "18px")]));
        assert_eq!(r.stroke_width.as_deref(), Some("4"));
        assert_eq!(r.font_size.as_deref(), Some("18"));
    }

    #[test]
    fn honors_font_weight_style_opacity() {
        let r = pick(&s(&[
            ("font-weight", "bold"),
            ("font-style", "italic"),
            ("opacity", "0.5"),
        ]));
        assert_eq!(r.font_weight.as_deref(), Some("bold"));
        assert_eq!(r.font_style.as_deref(), Some("italic"));
        assert_eq!(r.opacity.as_deref(), Some("0.5"));
        assert_eq!(
            r.text_attrs(),
            " font-weight=\"bold\" font-style=\"italic\""
        );
        assert!(r
            .shape_attrs("#fff", "#000", "1")
            .contains("opacity=\"0.5\""));
    }

    #[test]
    fn edge_default_then_indexed() {
        let default = s(&[("stroke", "#000"), ("stroke-width", "1")]);
        let indexed = s(&[("stroke", "#ff3")]);
        let r = resolve_edge_style(&default, Some(&indexed));
        assert_eq!(r.stroke.as_deref(), Some("#ff3"));
        assert_eq!(r.stroke_width.as_deref(), Some("1"));
    }
}
