//! Boundary frames in graph mode: sizing a frame around its items, and
//! where a route crosses one — cutting a piece at the frame, leaving it
//! square-on, and keeping clear of the header text.

use super::super::super::label_layout::segment_rect_overlap;
use crate::parse::C4Element;

use super::super::outline::{bounds, Span};
use super::super::{header_min_w, BOUNDARY_HDR, BOUNDARY_MIN_W, BOUNDARY_PAD};
use super::{Dir, Pt};

/// Clearance a detour keeps from a boundary's header text, and the spacing
/// between several detours past the same header.
const HEADER_CLEAR: f64 = 8.0;
const HEADER_SLOT: f64 = 10.0;
/// Each detour in one cluster also gets its own corridor depth, this far
/// apart, so two detours' sideways legs never share a line; `HEADER_TRACKS`
/// of them fit in `HEADER_ROUTE_ROOM`.
const HEADER_TRACK: f64 = 8.0;
const HEADER_TRACKS: usize = 4;
/// How far a route runs straight out of a frame before it may turn, so it
/// never slides along the frame's border.
const EXIT_STUB: f64 = 12.0;

/// `pts` up to (and including, doubled) the point where it first leaves
/// `frame` — through the top or bottom edge, straight above or below the
/// last point inside.
pub(super) fn until_exit(pts: &[Pt], frame: Span) -> Vec<Pt> {
    let Span { x0, y0, x1, y1 } = frame;
    let inside = |p: Pt| frame.contains(p);
    let mut out: Vec<Pt> = pts.first().copied().into_iter().collect();
    for w in pts.windows(2) {
        if inside(w[1]) {
            out.push(w[1]);
            continue;
        }
        let (a, b) = (w[0], w[1]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let mut t = 1.0_f64;
        if dx > 1e-9 {
            t = t.min((x1 - a.0) / dx);
        } else if dx < -1e-9 {
            t = t.min((x0 - a.0) / dx);
        }
        if dy > 1e-9 {
            t = t.min((y1 - a.1) / dy);
        } else if dy < -1e-9 {
            t = t.min((y0 - a.1) / dy);
        }
        let t = t.max(0.0);
        let mut exit = (a.0 + t * dx, a.1 + t * dy);
        // Leaving through the top or bottom (the port sides), go straight up
        // or down from the last point inside: a slant towards a port off to
        // the side would run along the frame's edge.
        let through_flow_edge = (exit.1 - y0).abs() < 1e-6 || (exit.1 - y1).abs() < 1e-6;
        if through_flow_edge && a.0 >= x0 && a.0 <= x1 {
            exit.0 = a.0;
        }
        out.extend([exit, exit]);
        break;
    }
    out
}

/// Depth of detour corridor `slot` below the header: `HEADER_TRACKS` tracks
/// `HEADER_TRACK` apart, then the same again halfway between, and so on, so
/// no two detours of one header ever share a corridor.
fn track_depth(slot: usize) -> f64 {
    let (round, track) = (slot / HEADER_TRACKS, slot % HEADER_TRACKS);
    let mut offset = 0.0;
    let mut part = HEADER_TRACK / 2.0;
    let mut r = round;
    while r > 0 {
        if r % 2 == 1 {
            offset += part;
        }
        part /= 2.0;
        r /= 2;
    }
    track as f64 * HEADER_TRACK + offset
}

/// If an inside-out piece crosses the boundary's header text, re-route its
/// tail around it: from the last point before the crossing, head for a slot
/// just past the header's far edge — below the header in top-down mode, to
/// its right in left-right mode (where the screen-space header sits on the
/// top-down left) — and leave the frame there. Each detour in one cluster
/// gets its own slot, so they don't share a stretch. Returns whether a
/// detour was made.
pub(super) fn avoid_header(part: &mut Vec<Pt>, header: Span, lr: bool, slot: usize) -> bool {
    let (hx1, hy1) = (header.x1, header.y1);
    let hit = header.to_rect();
    let Some(k) = part
        .windows(2)
        .position(|w| segment_rect_overlap(w[0], w[1], hit) > 0.0)
    else {
        return false;
    };
    let Some(&exit) = part.last() else {
        return false;
    };
    let x = hx1 + HEADER_CLEAR + slot as f64 * HEADER_SLOT;
    part.truncate(k + 1);
    if lr {
        let q = part[k];
        part.extend([(x, q.1), (x, q.1)]);
    } else {
        let below = hy1 + HEADER_CLEAR + track_depth(slot);
        // Points between the header and its corridor would make the turn
        // double back; drop them, then rise straight to the corridor before
        // turning, so the detour never slants along a shape's top.
        while part.len() > 1 && part.last().is_some_and(|p| p.1 < below - 0.5) {
            part.pop();
        }
        let q = *part.last().unwrap_or(&exit);
        if (q.1 - below).abs() > 0.5 {
            part.extend([(q.0, below), (q.0, below)]);
        }
        part.extend([(x, below), (x, below)]);
    }
    part.extend([(x, exit.1), (x, exit.1)]);
    true
}

/// If a piece continuing from an inner boundary's exit at `x` goes straight
/// for its own frame (nothing but the bridge out of the inner block's band
/// in between), drop it vertically through the corridor between the two
/// frames instead of slanting along it.
pub(super) fn drop_straight(part: &mut [Pt], x: f64, frame: Span) {
    let n = part.len();
    if n < 2 || x < frame.x0 || x > frame.x1 {
        return;
    }
    let straight = part[..n - 2].iter().all(|p| (p.0 - x).abs() < 0.5);
    if straight {
        part[n - 2].0 = x;
        part[n - 1].0 = x;
    }
}

/// Extend an inside-out piece that ends on its frame's edge by a short run
/// perpendicular to that edge (`extra` longer through the top or bottom).
pub(super) fn push_stub(part: &mut Vec<Pt>, frame: Span, extra: f64) {
    let Some(&(x, y)) = part.last() else { return };
    let Span { x0, y0, x1, y1 } = frame;
    let stub = if (y - y0).abs() < 0.5 {
        (x, y0 - EXIT_STUB - extra)
    } else if (y - y1).abs() < 0.5 {
        (x, y1 + EXIT_STUB + extra)
    } else if (x - x0).abs() < 0.5 {
        (x0 - EXIT_STUB, y)
    } else if (x - x1).abs() < 0.5 {
        (x1 + EXIT_STUB, y)
    } else {
        return;
    };
    // Doubled, so the curve reaches the stub before it bends and never
    // skims along the frame edge.
    part.extend([stub, stub]);
}

/// Extra room under a boundary's header band, so a route can detour beneath
/// the header text instead of crossing it (see `route::avoid_header`).
pub(super) const HEADER_ROUTE_ROOM: f64 = 32.0;
/// The header text block: title + `[kind]` line, inset like `draw_boundary_rect`.
const HEADER_TEXT_X: f64 = 14.0;
const HEADER_TEXT_H: f64 = 40.0;
/// A frame keeps this much room beside its outermost port.
const PORT_MARGIN: f64 = 10.0;

pub(super) fn items_bbox(centers: &[Pt], sizes: &[Pt]) -> Span {
    bounds(
        centers
            .iter()
            .zip(sizes)
            .map(|(&(cx, cy), &(w, h))| (cx - w / 2.0, cy - h / 2.0, w, h)),
    )
    .unwrap_or(Span::new(0.0, 0.0, 0.0, 0.0))
}

/// The items' bounding box padded like a grid-mode boundary (plus
/// `extra_bottom` of screen-space bottom padding, `enclosure_room`) (header band on
/// the screen-space top, plus `HEADER_ROUTE_ROOM`) and grown to fit the
/// header text, plus the header text's own box — all in top-down space.
pub(super) fn boundary_frame(
    centers: &[Pt],
    sizes: &[Pt],
    port_xs: &[f64],
    b: &C4Element,
    dir: Dir,
    (font_size, extra_bottom): (f64, f64),
) -> (Span, Option<Span>) {
    let Span {
        mut x0,
        y0,
        mut x1,
        y1,
    } = items_bbox(centers, sizes);
    for &px in port_xs {
        x0 = x0.min(px - PORT_MARGIN);
        x1 = x1.max(px + PORT_MARGIN);
    }
    let header_w = header_min_w(b, font_size);
    // Screen-space pads (left, top) and (right, bottom), swapped into TD space.
    let (pl, pt) = dir.swap((
        BOUNDARY_PAD,
        BOUNDARY_PAD + BOUNDARY_HDR + HEADER_ROUTE_ROOM,
    ));
    let (pr, pb) = dir.swap((BOUNDARY_PAD, BOUNDARY_PAD + extra_bottom));
    let min_screen = (
        BOUNDARY_MIN_W.max(header_w),
        BOUNDARY_HDR + HEADER_ROUTE_ROOM + 2.0 * BOUNDARY_PAD,
    );
    let (min_w, min_h) = dir.swap(min_screen);
    let (mut fx0, mut fy0, mut fx1, mut fy1) = (x0 - pl, y0 - pt, x1 + pr, y1 + pb);
    let grow_w = (min_w - (fx1 - fx0)).max(0.0) / 2.0;
    let grow_h = (min_h - (fy1 - fy0)).max(0.0) / 2.0;
    fx0 -= grow_w;
    fx1 += grow_w;
    fy0 -= grow_h;
    fy1 += grow_h;
    // `header_min_w` is the text width plus 2·14 of inset.
    let text = (
        HEADER_TEXT_X,
        0.0,
        header_w - 2.0 * HEADER_TEXT_X,
        HEADER_TEXT_H,
    );
    let (hx, hy, hw, hh) = dir.rect(text);
    let header = Span::new(fx0 + hx, fy0 + hy, fx0 + hx + hw, fy0 + hy + hh);
    (Span::new(fx0, fy0, fx1, fy1), Some(header))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_detour_corridors_never_repeat() {
        let depths: Vec<f64> = (0..16).map(track_depth).collect();
        for (i, a) in depths.iter().enumerate() {
            assert!(*a < HEADER_TRACKS as f64 * HEADER_TRACK, "{a}");
            for b in &depths[i + 1..] {
                assert!((a - b).abs() >= 1.0, "{depths:?}");
            }
        }
    }

    #[test]
    fn until_exit_stops_on_the_frame() {
        let pts = [(5.0, 5.0), (5.0, 8.0), (5.0, 30.0), (40.0, 40.0)];
        let out = until_exit(&pts, Span::new(0.0, 0.0, 10.0, 10.0));
        assert_eq!(out, vec![(5.0, 5.0), (5.0, 8.0), (5.0, 10.0), (5.0, 10.0)]);
    }

    #[test]
    fn avoid_header_detours_below_the_title() {
        // A piece leaving straight up through a header at x 0..100, y 0..40.
        let header = Span::new(0.0, 0.0, 100.0, 40.0);
        let mut part = vec![(50.0, 120.0), (50.0, 60.0), (50.0, 0.0), (50.0, 0.0)];
        assert!(avoid_header(&mut part, header, false, 0));
        let hit = (0.0, 0.0, 100.0, 40.0);
        for w in part.windows(2) {
            assert_eq!(segment_rect_overlap(w[0], w[1], hit), 0.0, "{part:?}");
        }
        assert_eq!(part.last(), Some(&(108.0, 0.0)), "leaves past the header");
        // Clear pieces are untouched.
        let mut clear = vec![(150.0, 120.0), (150.0, 0.0)];
        assert!(!avoid_header(&mut clear, header, false, 0));
        assert_eq!(clear, vec![(150.0, 120.0), (150.0, 0.0)]);
    }

    #[test]
    fn avoid_header_in_left_right_space_runs_beside_it() {
        // In top-down space the left-right header is a column at x 0..40.
        let header = Span::new(0.0, 10.0, 40.0, 110.0);
        let mut part = vec![(80.0, 60.0), (20.0, 30.0), (20.0, 0.0), (20.0, 0.0)];
        assert!(avoid_header(&mut part, header, true, 1));
        let hit = (0.0, 10.0, 40.0, 100.0);
        for w in part.windows(2) {
            assert_eq!(segment_rect_overlap(w[0], w[1], hit), 0.0, "{part:?}");
        }
        assert_eq!(part.last(), Some(&(58.0, 0.0)));
    }

    #[test]
    fn drop_straight_runs_through_the_corridor() {
        // An inner exit at x = 40; the piece slants to an exit at x = 90.
        let frame = Span::new(0.0, 0.0, 200.0, 100.0);
        let mut part = vec![(40.0, 80.0), (40.0, 80.0), (90.0, 100.0), (90.0, 100.0)];
        drop_straight(&mut part, 40.0, frame);
        assert_eq!(part[2..], [(40.0, 100.0), (40.0, 100.0)]);
        // With a real waypoint in between it is left alone.
        let mut part = vec![(70.0, 50.0), (90.0, 100.0), (90.0, 100.0)];
        drop_straight(&mut part, 40.0, frame);
        assert_eq!(part[1], (90.0, 100.0));
    }

    #[test]
    fn push_stub_leaves_perpendicular_to_the_edge() {
        let frame = Span::new(0.0, 0.0, 200.0, 100.0);
        let mut part = vec![(50.0, 50.0), (60.0, 100.0)];
        push_stub(&mut part, frame, 0.0);
        assert_eq!(part.last(), Some(&(60.0, 112.0)));
        let mut top = vec![(50.0, 50.0), (60.0, 0.0)];
        push_stub(&mut top, frame, 0.0);
        assert_eq!(top.last(), Some(&(60.0, -12.0)));
        let mut inside = vec![(50.0, 50.0)];
        push_stub(&mut inside, frame, 0.0);
        assert_eq!(inside.len(), 1);
    }
}
