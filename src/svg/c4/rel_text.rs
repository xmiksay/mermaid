//! A relation's label block: wrapped label and `[techn]` lines on one
//! background, and the room a bundle of them needs.

use crate::parse::C4Relation;

use super::wrap::{measure, wrap, Font};

/// Relation labels wrap at about the width they used to be truncated at —
/// or narrower in left-right layouts, where a label's width is what spaces
/// the layers apart.
const REL_LABEL_MAX_W: f64 = 200.0;
const REL_NARROW_MAX_W: f64 = 125.0;
const REL_LABEL_LINE_H: f64 = 12.0;
const REL_TECH_LINE_H: f64 = 11.0;
const REL_LABEL_VPAD: f64 = 3.0;

/// A relation's wrapped label (`label` lines, then italic `[techn]` lines),
/// drawn on one opaque background of `w`×`h` centered on the label point.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct RelLabel {
    pub label: Vec<String>,
    pub tech: Vec<String>,
    pub w: f64,
    pub h: f64,
}

impl RelLabel {
    pub(super) fn new(r: &C4Relation, narrow: bool) -> Option<Self> {
        let max_w = if narrow {
            REL_NARROW_MAX_W
        } else {
            REL_LABEL_MAX_W
        };
        let label = wrap(&r.label, max_w, Font::Label);
        let tech = r
            .technology
            .as_deref()
            .filter(|t| !t.trim().is_empty())
            .map(|t| wrap(&format!("[{t}]"), max_w, Font::LabelTech))
            .unwrap_or_default();
        if label.is_empty() && tech.is_empty() {
            return None;
        }
        let w = label
            .iter()
            .map(|l| measure(l, Font::Label))
            .chain(tech.iter().map(|t| measure(t, Font::LabelTech)))
            .fold(0.0_f64, f64::max)
            + 6.0;
        let h = label.len() as f64 * REL_LABEL_LINE_H
            + tech.len() as f64 * REL_TECH_LINE_H
            + 2.0 * REL_LABEL_VPAD;
        Some(Self { label, tech, w, h })
    }

    /// Baselines of every label line then every tech line, for a block
    /// centered vertically on `cy`.
    pub(super) fn baselines(&self, cy: f64) -> (Vec<f64>, Vec<f64>) {
        let top = cy - self.h / 2.0 + REL_LABEL_VPAD;
        let label: Vec<f64> = (0..self.label.len())
            .map(|i| top + 9.5 + i as f64 * REL_LABEL_LINE_H)
            .collect();
        let tech_top = top + self.label.len() as f64 * REL_LABEL_LINE_H;
        let tech: Vec<f64> = (0..self.tech.len())
            .map(|i| tech_top + 8.5 + i as f64 * REL_TECH_LINE_H)
            .collect();
        (label, tech)
    }
}

/// The largest combined `(width, height)` of the labels of any bundle of
/// relations joining one pair of elements (either direction; a lone
/// relation is a bundle of one), with `gap` between neighbouring labels.
/// Parallel relations run side by side, so their labels have to fit in the
/// space between the two shapes one after another along the lines.
pub(super) fn bundle_label_extent(rels: &[C4Relation], narrow: bool, gap: f64) -> (f64, f64) {
    let mut bundles: std::collections::HashMap<(&str, &str), (f64, f64, usize)> =
        std::collections::HashMap::new();
    for r in rels.iter().filter(|r| r.from != r.to) {
        let Some(l) = RelLabel::new(r, narrow) else {
            continue;
        };
        let key = if r.from <= r.to {
            (r.from.as_str(), r.to.as_str())
        } else {
            (r.to.as_str(), r.from.as_str())
        };
        let e = bundles.entry(key).or_insert((0.0, 0.0, 0));
        *e = (e.0 + l.w, e.1 + l.h, e.2 + 1);
    }
    bundles
        .values()
        .fold((0.0_f64, 0.0_f64), |(w, h), &(bw, bh, n)| {
            let gaps = n.saturating_sub(1) as f64 * gap;
            (w.max(bw + gaps), h.max(bh + gaps))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::C4RelDirection;

    fn rel(label: &str, tech: Option<&str>) -> C4Relation {
        C4Relation {
            from: "a".into(),
            to: "b".into(),
            label: label.into(),
            technology: tech.map(Into::into),
            sprite: None,
            tags: None,
            link: None,
            direction: C4RelDirection::Default,
            bidirectional: false,
            reversed: false,
        }
    }

    #[test]
    fn bundles_add_up_their_labels() {
        let rels = [rel("one", None), rel("two", None), rel("three", None)];
        let single = RelLabel::new(&rels[0], false).expect("labelled");
        let (w, h) = bundle_label_extent(&rels, false, 6.0);
        assert!(h > 3.0 * single.h && w > 3.0 * single.w * 0.8, "{w} {h}");
        let (_, lone) = bundle_label_extent(&rels[..1], false, 6.0);
        assert_eq!(lone, single.h);
    }

    #[test]
    fn long_relation_label_wraps_instead_of_truncating() {
        let r = rel(
            "Findings, evidence metadata, treatment status / pull context",
            Some("HTTPS mTLS, outbound"),
        );
        let l = RelLabel::new(&r, false).expect("labelled");
        assert!(l.label.len() >= 2);
        assert!(l.label.iter().all(|s| !s.contains('…')));
        assert!(l
            .label
            .iter()
            .all(|s| measure(s, Font::Label) <= REL_LABEL_MAX_W));
        // An unbreakable label is broken too, never wider than the maximum.
        let token = RelLabel::new(
            &rel(
                "Unbreakable_relation_label_token_without_spaces_that_is_much_longer_than_any_box",
                None,
            ),
            false,
        )
        .expect("labelled");
        assert!(token.label.len() > 1 && token.w <= REL_LABEL_MAX_W + 6.0);
        assert_eq!(
            l.label.join(" "),
            "Findings, evidence metadata, treatment status / pull context"
        );
        assert_eq!(l.tech, vec!["[HTTPS mTLS, outbound]"]);
        // The background covers every line.
        let (lb, tb) = l.baselines(0.0);
        let last = *tb.last().expect("tech line");
        assert!(lb[0] - 9.0 >= -l.h / 2.0 && last <= l.h / 2.0);
        assert!(RelLabel::new(&rel("", None), false).is_none());
        let narrow = RelLabel::new(&r, true).expect("labelled");
        assert!(narrow.w < l.w && narrow.label.len() > l.label.len());
    }
}
