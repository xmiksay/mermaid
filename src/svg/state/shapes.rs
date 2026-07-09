//! State node geometry: intrinsic size per `StateKind` and the custom shape
//! drawing for start/end/choice/fork/join pseudo-states and normal states.

use std::collections::HashMap;

use crate::parse::{State, StateKind, Style};
use crate::svg::builder::{fnum, SvgBuilder};
use crate::svg::metrics::text_width;
use crate::svg::style::resolve_style;
use crate::svg::theme::Theme;

use super::{CHAR_W, CHOICE_H, CHOICE_W, LINE_H, MIN_H, MIN_W, PAD_X, PAD_Y, PSEUDO_R};

pub(super) fn state_size(s: &State, font_size: f64) -> (f64, f64) {
    match s.kind {
        StateKind::Start | StateKind::End | StateKind::History { .. } => {
            (PSEUDO_R * 2.0, PSEUDO_R * 2.0)
        }
        StateKind::Choice => (CHOICE_W, CHOICE_H),
        StateKind::Fork | StateKind::Join => (80.0, 12.0),
        StateKind::Normal => {
            let w = (text_width(&s.label, CHAR_W, font_size) + PAD_X * 2.0).max(MIN_W);
            let h = (LINE_H + PAD_Y * 2.0).max(MIN_H);
            (w, h)
        }
    }
}

pub(super) fn draw_state(
    svg: &mut SvgBuilder,
    (cx, cy): (f64, f64),
    (w, h): (f64, f64),
    s: &State,
    class_defs: &HashMap<String, Style>,
    theme: &Theme,
) {
    let rs = resolve_style(class_defs, &s.classes, &s.style);
    let fg = rs.label_fill(&theme.fg);
    // Pseudo-state marker fill: `&theme.fg` keeps the dark dot on light themes
    // yet stays visible on the dark theme (was a hardcoded near-invisible #333).
    let pseudo = &theme.fg;
    match s.kind {
        StateKind::Start => {
            svg.circle(
                cx,
                cy,
                PSEUDO_R,
                &format!("fill=\"{pseudo}\" stroke=\"none\""),
            );
        }
        StateKind::End => {
            svg.circle(
                cx,
                cy,
                PSEUDO_R,
                &format!("fill=\"none\" stroke=\"{pseudo}\" stroke-width=\"1.5\""),
            );
            svg.circle(
                cx,
                cy,
                PSEUDO_R - 4.0,
                &format!("fill=\"{pseudo}\" stroke=\"none\""),
            );
        }
        StateKind::Choice => {
            let hw = CHOICE_W / 2.0;
            let hh = CHOICE_H / 2.0;
            let d = format!(
                "M{cx} {top}L{right} {cy}L{cx} {bot}L{left} {cy}Z",
                cx = fnum(cx),
                top = fnum(cy - hh),
                right = fnum(cx + hw),
                bot = fnum(cy + hh),
                left = fnum(cx - hw)
            );
            svg.path(
                &d,
                &rs.shape_attrs(&theme.flow_node_fill, &theme.flow_node_stroke, "1.5"),
            );
        }
        StateKind::Fork | StateKind::Join => {
            svg.rect(
                cx - w / 2.0,
                cy - h / 2.0,
                w,
                h,
                &format!("fill=\"{pseudo}\" stroke=\"none\""),
            );
        }
        StateKind::History { deep } => {
            svg.circle(
                cx,
                cy,
                PSEUDO_R,
                &rs.shape_attrs(&theme.flow_node_fill, &theme.flow_node_stroke, "1.5"),
            );
            let label = if deep { "H*" } else { "H" };
            svg.text(
                cx,
                cy + 4.0,
                &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"11\""),
                label,
            );
        }
        StateKind::Normal => {
            let base = rs.shape_attrs(&theme.flow_node_fill, &theme.flow_node_stroke, "1.5");
            svg.rect(
                cx - w / 2.0,
                cy - h / 2.0,
                w,
                h,
                &format!("{base} rx=\"10\""),
            );
            svg.text(
                cx,
                cy + 5.0,
                &format!("text-anchor=\"middle\" fill=\"{fg}\""),
                &s.label,
            );
        }
    }
}
