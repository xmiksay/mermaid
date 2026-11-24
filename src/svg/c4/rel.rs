//! C4 relations: each one follows its placement route (straight in grid
//! mode, a sugiyama-routed spline in graph mode), clipped to the two shapes'
//! drawn outlines, with an arrow head and a wrapped label + `[techn]` block.

use std::collections::HashMap;

use crate::parse::{C4Element, C4RelStyle, C4Relation};

use super::super::builder::{curve_basis_path, curve_basis_points, curve_linear_path, SvgBuilder};
use super::super::label::edge_label_bg;
use super::super::label_layout::rect_at;
use super::super::theme::Theme;
use super::outline::{clip_route, Rect};
use super::rel_annotate::place_labels;
use super::rel_label::{LABEL_CLEARANCE, SHAPE_CLEARANCE};
use super::rel_loop::self_loop;
use super::rel_route::{enclosure_route, endpoint_shapes, RoutePath};
use super::rel_straight::straight_routes;
use super::text::{bundle_label_extent, RelLabel};
use super::{Placement, C4_LINE};

/// Arrow marker id; its `refX` puts the tip exactly on the path's end point.
pub(super) const ARROW_ID: &str = "c4-arrow";
pub(super) const ARROW_REF_X: u32 = 10;
pub(super) const ARROW_SIZE: u32 = 9;

/// The arrowhead itself (9px long, 9px wide) plus a little room: a label may
/// never cover it, in any layout.
pub(super) const ARROWHEAD: f64 = 14.0;
pub(super) const ARROWHEAD_PAD: f64 = 6.0;

/// Minimum visual gap to leave between two reversed-pair labels once both are
/// placed along their shared connector.
const REL_LABEL_CLEARANCE: f64 = 10.0;

/// One relation, fully resolved before anything is drawn so the canvas can be
/// sized to include every line and label.
pub(super) struct RelPlan<'a> {
    pub rel: &'a C4Relation,
    /// Control points of the drawn path, ends on the shape outlines.
    pub path: Vec<(f64, f64)>,
    /// The drawn line itself (the flattened spline when `curved`).
    pub line: Vec<(f64, f64)>,
    /// Drawn as a spline (graph-mode routes, self-loops) or straight.
    pub curved: bool,
    pub label: Option<(RelLabel, (f64, f64))>,
    /// A thin line from the label to its relation's line, when the label had
    /// to sit off it (`rel_place::Spot`).
    pub leader: Option<((f64, f64), (f64, f64))>,
}

impl RelPlan<'_> {
    pub(super) fn label_rect(&self) -> Option<Rect> {
        self.label.as_ref().map(|(l, c)| rect_at(*c, (l.w, l.h)))
    }

    pub(super) fn shift(&mut self, dx: f64, dy: f64) {
        for p in self.path.iter_mut().chain(self.line.iter_mut()) {
            p.0 += dx;
            p.1 += dy;
        }
        if let Some((_, c)) = &mut self.label {
            c.0 += dx;
            c.1 += dy;
        }
        if let Some((a, b)) = &mut self.leader {
            *a = (a.0 + dx, a.1 + dy);
            *b = (b.0 + dx, b.1 + dy);
        }
    }
}

/// The `(column, row)` gaps the grid needs so relation labels fit between
/// the shapes they connect instead of covering them: a column gap as wide as
/// the widest label and a row gap as tall as the tallest, plus clearance on
/// both sides. A reversed pair (`A->B` alongside `B->A`) runs on two
/// parallel lines whose labels sit side by side across a row gap or stacked
/// in a column gap, so it needs their combined width across columns but only
/// their combined height across rows (#399). `(0.0, 0.0)` without labels,
/// so the grid keeps its default spacing.
pub(super) fn min_label_gaps(rels: &[C4Relation]) -> (f64, f64) {
    // Clearance either side, plus the arrowhead the label must not cover.
    let margin = 2.0 * (SHAPE_CLEARANCE + LABEL_CLEARANCE) + ARROWHEAD + ARROWHEAD_PAD;
    let (w, h) = bundle_label_extent(rels, false, REL_LABEL_CLEARANCE);
    if w == 0.0 {
        (0.0, 0.0)
    } else {
        (w + margin, h + margin)
    }
}

/// Resolve every relation's clipped path and label position. Labels are
/// placed in source order, each one an obstacle for every later one, so
/// relations between the same pair (e.g. reversed `RelIndex` steps) don't
/// stack their labels (#381).
pub(super) fn plan_rels<'a>(
    rels: &'a [C4Relation],
    elements: &[C4Element],
    placement: &Placement,
    rel_styles: &HashMap<(&str, &str), &C4RelStyle>,
) -> Vec<RelPlan<'a>> {
    let shapes = endpoint_shapes(elements, placement);
    let needed: Vec<bool> = (0..rels.len())
        .map(|i| placement.routes.get(i).is_none_or(Option::is_none))
        .collect();
    let straight = straight_routes(rels, &shapes, placement, &needed);

    // Self-loops and boundary-anchored relations are planned last, so they
    // can keep clear of every other relation's ends.
    let mut paths: Vec<Option<RoutePath>> = vec![None; rels.len()];
    let mut deferred = Vec::new();
    for (i, r) in rels.iter().enumerate() {
        let (Some(&start), Some(&end)) = (shapes.get(r.from.as_str()), shapes.get(r.to.as_str()))
        else {
            continue;
        };
        paths[i] = if r.from == r.to || start.2 || end.2 {
            deferred.push((i, start, end));
            None
        } else if let Some(route) = placement.routes.get(i).cloned().flatten() {
            let ends = ((start.0, start.1), (end.0, end.1));
            Some((clip_route(&route, ends.0, ends.1), placement.curved))
        } else {
            straight.get(i).cloned().flatten()
        };
    }
    // Boundary-anchored routes may still be routed by the graph layout.
    for &(i, start, end) in &deferred {
        if rels[i].from != rels[i].to {
            if let Some(route) = placement.routes.get(i).cloned().flatten() {
                let ends = ((start.0, start.1), (end.0, end.1));
                paths[i] = Some((clip_route(&route, ends.0, ends.1), placement.curved));
            }
        }
    }
    // Self-loops first (enclosure routes then slide clear of their ends too).
    deferred.sort_by_key(|&(i, ..)| rels[i].from != rels[i].to);
    for (i, start, end) in deferred {
        if paths[i].is_some() {
            continue;
        }
        let taken: Vec<(f64, f64)> = paths
            .iter()
            .flatten()
            .flat_map(|(p, _)| [p.first(), p.last()])
            .flatten()
            .copied()
            .collect();
        paths[i] = if rels[i].from == rels[i].to {
            // The shape's own earlier loops are nested round, not avoided.
            let lines: Vec<&[(f64, f64)]> = paths
                .iter()
                .zip(rels)
                .filter(|(_, r)| !(r.from == r.to && r.from == rels[i].from))
                .filter_map(|(p, _)| p.as_ref().map(|(p, _)| p.as_slice()))
                .collect();
            Some((self_loop(start, placement, &taken, &lines), true))
        } else {
            let label =
                RelLabel::new(&rels[i], placement.narrow_labels).map_or((0.0, 0.0), |l| (l.w, l.h));
            let lines: Vec<&[(f64, f64)]> =
                paths.iter().flatten().map(|(p, _)| p.as_slice()).collect();
            enclosure_route(start, end, placement, (&taken, &lines), label)
                .map(|p| (p, false))
                .or_else(|| straight.get(i).cloned().flatten())
        };
    }
    let mut plans: Vec<RelPlan<'a>> = rels
        .iter()
        .zip(paths)
        .filter_map(|(r, p)| {
            let (path, curved) = p?;
            let line = if curved {
                curve_basis_points(&path)
            } else {
                path.clone()
            };
            Some(RelPlan {
                rel: r,
                path,
                line,
                curved,
                label: None,
                leader: None,
            })
        })
        .collect();

    place_labels(&mut plans, placement, rel_styles);
    plans
}

/// Draw every planned relation: all lines first, then all labels, so no line
/// is painted over an earlier relation's label.
pub(super) fn draw_rels(
    plans: &[RelPlan],
    rel_styles: &HashMap<(&str, &str), &C4RelStyle>,
    svg: &mut SvgBuilder,
    theme: &Theme,
) {
    for plan in plans {
        let r = plan.rel;
        let ov = rel_styles.get(&r.written_pair()).copied();
        let stroke: &str = ov.and_then(|s| s.line_color.as_deref()).unwrap_or(C4_LINE);
        let markers = if r.bidirectional {
            format!("marker-start=\"url(#{ARROW_ID})\" marker-end=\"url(#{ARROW_ID})\"")
        } else {
            format!("marker-end=\"url(#{ARROW_ID})\"")
        };
        let d = if plan.curved {
            curve_basis_path(&plan.path)
        } else {
            curve_linear_path(&plan.path)
        };
        svg.path(
            &d,
            &format!("fill=\"none\" stroke=\"{stroke}\" stroke-width=\"1\" {markers}"),
        );
    }
    for plan in plans {
        let Some((label, (mx, my))) = &plan.label else {
            continue;
        };
        if let Some((a, b)) = plan.leader {
            svg.line(
                a.0,
                a.1,
                b.0,
                b.1,
                &format!("stroke=\"{C4_LINE}\" stroke-width=\"0.75\" stroke-dasharray=\"2 2\""),
            );
        }
        let r = plan.rel;
        let ov = rel_styles.get(&r.written_pair()).copied();
        let fg: &str = ov
            .and_then(|s| s.text_color.as_deref())
            .unwrap_or(&theme.fg);
        let fg_muted = &theme.fg_muted;
        // One opaque background behind the whole block keeps every line
        // legible where it crosses geometry (upstream `edgeLabelBackground`,
        // #260).
        edge_label_bg(svg, *mx, *my, label.w, label.h, theme);
        let (label_ys, tech_ys) = label.baselines(*my);
        for (text, y) in label.label.iter().zip(label_ys) {
            svg.text(
                *mx,
                y,
                &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"10\""),
                text,
            );
        }
        for (text, y) in label.tech.iter().zip(tech_ys) {
            svg.text(
                *mx,
                y,
                &format!(
                    "text-anchor=\"middle\" fill=\"{fg_muted}\" font-size=\"9\" font-style=\"italic\""
                ),
                text,
            );
        }
    }
}
