//! C4 relation drawing: straight point-to-point connectors clipped to each
//! node, with an arrow head and an optional `[techn]` tag.

use std::collections::HashMap;

use crate::parse::{C4RelStyle, C4Relation};

use super::super::builder::{fnum, SvgBuilder};
use super::super::geometry::{clip_rect, polyline_midpoint};
use super::super::label::edge_label_bg;
use super::super::label_layout::{rect_overlap_area, segment_rect_overlap};
use super::super::metrics::text_width;
use super::super::theme::Theme;
use super::C4_LINE;

/// A line segment obstacle: another relation's clipped-to-boundary endpoints.
type Segment = ((f64, f64), (f64, f64));

/// A relation's clipped endpoints, keyed by the alias pair it connects — kept
/// around so every relation's label search can also dodge every *other*
/// relation's routed line, not just node rects (#382).
struct RelLine {
    from: String,
    to: String,
    p0: (f64, f64),
    p1: (f64, f64),
}

/// Draw every relation in `rels`. Relations are resolved in source order, and
/// each one's placed label rect(s) become an obstacle for every relation
/// drawn after it — the same mechanism the upstream `#312` opposite-pair
/// stagger uses elsewhere, applied here to keep two relations between the same
/// pair of elements (e.g. `RelIndex` steps that go back and forth) from
/// stacking their labels exactly on top of one another (#381).
pub(super) fn draw_rels(
    rels: &[C4Relation],
    rel_styles: &HashMap<(&str, &str), &C4RelStyle>,
    pos: &HashMap<String, (f64, f64, f64, f64)>,
    svg: &mut SvgBuilder,
    theme: &Theme,
) {
    let lines: Vec<Option<RelLine>> = rels
        .iter()
        .map(|r| {
            let &(ax, ay, aw, ah) = pos.get(&r.from)?;
            let &(bx, by, bw, bh) = pos.get(&r.to)?;
            let (sx, sy) = (ax + aw / 2.0, ay + ah / 2.0);
            let (tx, ty) = (bx + bw / 2.0, by + bh / 2.0);
            Some(RelLine {
                from: r.from.clone(),
                to: r.to.clone(),
                p0: clip_rect((tx, ty), (sx, sy), (aw, ah)),
                p1: clip_rect((sx, sy), (tx, ty), (bw, bh)),
            })
        })
        .collect();

    let mut placed_labels: Vec<(f64, f64, f64, f64)> = Vec::new();

    for (i, r) in rels.iter().enumerate() {
        let Some(line) = &lines[i] else { continue };
        let ov = rel_styles.get(&(r.from.as_str(), r.to.as_str())).copied();

        // A relation reversing this one's pair (`B->A` alongside `A->B`) is
        // collinear by construction — its line can never be dodged without
        // also dodging our own, so it's excluded here and handled instead via
        // `placed_labels` below.
        let other_lines: Vec<Segment> = lines
            .iter()
            .enumerate()
            .filter_map(|(j, l)| {
                if j == i {
                    return None;
                }
                let l = l.as_ref()?;
                let same_pair =
                    (l.from == r.from && l.to == r.to) || (l.from == r.to && l.to == r.from);
                (!same_pair).then_some((l.p0, l.p1))
            })
            .collect();

        draw_one_rel(
            r,
            ov,
            line.p0,
            line.p1,
            pos,
            &other_lines,
            &mut placed_labels,
            svg,
            theme,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_one_rel(
    r: &C4Relation,
    ov: Option<&C4RelStyle>,
    p_first: (f64, f64),
    p_last: (f64, f64),
    pos: &HashMap<String, (f64, f64, f64, f64)>,
    other_lines: &[Segment],
    placed_labels: &mut Vec<(f64, f64, f64, f64)>,
    svg: &mut SvgBuilder,
    theme: &Theme,
) {
    let fg: &str = ov
        .and_then(|s| s.text_color.as_deref())
        .unwrap_or(&theme.fg);
    let fg_muted = &theme.fg_muted;
    let stroke: &str = ov.and_then(|s| s.line_color.as_deref()).unwrap_or(C4_LINE);

    let markers = if r.bidirectional {
        "marker-start=\"url(#c4-arrow)\" marker-end=\"url(#c4-arrow)\""
    } else {
        "marker-end=\"url(#c4-arrow)\""
    };

    svg.path(
        &format!(
            "M{} {} L{} {}",
            fnum(p_first.0),
            fnum(p_first.1),
            fnum(p_last.0),
            fnum(p_last.1),
        ),
        &format!("fill=\"none\" stroke=\"{stroke}\" stroke-width=\"1\" {markers}"),
    );

    let label = &r.label;
    let tech = r.technology.as_deref();
    if label.is_empty() && tech.is_none() {
        return;
    }

    let label = truncate(label, 36);
    let tech = tech.map(|t| format!("[{}]", truncate(t, 30)));
    let lw = text_width(&label, 8.0, 10.0) + 6.0;
    let tw = tech.as_deref().map(|t| text_width(t, 8.0, 9.0) + 6.0);

    // Nodes the relation doesn't itself connect to must stay clear of the
    // label — a straight point-to-point line can pass directly through an
    // unrelated node in a cramped row-flow layout, and upstream never lets
    // the label land on its fill (#366). Every other relation's already-placed
    // label is an obstacle too, so two relations between the same pair (or
    // any other pair that lands its label at the same spot) don't stack their
    // labels on top of each other (#381).
    let obstacles: Vec<(f64, f64, f64, f64)> = pos
        .iter()
        .filter(|(id, _)| **id != r.from && **id != r.to)
        .map(|(_, &rect)| rect)
        .chain(placed_labels.iter().copied())
        .collect();
    let (mut mx, mut my) = find_clear_label_point(p_first, p_last, lw, tw, &obstacles, other_lines);
    if let Some(s) = ov {
        mx += s.offset_x.unwrap_or(0.0);
        my += s.offset_y.unwrap_or(0.0);
    }
    let rects = label_rects(mx, my, lw, tw);
    placed_labels.extend(rects.iter().copied());

    // Opaque background behind each label line so the relation label stays
    // legible where it crosses geometry (upstream `edgeLabelBackground`, #260).
    if let Some(tech) = tech {
        let tw = tw.expect("tw is Some whenever tech is Some");
        edge_label_bg(svg, mx, my - 4.0, lw, 13.0, theme);
        edge_label_bg(svg, mx, my + 9.0, tw, 12.0, theme);
        svg.text(
            mx,
            my - 1.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"10\""),
            &label,
        );
        svg.text(
            mx,
            my + 12.0,
            &format!(
                "text-anchor=\"middle\" fill=\"{fg_muted}\" font-size=\"9\" font-style=\"italic\""
            ),
            &tech,
        );
    } else {
        edge_label_bg(svg, mx, my, lw, 14.0, theme);
        svg.text(
            mx,
            my + 4.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"10\""),
            &label,
        );
    }
}

/// The background rect(s) a label centered at `(mx, my)` occupies — one for a
/// plain label, two (label + `[techn]`) when `tw` is `Some`. Mirrors the
/// geometry the draw calls in [`draw_one_rel`] use, so `placed_labels` tracks
/// exactly what got painted.
fn label_rects(mx: f64, my: f64, lw: f64, tw: Option<f64>) -> Vec<(f64, f64, f64, f64)> {
    match tw {
        Some(tw) => vec![
            (mx - lw / 2.0, my - 10.5, lw, 13.0),
            (mx - tw / 2.0, my + 3.0, tw, 12.0),
        ],
        None => vec![(mx - lw / 2.0, my - 7.0, lw, 14.0)],
    }
}

/// Slide the label along the relation line — preferring its arc-length
/// midpoint — until its background rect(s) clear every `obstacles` rect and
/// don't cross any of `line_obstacles`. Falls back to the position with the
/// least overlap if the line is too short for a fully clear spot (e.g. two
/// adjacent nodes with barely a gap).
fn find_clear_label_point(
    p0: (f64, f64),
    p1: (f64, f64),
    lw: f64,
    tw: Option<f64>,
    obstacles: &[(f64, f64, f64, f64)],
    line_obstacles: &[Segment],
) -> (f64, f64) {
    if obstacles.is_empty() && line_obstacles.is_empty() {
        return polyline_midpoint(&[p0, p1]);
    }
    const STEPS: i32 = 100;
    let mut best = (p0.0 + 0.5 * (p1.0 - p0.0), p0.1 + 0.5 * (p1.1 - p0.1));
    let mut best_overlap = f64::INFINITY;
    for i in 0..=STEPS {
        let k = (i + 1) / 2;
        let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
        let t = (0.5 + sign * k as f64 / STEPS as f64).clamp(0.0, 1.0);
        let mx = p0.0 + t * (p1.0 - p0.0);
        let my = p0.1 + t * (p1.1 - p0.1);
        let overlap = label_overlap(mx, my, lw, tw, obstacles, line_obstacles);
        if overlap <= 0.0 {
            return (mx, my);
        }
        if overlap < best_overlap {
            best_overlap = overlap;
            best = (mx, my);
        }
    }
    best
}

/// Total overlap between the label's background rect(s), centered at
/// `(mx, my)`, and every rect in `obstacles`, plus the crossing length of
/// every segment in `line_obstacles` that runs through them.
fn label_overlap(
    mx: f64,
    my: f64,
    lw: f64,
    tw: Option<f64>,
    obstacles: &[(f64, f64, f64, f64)],
    line_obstacles: &[Segment],
) -> f64 {
    let rects = label_rects(mx, my, lw, tw);
    let rect_overlap: f64 = rects
        .iter()
        .flat_map(|rect| {
            obstacles
                .iter()
                .map(move |obs| rect_overlap_area(*rect, *obs))
        })
        .sum();
    let line_overlap: f64 = rects
        .iter()
        .flat_map(|rect| {
            line_obstacles
                .iter()
                .map(move |(p0, p1)| segment_rect_overlap(*p0, *p1, *rect))
        })
        .sum();
    rect_overlap + line_overlap
}

fn truncate(s: &str, n: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= n {
        s.to_string()
    } else {
        let mut out: String = chars[..n.saturating_sub(1)].iter().collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_clear_label_point_uses_midpoint_when_unobstructed() {
        let p = find_clear_label_point((0.0, 0.0), (100.0, 0.0), 20.0, None, &[], &[]);
        assert_eq!(p, (50.0, 0.0));
    }

    #[test]
    fn find_clear_label_point_dodges_a_node_straddling_the_midpoint() {
        // Mirrors #366: a third node sits directly on the line between the
        // two shapes the relation connects, covering the geometric midpoint.
        let obstacle = (30.0, -10.0, 40.0, 20.0); // x 30..70, y -10..10
        let (mx, my) =
            find_clear_label_point((0.0, 0.0), (100.0, 0.0), 20.0, None, &[obstacle], &[]);
        let label_rect = (mx - 10.0, my - 7.0, 20.0, 14.0);
        assert_eq!(
            rect_overlap_area(label_rect, obstacle),
            0.0,
            "label {label_rect:?} must clear obstacle {obstacle:?}"
        );
        assert_eq!(my, 0.0, "label should stay on the relation line");
    }

    #[test]
    fn find_clear_label_point_falls_back_when_no_gap_exists() {
        // The obstacle covers the whole line, so no clear spot exists — the
        // best-effort fallback must still return a finite point on the line.
        let obstacle = (-100.0, -10.0, 300.0, 20.0);
        let (mx, my) =
            find_clear_label_point((0.0, 0.0), (100.0, 0.0), 20.0, None, &[obstacle], &[]);
        assert!(mx.is_finite() && my.is_finite());
    }

    #[test]
    fn find_clear_label_point_dodges_a_crossing_line() {
        // An unrelated relation's line runs vertically straight through the
        // midpoint of this one's line — the label must slide off it (#382).
        let crossing = ((50.0, -50.0), (50.0, 50.0));
        let (mx, my) =
            find_clear_label_point((0.0, 0.0), (100.0, 0.0), 20.0, None, &[], &[crossing]);
        let label_rect = (mx - 10.0, my - 7.0, 20.0, 14.0);
        assert_eq!(
            segment_rect_overlap(crossing.0, crossing.1, label_rect),
            0.0,
            "label {label_rect:?} must clear the crossing line {crossing:?}"
        );
    }

    #[test]
    fn draw_rels_separates_reversed_pair_labels() {
        // #381: RelIndex(2, web, api, ...) and RelIndex(4, api, web, ...) are
        // the exact same line, reversed. Their labels must not land on top of
        // one another once both are placed.
        let mut pos = HashMap::new();
        pos.insert("web".to_string(), (292.0, 88.0, 220.0, 130.0));
        pos.insert("api".to_string(), (32.0, 258.0, 220.0, 130.0));
        let rels = vec![
            C4Relation {
                from: "web".to_string(),
                to: "api".to_string(),
                label: "POST /checkout".to_string(),
                technology: None,
                sprite: None,
                tags: None,
                link: None,
                direction: Default::default(),
                bidirectional: false,
            },
            C4Relation {
                from: "api".to_string(),
                to: "web".to_string(),
                label: "Order confirmation".to_string(),
                technology: None,
                sprite: None,
                tags: None,
                link: None,
                direction: Default::default(),
                bidirectional: false,
            },
        ];
        let rel_styles = HashMap::new();
        let theme = Theme::default_theme();
        let mut svg = SvgBuilder::new(600.0, 500.0).theme(&theme);
        draw_rels(&rels, &rel_styles, &pos, &mut svg, &theme);
        let out = svg.finish();
        assert!(out.contains("POST /checkout"));
        assert!(out.contains("Order confirmation"));

        // Parse the opaque label-background rects (the theme's
        // `flow_label_bg` fill) and check no two of them overlap — a hidden
        // label would still show up as text in the markup, so the real
        // regression is two backgrounds painted on top of one another.
        let bg = format!("fill=\"{}\" stroke=\"none\"", theme.flow_label_bg);
        let label_rects: Vec<(f64, f64, f64, f64)> = out
            .split("<rect ")
            .skip(1)
            .filter(|chunk| chunk.contains(&bg))
            .map(|chunk| {
                let attr = |name: &str| -> f64 {
                    let needle = format!("{name}=\"");
                    let start = chunk.find(&needle).unwrap() + needle.len();
                    let end = chunk[start..].find('"').unwrap() + start;
                    chunk[start..end].parse().unwrap()
                };
                (attr("x"), attr("y"), attr("width"), attr("height"))
            })
            .collect();
        assert!(
            label_rects.len() >= 2,
            "expected at least two label backgrounds, got {label_rects:?}"
        );
        for i in 0..label_rects.len() {
            for j in (i + 1)..label_rects.len() {
                assert_eq!(
                    rect_overlap_area(label_rects[i], label_rects[j]),
                    0.0,
                    "label backgrounds {:?} and {:?} overlap",
                    label_rects[i],
                    label_rects[j]
                );
            }
        }
    }
}
