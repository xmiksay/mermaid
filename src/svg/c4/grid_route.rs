//! Grid-mode detours: a straight connector that would run through a foreign
//! shape or a boundary's header text is re-routed around it — A* over a
//! coarse grid of the free space between rows and columns, simplified to a
//! few waypoints by line of sight.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use super::super::label_layout::segment_rect_overlap;
use super::outline::Rect;
use super::rel_label::inflate;

type Pt = (f64, f64);

/// Grid resolution, and a ceiling on grid cells (coarsened above it).
const CELL: f64 = 8.0;
const MAX_CELLS: usize = 40_000;
/// Clearance kept from obstacles by the search, and the smaller clearance
/// the simplified path must still keep.
const SEARCH_MARGIN: f64 = 10.0;
const PATH_MARGIN: f64 = 5.0;
/// Extra cost of a change of direction, in cells: favours few, long runs.
const TURN_COST: u64 = 3;
/// Cells this close to another relation's line cost `LINE_PENALTY` extra.
const LINE_NEAR: f64 = 8.0;
const LINE_PENALTY: u64 = 40;
/// Straight run out of an end shape before a detour turns.
const STUB: f64 = 12.0;
/// Room around the obstacles the search may use.
const BORDER: f64 = 80.0;

/// Whether the polyline `path` runs through any of `obstacles`.
pub(super) fn blocked(path: &[Pt], obstacles: &[Rect]) -> bool {
    path.windows(2).any(|w| {
        obstacles
            .iter()
            .any(|&o| segment_rect_overlap(w[0], w[1], o) > 0.0)
    })
}

/// A route from `from` to `to` (shape centres inside the rects `own`)
/// around `obstacles`, as B-spline control points with doubled interior
/// waypoints so the curve hugs the clear polyline. `None` if no way round.
pub(super) fn detour(
    from: Pt,
    to: Pt,
    own: [Rect; 2],
    obstacles: &[Rect],
    avoid: &[Vec<Pt>],
) -> Option<Vec<Pt>> {
    let mut hull = [own[0], own[1]].to_vec();
    hull.extend_from_slice(obstacles);
    let super::outline::Span { x0, y0, x1, y1 } = super::outline::bounds(hull)?;
    let (x0, y0) = (x0 - BORDER, y0 - BORDER);
    let (w, h) = (x1 - x0 + 2.0 * BORDER, y1 - y0 + 2.0 * BORDER);
    let mut cell = CELL;
    while (w / cell).ceil() * (h / cell).ceil() > MAX_CELLS as f64 {
        cell *= 2.0;
    }
    let (cols, rows) = ((w / cell).ceil() as usize, (h / cell).ceil() as usize);
    let centre = |c: usize, r: usize| (x0 + (c as f64 + 0.5) * cell, y0 + (r as f64 + 0.5) * cell);
    let inside = |p: Pt, r: Rect| p.0 >= r.0 && p.0 <= r.0 + r.2 && p.1 >= r.1 && p.1 <= r.1 + r.3;
    let wide: Vec<Rect> = obstacles
        .iter()
        .map(|&o| inflate(o, SEARCH_MARGIN))
        .collect();
    let free: Vec<bool> = (0..rows * cols)
        .map(|i| {
            let p = centre(i % cols, i / cols);
            own.iter().any(|&r| inside(p, r)) || !wide.iter().any(|&o| inside(p, o))
        })
        .collect();
    // Running alongside another relation's line costs extra; crossing it
    // only costs a cell or two.
    // Stamp each line's neighbourhood instead of testing every cell
    // against every line.
    let mut penalty = vec![0u64; rows * cols];
    let cell_of = |v: f64, origin: f64, n: usize| {
        (((v - origin) / cell).floor().max(0.0) as usize).min(n - 1)
    };
    for w in avoid.iter().flat_map(|l| l.windows(2)) {
        let (c0, c1) = (
            cell_of(w[0].0.min(w[1].0) - LINE_NEAR, x0, cols),
            cell_of(w[0].0.max(w[1].0) + LINE_NEAR, x0, cols),
        );
        let (r0, r1) = (
            cell_of(w[0].1.min(w[1].1) - LINE_NEAR, y0, rows),
            cell_of(w[0].1.max(w[1].1) + LINE_NEAR, y0, rows),
        );
        for r in r0..=r1 {
            for c in c0..=c1 {
                let p = centre(c, r);
                if penalty[r * cols + c] == 0
                    && seg_distance(p, w[0], w[1]) < LINE_NEAR
                    && !own.iter().any(|&o| inside(p, inflate(o, LINE_NEAR)))
                {
                    penalty[r * cols + c] = LINE_PENALTY;
                }
            }
        }
    }
    let at = |p: Pt| {
        let c = (((p.0 - x0) / cell) as usize).min(cols - 1);
        let r = (((p.1 - y0) / cell) as usize).min(rows - 1);
        r * cols + c
    };
    let (start, goal) = (at(from), at(to));
    let cells = search(start, goal, cols, rows, &free, &penalty)?;
    let mut pts: Vec<Pt> = vec![from];
    pts.extend(cells.iter().map(|&i| centre(i % cols, i / cols)));
    pts.push(to);
    let clear: Vec<Rect> = obstacles.iter().map(|&o| inflate(o, PATH_MARGIN)).collect();
    // A shortcut may not run along another line either (away from the two
    // shapes, where lines between the same pair necessarily meet).
    // Only the first and last legs may touch the end shapes: a shortcut
    // grazing one runs along its border.
    let own_wide: Vec<Rect> = own.iter().map(|&r| inflate(r, PATH_MARGIN)).collect();
    let along = |a: Pt, b: Pt| {
        if a != from && b != to && blocked(&[a, b], &own_wide) {
            return true;
        }
        // Sampled against the stamped neighbourhoods (`penalty`).
        let d = (b.0 - a.0).hypot(b.1 - a.1);
        let steps = (d / (cell / 2.0)).ceil().max(1.0) as usize;
        (0..=steps).any(|k| {
            let t = k as f64 / steps as f64;
            let p = (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
            penalty[at(p)] > 0
        })
    };
    Some(double_interior(&simplify(&pts, &clear, &along)))
}

/// Leave and enter the end shapes square-on: a detour clipped near a
/// shape's corner would otherwise slide along its border on the way out.
/// Each end gets a doubled point `STUB` straight out of the face it sits on,
/// unless the stub or the turn after it would hit one of `obstacles`.
pub(super) fn square_off(path: &[Pt], (from, to): (Rect, Rect), obstacles: &[Rect]) -> Vec<Pt> {
    let n = path.len();
    if n < 2 {
        return path.to_vec();
    }
    let stub = |p: Pt, r: Rect| {
        let faces = [
            ((p.0 - r.0).abs(), (-STUB, 0.0)),
            ((p.0 - r.0 - r.2).abs(), (STUB, 0.0)),
            ((p.1 - r.1).abs(), (0.0, -STUB)),
            ((p.1 - r.1 - r.3).abs(), (0.0, STUB)),
        ];
        let (_, d) = faces
            .into_iter()
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .unwrap_or((0.0, (0.0, 0.0)));
        (p.0 + d.0, p.1 + d.1)
    };
    let clear = |p: Pt, s: Pt, next: Pt| !blocked(&[p, s, next], obstacles);
    let (a, z) = (stub(path[0], from), stub(path[n - 1], to));
    let mut out = vec![path[0]];
    if clear(path[0], a, path[1]) {
        out.extend([a, a]);
    }
    out.extend_from_slice(&path[1..n - 1]);
    if clear(path[n - 1], z, path[n - 2]) {
        out.extend([z, z]);
    }
    out.push(path[n - 1]);
    out
}

/// A* over 8-connected free cells; state = (cell, heading) so turns cost.
fn search(
    start: usize,
    goal: usize,
    cols: usize,
    rows: usize,
    free: &[bool],
    penalty: &[u64],
) -> Option<Vec<usize>> {
    const DIRS: [(i64, i64, u64); 8] = [
        (1, 0, 10),
        (-1, 0, 10),
        (0, 1, 10),
        (0, -1, 10),
        (1, 1, 14),
        (1, -1, 14),
        (-1, 1, 14),
        (-1, -1, 14),
    ];
    let n = cols * rows;
    let state = |cell: usize, d: usize| cell * 9 + d;
    let (gc, gr) = ((goal % cols) as i64, (goal / cols) as i64);
    let heuristic = |cell: usize| {
        let (dx, dy) = (
            ((cell % cols) as i64 - gc).abs(),
            ((cell / cols) as i64 - gr).abs(),
        );
        (10 * dx.max(dy) + 4 * dx.min(dy)) as u64
    };
    let mut best = vec![u64::MAX; n * 9];
    let mut prev = vec![usize::MAX; n * 9];
    let mut heap = BinaryHeap::new();
    best[state(start, 8)] = 0;
    heap.push(Reverse((heuristic(start), 0u64, state(start, 8))));
    while let Some(Reverse((_, cost, s))) = heap.pop() {
        if cost > best[s] {
            continue;
        }
        let (cell, heading) = (s / 9, s % 9);
        if cell == goal {
            let mut path = Vec::new();
            let mut cur = s;
            while cur != usize::MAX {
                path.push(cur / 9);
                cur = prev[cur];
            }
            path.reverse();
            path.dedup();
            return Some(path);
        }
        let (c, r) = ((cell % cols) as i64, (cell / cols) as i64);
        for (d, &(dx, dy, step)) in DIRS.iter().enumerate() {
            let (nc, nr) = (c + dx, r + dy);
            if nc < 0 || nr < 0 || nc >= cols as i64 || nr >= rows as i64 {
                continue;
            }
            let next = nr as usize * cols + nc as usize;
            if !free[next] && next != goal {
                continue;
            }
            let turn = if heading != 8 && heading != d {
                TURN_COST * 10
            } else {
                0
            };
            let ns = state(next, d);
            let nb = cost + step + turn + penalty[next];
            if nb < best[ns] {
                best[ns] = nb;
                prev[ns] = s;
                heap.push(Reverse((nb + heuristic(next), nb, ns)));
            }
        }
    }
    None
}

/// Greedy line-of-sight simplification: from each kept point jump to the
/// farthest later point still reachable in a straight, clear line.
fn simplify(pts: &[Pt], obstacles: &[Rect], along: &dyn Fn(Pt, Pt) -> bool) -> Vec<Pt> {
    let mut out = vec![pts[0]];
    let mut i = 0;
    while i + 1 < pts.len() {
        let mut j = pts.len() - 1;
        while j > i + 1 && (blocked(&[pts[i], pts[j]], obstacles) || along(pts[i], pts[j])) {
            j -= 1;
        }
        out.push(pts[j]);
        i = j;
    }
    out
}

fn seg_distance(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 < 1e-12 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    };
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}

fn double_interior(pts: &[Pt]) -> Vec<Pt> {
    let n = pts.len();
    let mut out = Vec::with_capacity(2 * n);
    for (k, &p) in pts.iter().enumerate() {
        out.push(p);
        if k > 0 && k + 1 < n {
            out.push(p);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detour_goes_around_a_shape_in_the_way() {
        // Two shapes with a third squarely between them.
        let a = (0.0, 0.0, 100.0, 60.0);
        let b = (400.0, 0.0, 100.0, 60.0);
        let wall = (180.0, -20.0, 140.0, 100.0);
        let from = (50.0, 30.0);
        let to = (450.0, 30.0);
        assert!(blocked(&[from, to], &[wall]));
        let route = detour(from, to, [a, b], &[wall], &[]).expect("a way round");
        assert_eq!(route.first(), Some(&from));
        assert_eq!(route.last(), Some(&to));
        assert!(!blocked(&route, &[wall]), "{route:?}");
        assert!(route.len() > 2);
    }

    #[test]
    fn detour_keeps_off_other_lines() {
        // Going round the wall over the top would run along another line
        // drawn there; the detour takes the other side.
        let a = (0.0, 0.0, 100.0, 60.0);
        let b = (400.0, 0.0, 100.0, 60.0);
        let wall = (180.0, -20.0, 140.0, 100.0);
        let other = vec![(100.0, -40.0), (400.0, -40.0)];
        let route = detour(
            (50.0, 30.0),
            (450.0, 30.0),
            [a, b],
            &[wall],
            std::slice::from_ref(&other),
        )
        .expect("a way round");
        // Never within 6px of the other line between the two shapes.
        for w in route.windows(2) {
            for k in 0..=20 {
                let t = k as f64 / 20.0;
                let p = (
                    w[0].0 + t * (w[1].0 - w[0].0),
                    w[0].1 + t * (w[1].1 - w[0].1),
                );
                if p.0 > 110.0 && p.0 < 390.0 {
                    assert!(seg_distance(p, other[0], other[1]) >= 6.0, "{route:?}");
                }
            }
        }
    }

    #[test]
    fn square_off_leaves_each_face_straight() {
        let a = (0.0, 0.0, 100.0, 60.0);
        let b = (0.0, 200.0, 100.0, 60.0);
        let out = square_off(&[(10.0, 60.0), (-20.0, 80.0), (10.0, 200.0)], (a, b), &[]);
        assert_eq!(out[1], (10.0, 72.0));
        assert_eq!(out[out.len() - 2], (10.0, 188.0));
    }

    #[test]
    fn walled_in_target_has_no_detour() {
        let a = (0.0, 0.0, 10.0, 10.0);
        let b = (200.0, 200.0, 10.0, 10.0);
        // A ring of obstacles around b, too tight to pass.
        let ring = [
            (150.0, 150.0, 110.0, 30.0),
            (150.0, 230.0, 110.0, 30.0),
            (150.0, 150.0, 30.0, 110.0),
            (230.0, 150.0, 30.0, 110.0),
        ];
        assert!(detour((5.0, 5.0), (205.0, 205.0), [a, b], &ring, &[]).is_none());
    }
}
