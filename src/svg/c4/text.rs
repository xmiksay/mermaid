//! Text-driven sizing for C4 shapes and relation labels: both the box height
//! and the label background are derived from the same wrapped line layout
//! that gets drawn, so text can never spill out of its box.

use crate::parse::{C4Element, C4ElementKind};

pub(super) use super::rel_text::{bundle_label_extent, RelLabel};
use super::wrap::{wrap, Font};

/// Upstream's fixed shape width; height follows the wrapped text.
pub(super) const BOX_W: f64 = 220.0;
/// Floor so a shape holding a single short name doesn't collapse to a strip.
const BOX_MIN_H: f64 = 70.0;
/// Cylinder cap half-height (`ry`) — the top cap eats `2·ry` of the box, the
/// bottom arc `ry`.
pub(super) const CYL_RY: f64 = 10.0;
/// A queue is the same cylinder lying down, as upstream's `h-cyl` shape
/// (`rendering-elements/shapes/tiltedCylinder.ts`) draws it: end caps are
/// ellipses of half-height `ry = h/2` and half-width
/// `rx = ry / (2.5 + h/50)`, which grows a little with height and stays
/// below `QUEUE_RX_MAX`.
pub(super) fn queue_rx(h: f64) -> f64 {
    let ry = h / 2.0;
    ry / (2.5 + h / 50.0)
}
pub(super) const QUEUE_RX_MAX: f64 = 25.0;

/// Height of the centered person figure (head + shoulders), reserved between
/// the stereotype and the name in a person box.
pub(super) const PERSON_GLYPH_H: f64 = 30.0;

const TOP_PAD: f64 = 6.0;
const BOTTOM_PAD: f64 = 10.0;
/// Room left of and right of the text inside a shape, in total.
const SIDE_PAD: f64 = 16.0;

/// The widest a line of shape text may be in a `w`-wide shape.
pub(super) fn inner_width(w: f64) -> f64 {
    (w - SIDE_PAD).max(40.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LineRole {
    Stereotype,
    Name,
    Techn,
    Descr,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct TextLine {
    pub text: String,
    pub role: LineRole,
    /// Baseline, relative to the top of the text block.
    pub baseline: f64,
}

/// A shape's wrapped text block, stacked top-down as upstream does:
/// stereotype → (person glyph) → name → `[techn]` → description, each
/// following the previous one at its normal line spacing.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ShapeText {
    pub lines: Vec<TextLine>,
    /// Top of the person glyph, relative to the block top.
    pub glyph_top: Option<f64>,
    pub height: f64,
}

/// A queue's text keeps clear of its end caps — the back cap's `rx` and the
/// front cap's full ellipse (`2 * rx`), the same on both sides — at the caps'
/// widest (`QUEUE_RX_MAX`), so the wrap never depends on the height it
/// produces (a grid row may stretch the queue taller still)
/// so the centred block fits.
pub(super) fn layout_shape_text(el: &C4Element, stereotype: &str, w: f64) -> ShapeText {
    let caps = if is_queue(el.kind) {
        4.0 * QUEUE_RX_MAX
    } else {
        0.0
    };
    layout_with(el, stereotype, (inner_width(w) - caps).max(40.0))
}

pub(super) fn is_queue(kind: C4ElementKind) -> bool {
    matches!(
        kind,
        C4ElementKind::SystemQueue | C4ElementKind::ContainerQueue | C4ElementKind::ComponentQueue
    )
}

fn layout_with(el: &C4Element, stereotype: &str, inner: f64) -> ShapeText {
    let mut lines = Vec::new();
    let mut push = |text: String, role: LineRole, baseline: f64| {
        lines.push(TextLine {
            text,
            role,
            baseline,
        })
    };
    let mut cursor = TOP_PAD;
    push(stereotype.to_string(), LineRole::Stereotype, cursor + 12.0);
    cursor += 20.0;
    let mut glyph_top = None;
    if matches!(el.kind, C4ElementKind::Person) {
        glyph_top = Some(cursor);
        cursor += PERSON_GLYPH_H + 4.0;
    }
    for line in wrap(&el.label, inner, Font::Name) {
        push(line, LineRole::Name, cursor + 12.0);
        cursor += 16.0;
    }
    if let Some(t) = el.technology.as_deref().filter(|t| !t.trim().is_empty()) {
        cursor += 1.0;
        for line in wrap(&format!("[{t}]"), inner, Font::Techn) {
            push(line, LineRole::Techn, cursor + 10.0);
            cursor += 13.0;
        }
    }
    if let Some(d) = &el.descr {
        cursor += 4.0;
        for line in wrap(d, inner, Font::Descr) {
            push(line, LineRole::Descr, cursor + 10.0);
            cursor += 12.0;
        }
    }
    ShapeText {
        lines,
        glyph_top,
        height: cursor + BOTTOM_PAD,
    }
}

/// Room a shape outline takes above and below its text block.
pub(super) fn outline_insets(kind: C4ElementKind) -> (f64, f64) {
    match kind {
        C4ElementKind::SystemDb | C4ElementKind::ContainerDb | C4ElementKind::ComponentDb => {
            (2.0 * CYL_RY, CYL_RY)
        }
        _ => (0.0, 0.0),
    }
}

/// A shape's natural `(w, h)`: fixed width, height from its wrapped text.
pub(super) fn shape_size(el: &C4Element) -> (f64, f64) {
    let text = layout_shape_text(el, "", BOX_W);
    let (top, bottom) = outline_insets(el.kind);
    (BOX_W, (text.height + top + bottom).max(BOX_MIN_H))
}

#[cfg(test)]
mod tests {
    use super::super::wrap::measure;
    use super::*;

    fn element(kind: C4ElementKind, descr: Option<&str>, techn: Option<&str>) -> C4Element {
        C4Element {
            kind,
            alias: "a".into(),
            label: "Name".into(),
            descr: descr.map(Into::into),
            technology: techn.map(Into::into),
            sprite: None,
            tags: None,
            link: None,
            external: false,
            boundary_alias: None,
            boundary_label: None,
            boundary_kind: None,
            boundary_type: None,
            members: vec![],
        }
    }

    fn role_font(role: LineRole) -> Font {
        match role {
            // Drawn at the description's size, never wrapped.
            LineRole::Stereotype => Font::Descr,
            LineRole::Name => Font::Name,
            LineRole::Techn => Font::Techn,
            LineRole::Descr => Font::Descr,
        }
    }

    #[test]
    fn queue_caps_follow_upstream_tilted_cylinder() {
        // rx = (h/2) / (2.5 + h/50): 10.6 at 70px, growing towards 25.
        assert!((queue_rx(70.0) - 35.0 / 3.9).abs() < 1e-9);
        let mut last = 0.0;
        for h in [40.0, 70.0, 140.0, 400.0, 5000.0] {
            let rx = queue_rx(h);
            assert!(rx > last && rx < QUEUE_RX_MAX, "{h}: {rx}");
            last = rx;
        }
    }

    #[test]
    fn queue_text_clears_the_end_caps() {
        // A long name and description: the wrap width is fixed by the caps,
        // never narrowed further as the shape grows taller.
        let descr = "Carries every domain event from all of the services to the analytics pipeline and the audit log consumers";
        let el = C4Element {
            label: "Order events queue with a long name".into(),
            ..element(C4ElementKind::ContainerQueue, Some(descr), Some("Kafka"))
        };
        let text = layout_shape_text(&el, "<<container_queue>>", BOX_W);
        let room = BOX_W - SIDE_PAD - 4.0 * QUEUE_RX_MAX;
        for l in text.lines.iter().filter(|l| l.role != LineRole::Stereotype) {
            assert!(
                measure(&l.text, role_font(l.role)) <= room + 0.5,
                "{:?}",
                l.text
            );
        }
        assert!(text.lines.iter().any(|l| l.text.contains("Order events")));
        // Sized and drawn alike, stereotype or not.
        assert_eq!(layout_shape_text(&el, "", BOX_W).height, text.height);
    }

    #[test]
    fn no_text_line_is_wider_than_the_box() {
        // #396 round 4: wide capitals, URLs, identifiers, CJK and emoji used
        // to spill out of the box sideways.
        let mut el = element(
            C4ElementKind::Container,
            Some("https://payments.internal.example.com/api/v2/charges WWW WWW WWW 日本語のユーザー名がとても長いです 🚀🚀🚀"),
            Some("io.github.someorg.payment-gateway-client"),
        );
        el.label = "MMMMMMMMMMMMMMMMMMMMMMM Supercalifragilisticexpialidocious_service_name".into();
        let text = layout_shape_text(&el, "<<container>>", BOX_W);
        for line in &text.lines {
            let w = measure(&line.text, role_font(line.role));
            assert!(w <= inner_width(BOX_W), "{:?} is {w:.0}px wide", line.text);
        }
        let all: String = text.lines.iter().map(|l| l.text.as_str()).collect();
        assert!(all.contains("charges") && all.contains("client") && all.contains("name"));
    }

    #[test]
    fn empty_technology_draws_no_line() {
        let el = element(C4ElementKind::Container, None, Some(" "));
        let text = layout_shape_text(&el, "", BOX_W);
        assert!(text.lines.iter().all(|l| l.role != LineRole::Techn));
    }

    #[test]
    fn shape_height_grows_with_description() {
        let short = shape_size(&element(C4ElementKind::Container, Some("x"), Some("T")));
        let long_descr = "word ".repeat(40);
        let long = shape_size(&element(
            C4ElementKind::Container,
            Some(&long_descr),
            Some("T"),
        ));
        assert_eq!(short.0, BOX_W);
        assert!(long.1 > short.1 + 50.0, "{long:?} vs {short:?}");
        // The whole text block fits inside the box.
        let text = layout_shape_text(
            &element(C4ElementKind::Container, Some(&long_descr), Some("T")),
            "",
            BOX_W,
        );
        let last = text.lines.last().map(|l| l.baseline).unwrap_or(0.0);
        assert!(last < long.1);
    }

    #[test]
    fn description_directly_follows_technology() {
        let el = element(C4ElementKind::Container, Some("desc"), Some("Rust"));
        let text = layout_shape_text(&el, "<<container>>", BOX_W);
        let techn = text.lines.iter().find(|l| l.role == LineRole::Techn);
        let descr = text.lines.iter().find(|l| l.role == LineRole::Descr);
        let (Some(t), Some(d)) = (techn, descr) else {
            panic!("missing lines in {text:?}");
        };
        assert!(
            d.baseline - t.baseline <= 18.0,
            "gap {}",
            d.baseline - t.baseline
        );
    }

    #[test]
    fn cylinder_and_person_reserve_extra_room() {
        let plain = shape_size(&element(C4ElementKind::Container, Some("d"), None));
        let db = shape_size(&element(C4ElementKind::ContainerDb, Some("d"), None));
        let person = shape_size(&element(C4ElementKind::Person, Some("d"), None));
        assert!(db.1 >= plain.1 + 2.0 * CYL_RY || db.1 == BOX_MIN_H);
        assert!(person.1 > plain.1);
    }
}
