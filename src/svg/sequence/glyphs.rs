//! Participant/actor header glyphs: the plain rounded box, the stick-figure
//! `actor`, and the ZenUML `@Boundary`/`@Control`/`@Entity`/`@Database`
//! stereotype icons. Also the label measurement (`actor_size`) that drives
//! column widths.

use crate::parse::ParticipantKind;
use crate::svg::builder::fnum;
use crate::svg::metrics::text_width;

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_actor(
    svg: &mut SvgBuilder,
    cx: f64,
    top: f64,
    w: f64,
    h: f64,
    label: &str,
    kind: ParticipantKind,
    theme: &Theme,
) {
    match kind {
        ParticipantKind::Actor => draw_actor_figure(svg, cx, top, h, label, theme),
        ParticipantKind::Boundary => {
            draw_stereotype(svg, cx, top, h, label, theme, Glyph::Boundary)
        }
        ParticipantKind::Control => draw_stereotype(svg, cx, top, h, label, theme, Glyph::Control),
        ParticipantKind::Entity => draw_stereotype(svg, cx, top, h, label, theme, Glyph::Entity),
        ParticipantKind::Database => {
            draw_stereotype(svg, cx, top, h, label, theme, Glyph::Database)
        }
        _ => draw_actor_box(svg, cx, top, w, h, label, theme),
    }
}

/// The four UML robustness / persistence stereotype glyphs drawn by ZenUML for
/// `@Boundary`/`@Control`/`@Entity`/`@Database` participants.
pub(super) enum Glyph {
    Boundary,
    Control,
    Entity,
    Database,
}

impl Glyph {
    /// The stereotype glyph for a participant kind, or `None` for a plain
    /// participant / stick-figure actor (which draw their own way).
    pub(super) fn from_kind(kind: ParticipantKind) -> Option<Self> {
        match kind {
            ParticipantKind::Boundary => Some(Glyph::Boundary),
            ParticipantKind::Control => Some(Glyph::Control),
            ParticipantKind::Entity => Some(Glyph::Entity),
            ParticipantKind::Database => Some(Glyph::Database),
            _ => None,
        }
    }
}

/// Draw a stereotype glyph centered on `cx` with the name below it, mirroring
/// upstream ZenUML's boundary/control/entity/database icons.
fn draw_stereotype(
    svg: &mut SvgBuilder,
    cx: f64,
    top: f64,
    h: f64,
    label: &str,
    theme: &Theme,
    glyph: Glyph,
) {
    let r = 10.0;
    let cy = top + r + 4.0;
    draw_glyph_shape(svg, cx, cy, r, &glyph, theme);
    draw_figure_name(svg, cx, top, h, label, theme);
}

/// Draw a stereotype glyph shape centered on `(cx, cy)` with radius `r`. Shared
/// by the top-header layout (name below, [`draw_stereotype`]) and ZenUML's box
/// layout (name to the right, [`draw_participant_icon`]).
pub(super) fn draw_glyph_shape(
    svg: &mut SvgBuilder,
    cx: f64,
    cy: f64,
    r: f64,
    glyph: &Glyph,
    theme: &Theme,
) {
    let stroke = &theme.actor_stroke;
    let fill = &theme.actor_fill;
    let attrs = format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.5\"");
    let line_attrs = format!("stroke=\"{stroke}\" stroke-width=\"1.5\"");
    let path_attrs = format!("fill=\"none\" stroke=\"{stroke}\" stroke-width=\"1.5\"");

    match glyph {
        Glyph::Boundary => {
            // Circle with a vertical bar to its left joined by a short stem.
            let bar_x = cx - r - 8.0;
            svg.line(bar_x, cy - r, bar_x, cy + r, &line_attrs);
            svg.line(bar_x, cy, cx - r, cy, &line_attrs);
            svg.circle(cx, cy, r, &attrs);
        }
        Glyph::Control => {
            // Circle with a small arrowhead on top (rotating-arrow icon).
            svg.circle(cx, cy, r, &attrs);
            let ax = cx;
            let ay = cy - r;
            svg.line(ax, ay, ax - 5.0, ay - 5.0, &line_attrs);
            svg.line(ax, ay, ax - 5.0, ay + 5.0, &line_attrs);
        }
        Glyph::Entity => {
            // Circle sitting on a short underline.
            svg.circle(cx, cy, r, &attrs);
            svg.line(cx - r, cy + r + 3.0, cx + r, cy + r + 3.0, &line_attrs);
        }
        Glyph::Database => {
            // A cylinder: top ellipse, straight sides, bottom arc.
            let rx = r;
            let ry = r * 0.4;
            let cyl_h = r * 1.6;
            let tcy = cy - 0.8 * r;
            let bcy = tcy + cyl_h;
            svg.rect(
                cx - rx,
                tcy,
                rx * 2.0,
                cyl_h,
                &format!("fill=\"{fill}\" stroke=\"none\""),
            );
            svg.line(cx - rx, tcy, cx - rx, bcy, &line_attrs);
            svg.line(cx + rx, tcy, cx + rx, bcy, &line_attrs);
            svg.path(
                &format!(
                    "M{} {} A{} {} 0 0 0 {} {} A{} {} 0 0 0 {} {} Z",
                    fnum(cx - rx),
                    fnum(tcy),
                    fnum(rx),
                    fnum(ry),
                    fnum(cx + rx),
                    fnum(tcy),
                    fnum(rx),
                    fnum(ry),
                    fnum(cx - rx),
                    fnum(tcy),
                ),
                &attrs,
            );
            svg.path(
                &format!(
                    "M{} {} A{} {} 0 0 0 {} {}",
                    fnum(cx - rx),
                    fnum(bcy),
                    fnum(rx),
                    fnum(ry),
                    fnum(cx + rx),
                    fnum(bcy),
                ),
                &path_attrs,
            );
        }
    }
}

/// Draw the stereotype/actor icon for a ZenUML participant box, centered on
/// `(cx, cy)` — the icon sits to the *left* of the name (issue #315), unlike the
/// top-header layout where it sits above.
pub(super) fn draw_participant_icon(
    svg: &mut SvgBuilder,
    cx: f64,
    cy: f64,
    kind: ParticipantKind,
    theme: &Theme,
) {
    match Glyph::from_kind(kind) {
        Some(glyph) => draw_glyph_shape(svg, cx, cy, 8.0, &glyph, theme),
        None if matches!(kind, ParticipantKind::Actor) => draw_actor_icon(svg, cx, cy, theme),
        None => {}
    }
}

/// A compact stick figure centered on `(cx, cy)` for a ZenUML participant box.
fn draw_actor_icon(svg: &mut SvgBuilder, cx: f64, cy: f64, theme: &Theme) {
    let stroke = &theme.actor_stroke;
    let fill = &theme.actor_fill;
    let attrs = format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.5\"");
    let line_attrs = format!("stroke=\"{stroke}\" stroke-width=\"1.5\"");
    let head_r = 5.0;
    let head_cy = cy - 9.0;
    let body_top = head_cy + head_r;
    let body_bot = body_top + 9.0;
    let arm_y = body_top + 3.0;
    svg.circle(cx, head_cy, head_r, &attrs);
    svg.line(cx, body_top, cx, body_bot, &line_attrs);
    svg.line(cx - 7.0, arm_y, cx + 7.0, arm_y, &line_attrs);
    svg.line(cx, body_bot, cx - 5.0, body_bot + 7.0, &line_attrs);
    svg.line(cx, body_bot, cx + 5.0, body_bot + 7.0, &line_attrs);
}

/// Draw a participant's name lines below its glyph, clamped inside the actor's
/// allotted height. Shared by the stick-figure actor and the stereotype glyphs.
fn draw_figure_name(svg: &mut SvgBuilder, cx: f64, top: f64, h: f64, label: &str, theme: &Theme) {
    let fg = theme.actor_text();
    let lines = label_lines(label);
    let mut y = (top + h - (lines.len() as f64 - 1.0) * ACTOR_LINE_H - 2.0).max(top + 34.0);
    for line in &lines {
        svg.text(
            cx,
            y,
            &format!("text-anchor=\"middle\" fill=\"{fg}\""),
            line,
        );
        y += ACTOR_LINE_H;
    }
}

fn draw_actor_box(
    svg: &mut SvgBuilder,
    cx: f64,
    top: f64,
    w: f64,
    h: f64,
    label: &str,
    theme: &Theme,
) {
    let fg = theme.actor_text();
    let actor_fill = &theme.actor_fill;
    let actor_stroke = &theme.actor_stroke;
    let x = cx - w / 2.0;
    svg.rect(
        x,
        top,
        w,
        h,
        &format!("fill=\"{actor_fill}\" stroke=\"{actor_stroke}\" stroke-width=\"1.5\" rx=\"4\""),
    );
    let lines = label_lines(label);
    let n = lines.len() as f64;
    let y0 = top + h / 2.0 - (n - 1.0) * ACTOR_LINE_H / 2.0 + 5.0;
    for (i, line) in lines.iter().enumerate() {
        svg.text(
            cx,
            y0 + i as f64 * ACTOR_LINE_H,
            &format!("text-anchor=\"middle\" fill=\"{fg}\""),
            line,
        );
    }
}

/// Draw an `actor` as a stick figure (head + body + arms + legs) with the name
/// underneath — mirrors upstream `drawActorTypeActor`.
fn draw_actor_figure(svg: &mut SvgBuilder, cx: f64, top: f64, h: f64, label: &str, theme: &Theme) {
    let fg = theme.actor_text();
    let stroke = &theme.actor_stroke;
    let fill = &theme.actor_fill;
    let attrs = format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.5\"");
    let line_attrs = format!("stroke=\"{stroke}\" stroke-width=\"1.5\"");

    let head_r = 7.0;
    let head_cy = top + head_r + 1.0;
    let body_top = head_cy + head_r;
    let body_bot = body_top + 13.0;
    let arm_y = body_top + 5.0;
    let arm_half = 10.0;
    let leg_dx = 8.0;
    let leg_dy = 10.0;

    svg.circle(cx, head_cy, head_r, &attrs);
    svg.line(cx, body_top, cx, body_bot, &line_attrs);
    svg.line(cx - arm_half, arm_y, cx + arm_half, arm_y, &line_attrs);
    svg.line(cx, body_bot, cx - leg_dx, body_bot + leg_dy, &line_attrs);
    svg.line(cx, body_bot, cx + leg_dx, body_bot + leg_dy, &line_attrs);

    // Name sits below the figure, within the actor's allotted height.
    let lines = label_lines(label);
    let mut y = (body_bot + leg_dy + 14.0).min(top + h - 2.0);
    for line in &lines {
        svg.text(
            cx,
            y,
            &format!("text-anchor=\"middle\" fill=\"{fg}\""),
            line,
        );
        y += ACTOR_LINE_H;
    }
}

/// Split a participant label into display lines, honoring `<br/>` (issue #3)
/// and literal `\n` escapes.
pub(super) fn label_lines(label: &str) -> Vec<String> {
    let mut normalized = label.to_string();
    for br in ["<br/>", "<br />", "<br>", "\\n"] {
        normalized = normalized.replace(br, "\n");
    }
    normalized
        .split('\n')
        .map(|l| l.trim().to_string())
        .collect()
}

/// Measure a participant box from its label: width grows to fit the widest
/// line, height grows with line count. Both clamp to sane minimums.
pub(super) fn actor_size(label: &str, font_size: f64) -> (f64, f64) {
    let lines = label_lines(label);
    let widest = lines
        .iter()
        .map(|l| text_width(l, ACTOR_CHAR_W, font_size))
        .fold(0.0_f64, f64::max);
    let w = (widest + ACTOR_PAD_X * 2.0).max(ACTOR_MIN_W);
    let h = (lines.len() as f64 * ACTOR_LINE_H + 14.0).max(ACTOR_H);
    (w, h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn build(s: &str) -> SequenceDiagram {
        match parse(s).unwrap() {
            crate::parse::Diagram::Sequence(d) => d,
            _ => panic!("not sequence"),
        }
    }

    #[test]
    fn actor_box_grows_to_fit_label() {
        // A wide label must produce a box wider than the fixed minimum, and the
        // canvas width must accommodate it.
        let wide = "sequenceDiagram\n\
            participant BE as Backend app06 :8082 (UAT) app14 :8081 (PROD) cyberscore-portal FrankenPHP\n\
            participant A\nA->>BE: hi\n";
        let svg = render(&build(wide), &Theme::default());
        // Find the widest actor rect width; it must exceed the old fixed 110.
        let max_w = svg
            .split("width=\"")
            .skip(1)
            .filter_map(|s| s.split('"').next())
            .filter_map(|s| s.parse::<f64>().ok())
            .fold(0.0_f64, f64::max);
        assert!(max_w > 110.0, "expected a box wider than 110, got {max_w}");
    }

    #[test]
    fn multiline_label_splits_on_br() {
        let svg = render(
            &build("sequenceDiagram\nparticipant BE as Backend<br/>app06\nA->>BE: hi\n"),
            &Theme::default(),
        );
        assert!(svg.contains(">Backend<"));
        assert!(svg.contains(">app06<"));
    }

    #[test]
    fn actor_size_matches_label() {
        assert_eq!(actor_size("A", 14.0), (ACTOR_MIN_W, ACTOR_H));
        let (w, h) = actor_size("one<br/>two<br/>three", 14.0);
        assert!(h > ACTOR_H, "multi-line label should be taller");
        assert_eq!(w, ACTOR_MIN_W, "short lines keep the minimum width");
    }

    #[test]
    fn actor_renders_as_stick_figure() {
        let svg = render(
            &build("sequenceDiagram\nactor A\nparticipant B\nA->>B: hi\n"),
            &Theme::default(),
        );
        // Stick figure emits a <circle> head; a plain participant box does not.
        assert!(svg.contains("<circle"), "actor should draw a circle head");
        assert!(svg.contains(">A</text>"), "actor name below figure");
    }

    #[test]
    fn zenuml_stereotypes_draw_distinct_glyphs() {
        // The database cylinder is drawn with <path> arcs, absent from a plain
        // participant box; the circle-based stereotypes emit a <circle>.
        let db = render(
            &build("zenuml\n@Database DB\n@Actor A\nA.query()\n"),
            &Theme::default(),
        );
        assert!(db.contains("<path"), "database renders a cylinder path");
        assert!(db.contains(">DB<"), "database name is drawn");

        let boundary = render(
            &build("zenuml\n@Boundary UI\nUI.show()\n"),
            &Theme::default(),
        );
        assert!(boundary.contains("<circle"), "boundary renders a circle");
        assert!(boundary.contains(">UI<"), "boundary name is drawn");
    }

    #[test]
    fn participant_stays_a_box() {
        let svg = render(
            &build("sequenceDiagram\nparticipant A\nA->>B: hi\n"),
            &Theme::default(),
        );
        assert!(!svg.contains("<circle"), "participant is a rounded rect");
    }
}
