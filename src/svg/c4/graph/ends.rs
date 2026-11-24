//! Shaping a stitched route's two ends.

use std::collections::HashMap;

use super::runs::apply_shifts;
pub(super) use super::runs::Scenery;
use super::Pt;

/// Widest spacing between neighbouring route ends sharing one shape face.
const MAX_END_SPREAD: f64 = 28.0;
/// Ends stay this far from a face's corners.
pub(super) const END_INSET: f64 = 14.0;
/// The least spacing the layout guarantees between ends on one face (it
/// widens a shape along its faces to fit every relation touching it).
pub(super) const MIN_END_GAP: f64 = 16.0;

/// Fan out route ends that meet the same face (top or bottom, in top-down
/// space) of one shape: ordered by where each route comes from, spaced
/// evenly around the face's centre — instead of every arrow piling onto one
/// point. `ends[i]` names relation `i`'s source and target shapes
/// (`(id, width)`); a shifted run must stay clear of `scenery`.
pub(super) fn spread_ends<K: Copy + Eq + std::hash::Hash>(
    routes: &mut [Option<Vec<Pt>>],
    ends: &[[Option<(K, f64)>; 2]],
    scenery: &Scenery<K>,
) {
    // Per (shape, top face): the shape width and every end arriving there as
    // (approach x, relation, which end, face centre x).
    type Face = (f64, Vec<(f64, usize, usize, f64)>);
    let mut faces: HashMap<(K, bool), Face> = HashMap::new();
    let mut walks: HashMap<(usize, usize), Vec<Pt>> = HashMap::new();
    for (i, route) in routes.iter().enumerate() {
        let Some(r) = route else { continue };
        let n = r.len();
        if n < 2 {
            continue;
        }
        for end in 0..2 {
            let Some(Some((key, w))) = ends.get(i).map(|e| e[end]) else {
                continue;
            };
            let walk: Vec<Pt> = if end == 0 {
                r.clone()
            } else {
                r.iter().rev().copied().collect()
            };
            let p = walk[0];
            // The route leaves the face straight along its own column first;
            // the face is the side it leaves by, and the first point off that
            // column is where it comes from.
            let Some(away) = walk.iter().find(|q| (q.1 - p.1).abs() > 0.5) else {
                continue;
            };
            let from_x = walk
                .iter()
                .find(|q| (q.0 - p.0).abs() > 0.5)
                .map_or(p.0, |q| q.0);
            let face = faces.entry((key, away.1 < p.1)).or_insert((w, Vec::new()));
            face.1.push((from_x, i, end, p.0));
            walks.insert((i, end), walk);
        }
    }
    // Work out every shift on the unshifted routes, then apply them: a
    // route that is one straight run end to end would otherwise be moved by
    // both of its ends.
    let mut shifts: HashMap<usize, [Option<(f64, usize)>; 2]> = HashMap::new();
    for (w, mut list) in faces.into_values() {
        if list.len() < 2 {
            continue;
        }
        // Routes between the same two shapes run as a bundle: they fan out
        // in the order they part, or they cross at a shallow angle on the
        // way. Others by where they come from.
        let pair = |i: usize| {
            let mut k: Vec<K> = ends[i].iter().flatten().map(|e| e.0).collect();
            k.dedup();
            k
        };
        let same_pair = |a: usize, b: usize| {
            let (pa, pb) = (pair(a), pair(b));
            pa.len() == pb.len() && pa.iter().all(|k| pb.contains(k))
        };
        list.sort_by(|a, b| {
            let order = match (walks.get(&(a.1, a.2)), walks.get(&(b.1, b.2))) {
                (Some(wa), Some(wb)) if same_pair(a.1, b.1) => first_divergence(wa, wb),
                _ => std::cmp::Ordering::Equal,
            };
            order.then(a.0.total_cmp(&b.0)).then(a.1.cmp(&b.1))
        });
        let gaps = (list.len() - 1) as f64;
        let spacing = ((w - 2.0 * END_INSET) / gaps).clamp(0.0, MAX_END_SPREAD);
        // Lines heading sideways turn at staggered depths, in staircase
        // order so they don't cross: heading right, the further left a line
        // leaves the deeper it turns; heading left, the mirror image.
        let right: Vec<usize> = (0..list.len())
            .filter(|&k| list[k].0 > list[k].3 + 1.0)
            .collect();
        let left: Vec<usize> = (0..list.len())
            .filter(|&k| list[k].0 < list[k].3 - 1.0)
            .collect();
        let track = |k: usize| {
            if let Some(r) = right.iter().position(|&j| j == k) {
                right.len() - 1 - r
            } else {
                left.iter().position(|&j| j == k).unwrap_or(0)
            }
        };
        for (k, &(_, i, end, _)) in list.iter().enumerate() {
            let dx = (k as f64 - gaps / 2.0) * spacing;
            shifts.entry(i).or_default()[end] = Some((dx, track(k)));
        }
    }
    apply_shifts(routes, &shifts, ends, scenery);
}

/// Make the route leave its source and enter its target along the flow
/// axis (vertical in top-down space), so the curve meets the box's top or
/// bottom face square-on instead of grazing in at a shallow diagonal —
/// the same S-shape flowchart's `basis_control_points` gives a two-point
/// edge, applied to both ends of any route.
pub(super) fn ease_ends(route: &[Pt]) -> Vec<Pt> {
    const ALIGNED_EPS: f64 = 2.0;
    let n = route.len();
    if n < 2 {
        return route.to_vec();
    }
    let (a, b) = (route[0], route[1]);
    let (y, z) = (route[n - 2], route[n - 1]);
    let mut out = Vec::with_capacity(n + 2);
    out.push(a);
    if (b.0 - a.0).abs() > ALIGNED_EPS {
        out.push((a.0, (a.1 + b.1) / 2.0));
    }
    out.extend_from_slice(&route[1..n - 1]);
    if (z.0 - y.0).abs() > ALIGNED_EPS {
        out.push((z.0, (y.1 + z.1) / 2.0));
    }
    out.push(z);
    out
}

/// Left-to-right order of two routes leaving one face (both walked from the
/// face): by their x at the first depth where they part — the order in
/// which they can fan out without crossing.
fn first_divergence(a: &[Pt], b: &[Pt]) -> std::cmp::Ordering {
    let Some(&(_, y0)) = a.first() else {
        return std::cmp::Ordering::Equal;
    };
    let mut levels: Vec<f64> = a
        .iter()
        .chain(b)
        .map(|p| p.1)
        .filter(|y| (y - y0).abs() > 0.5)
        .collect();
    levels.sort_by(|p, q| (p - y0).abs().total_cmp(&(q - y0).abs()));
    for y in levels {
        if let (Some(xa), Some(xb)) = (x_at(a, y), x_at(b, y)) {
            if (xa - xb).abs() > 0.5 {
                return xa.total_cmp(&xb);
            }
        }
    }
    std::cmp::Ordering::Equal
}

/// Where the walk first reaches depth `y`.
fn x_at(walk: &[Pt], y: f64) -> Option<f64> {
    walk.windows(2).find_map(|w| {
        let (lo, hi) = (w[0].1.min(w[1].1), w[0].1.max(w[1].1));
        if y < lo || y > hi || hi - lo < 1e-9 {
            return None;
        }
        Some(w[0].0 + (y - w[0].1) / (w[1].1 - w[0].1) * (w[1].0 - w[0].0))
    })
}

/// Snap a waypoint less than half a pixel from the one before onto it:
/// such slivers are rounding left over from shifting runs and make the
/// curve double back on itself. Exact repeats stay — they are deliberate.
pub(super) fn snap_slivers(route: Vec<Pt>) -> Vec<Pt> {
    let mut out: Vec<Pt> = Vec::with_capacity(route.len());
    for p in route {
        match out.last() {
            Some(&q) if (p.0 - q.0).hypot(p.1 - q.1) < 0.5 => out.push(q),
            _ => out.push(p),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::super::super::label_layout::segment_rect_overlap;
    use super::super::super::outline::Span;
    use super::super::super::rel_label::inflate;
    use super::super::runs::SHIFT_CLEARANCE;
    use super::*;

    fn none() -> Scenery<'static, usize> {
        Scenery {
            shapes: &[],
            skip: &[],
            frames: &[],
            headers: &[],
        }
    }

    #[test]
    fn a_shifted_run_stops_short_of_a_frame_edge() {
        // The run drops out of shape 1 across a frame's bottom edge at
        // y = 50, 15px beside another route's crossing: moving it all
        // would bring the two together, so only the stub above the edge
        // moves.
        let mut routes = vec![
            Some(vec![(0.0, 0.0), (0.0, 40.0), (0.0, 60.0), (300.0, 300.0)]),
            Some(vec![(0.0, 0.0), (-60.0, 100.0)]),
            Some(vec![(15.0, 40.0), (15.0, 60.0)]),
        ];
        let ends = vec![
            [Some((1, 220.0)), None],
            [Some((1, 220.0)), None],
            [None, None],
        ];
        let frames = [Span::new(-100.0, -100.0, 100.0, 50.0)];
        let scenery = Scenery {
            shapes: &[],
            skip: &[],
            frames: &frames,
            headers: &[],
        };
        spread_ends(&mut routes, &ends, &scenery);
        let r = routes[0].as_ref().expect("route");
        assert_eq!(r[..3], [(14.0, 0.0), (14.0, 40.0), (0.0, 60.0)]);
    }

    #[test]
    fn spread_ends_fans_out_a_shared_face() {
        // Three routes arrive at the top of shape 7 (centre x = 0) from the
        // left, the middle and the right.
        let mut routes = vec![
            Some(vec![(-100.0, -100.0), (0.0, 0.0)]),
            Some(vec![(0.0, -100.0), (0.0, 0.0)]),
            Some(vec![(100.0, -100.0), (0.0, 0.0)]),
        ];
        let ends = vec![[None, Some((7, 200.0))]; 3];
        spread_ends(&mut routes, &ends, &none());
        let xs: Vec<f64> = routes.iter().flatten().map(|r| r[1].0).collect();
        assert_eq!(xs, vec![-28.0, 0.0, 28.0]);
    }

    #[test]
    fn spread_ends_moves_the_straight_run_with_the_end() {
        // Both routes drop straight onto shape 3's top from (x, -40) after
        // arriving from opposite sides; the vertical run moves as one.
        let mut routes = vec![
            Some(vec![
                (-90.0, -100.0),
                (0.0, -40.0),
                (0.0, -40.0),
                (0.0, 0.0),
            ]),
            Some(vec![(90.0, -100.0), (0.0, -40.0), (0.0, -40.0), (0.0, 0.0)]),
        ];
        let ends = vec![[None, Some((3, 220.0))]; 2];
        spread_ends(&mut routes, &ends, &none());
        let left = routes[0].as_ref().expect("route");
        let right = routes[1].as_ref().expect("route");
        assert_eq!(left[1..], [(-14.0, -40.0), (-14.0, -40.0), (-14.0, 0.0)]);
        assert_eq!(right[1..], [(14.0, -40.0), (14.0, -40.0), (14.0, 0.0)]);
    }

    #[test]
    fn spread_ends_moves_a_straight_route_once() {
        // Four parallel straight relations between shape 1 (above) and
        // shape 2 (below): each end is shifted once, the routes stay
        // vertical and within the faces.
        let straight = || Some(vec![(0.0, 0.0), (0.0, 50.0), (0.0, 100.0)]);
        let mut routes = vec![straight(), straight(), straight(), straight()];
        let ends = vec![[Some((1, 220.0)), Some((2, 220.0))]; 4];
        spread_ends(&mut routes, &ends, &none());
        for r in routes.iter().flatten() {
            assert!(r.iter().all(|p| (p.0 - r[0].0).abs() < 1e-9), "{r:?}");
            assert!(r[0].0.abs() <= 110.0 - END_INSET);
        }
        let xs: Vec<f64> = routes.iter().flatten().map(|r| r[0].0).collect();
        assert_eq!(xs, vec![-42.0, -14.0, 14.0, 42.0]);
    }

    #[test]
    fn a_shifted_run_never_moves_into_a_shape() {
        // A route drops from shape 1 down a long dummy column beside shape
        // 9; shifting the whole column left would run through shape 9, so
        // only the stretch above it moves.
        let mut routes = vec![
            Some(vec![(0.0, 0.0), (0.0, 40.0), (0.0, 200.0), (300.0, 300.0)]),
            Some(vec![(0.0, 0.0), (-60.0, 100.0)]),
        ];
        let ends = vec![[Some((1, 220.0)), None], [Some((1, 220.0)), None]];
        let shapes = [(9, (10.0, 80.0, 50.0, 60.0))];
        spread_ends(
            &mut routes,
            &ends,
            &Scenery {
                shapes: &shapes,
                skip: &[],
                frames: &[],
                headers: &[],
            },
        );
        let r = routes[0].as_ref().expect("route");
        assert!((r[0].0 - 14.0).abs() < 1e-9, "{r:?}");
        assert_eq!(r[2], (0.0, 200.0), "the column stays: {r:?}");
        let wall = inflate(shapes[0].1, SHIFT_CLEARANCE);
        assert!(r
            .windows(2)
            .all(|w| segment_rect_overlap(w[0], w[1], wall) == 0.0));
    }

    #[test]
    fn ends_are_ordered_where_routes_part() {
        // Two relations between shapes 1 and 2 leave 1 down one column and
        // finish at the same x; the one that steps left first takes the
        // left end.
        let mut routes = vec![
            Some(vec![(0.0, 0.0), (0.0, 50.0), (0.0, 100.0), (-30.0, 300.0)]),
            Some(vec![
                (0.0, 0.0),
                (0.0, 50.0),
                (-12.0, 100.0),
                (-30.0, 300.0),
            ]),
        ];
        let ends = vec![[Some((1, 220.0)), Some((2, 220.0))]; 2];
        spread_ends(&mut routes, &ends, &none());
        let xs: Vec<f64> = routes.iter().flatten().map(|r| r[0].0).collect();
        assert!(xs[1] < xs[0], "{xs:?}");
    }

    #[test]
    fn spread_ends_fits_a_narrow_face() {
        // Five ends on a 70-wide face share its 42px between the insets.
        let mut routes: Vec<_> = (0..5)
            .map(|k| Some(vec![(k as f64 * 10.0, -100.0), (0.0, 0.0)]))
            .collect();
        let ends = vec![[None, Some((1, 70.0))]; 5];
        spread_ends(&mut routes, &ends, &none());
        let xs: Vec<f64> = routes.iter().flatten().map(|r| r[1].0).collect();
        assert_eq!(xs, vec![-21.0, -10.5, 0.0, 10.5, 21.0]);
    }

    #[test]
    fn snap_slivers_removes_sub_pixel_backtracks() {
        let out = snap_slivers(vec![
            (0.0, 0.0),
            (0.0, 918.329),
            (0.0, 918.274),
            (5.0, 950.0),
        ]);
        assert_eq!(
            out,
            vec![(0.0, 0.0), (0.0, 918.329), (0.0, 918.329), (5.0, 950.0)]
        );
    }

    #[test]
    fn ease_ends_departs_vertically() {
        let out = ease_ends(&[(0.0, 0.0), (100.0, 100.0)]);
        assert_eq!(
            out,
            vec![(0.0, 0.0), (0.0, 50.0), (100.0, 50.0), (100.0, 100.0)]
        );
        // Already vertical ends are left alone.
        let straight = [(0.0, 0.0), (0.0, 100.0)];
        assert_eq!(ease_ends(&straight), straight.to_vec());
    }
}
