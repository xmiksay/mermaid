//! Stitching a relation's per-cluster route pieces into one polyline.

use std::collections::HashMap;

use super::super::outline::Span;
use super::frames::{avoid_header, drop_straight, push_stub, until_exit};
use super::{add, Local, Pt, RelPath};

/// The relation's route in root top-down space, from its source centre to its
/// target centre. Each boundary level contributes its piece between the
/// member and the frame edge (where the piece leaves towards its port); the
/// common level contributes the piece between the two outermost items.
/// `marks` keeps different routes' frame crossings and header detours apart.
pub(super) fn assemble(
    path: &RelPath,
    rel: usize,
    locals: &[Local],
    offset: &[Pt],
    lr: bool,
    marks: &mut FrameMarks,
) -> Option<Vec<Pt>> {
    let ctx = Stitch {
        rel,
        locals,
        offset,
        lr,
    };
    let src = ctx.side(&path.src_side, false, marks)?;
    let dst = ctx.side(&path.dst_side, true, marks)?;
    let mut middle = ctx.piece(path.common)?;
    // The outermost member pieces already left their boundaries; the common
    // piece starts and ends at those boundaries' centres instead.
    if let Some(&exit) = src.last().and_then(|p| p.last()) {
        let centre = middle.remove(0);
        let toward = middle.first().copied().unwrap_or(exit);
        let bridge = ctx.bridge(path.common, centre, exit, toward);
        middle.splice(0..0, bridge);
    }
    if let Some(&entry) = dst.last().and_then(|p| p.last()) {
        let centre = middle.pop().unwrap_or(entry);
        let toward = middle.last().copied().unwrap_or(entry);
        middle.extend(
            ctx.bridge(path.common, centre, entry, toward)
                .into_iter()
                .rev(),
        );
    }
    let mut route: Vec<Pt> = src.into_iter().flatten().collect();
    route.extend(middle);
    for mut part in dst.into_iter().rev() {
        part.reverse();
        route.extend(part);
    }
    (route.len() >= 2).then_some(route)
}

/// What earlier routes already used at each frame: detour slots, and the
/// spots where they cross the top / bottom edge.
#[derive(Default)]
pub(super) struct FrameMarks {
    detours: HashMap<usize, usize>,
    exits: HashMap<(usize, bool), Vec<f64>>,
}

/// Least distance between two routes crossing one frame edge.
const EXIT_GAP: f64 = 12.0;

impl FrameMarks {
    /// Keep this piece's crossing of cluster `c`'s top or bottom edge at
    /// least `EXIT_GAP` from every earlier one, sliding it along the edge.
    fn claim_exit(&mut self, c: usize, part: &mut [Pt], frame: Span) {
        let Some(&exit) = part.last() else { return };
        let top = (exit.1 - frame.y0).abs() < 0.5;
        if !top && (exit.1 - frame.y1).abs() > 0.5 {
            return;
        }
        let used = self.exits.entry((c, top)).or_default();
        let free = |x: f64| used.iter().all(|u| (u - x).abs() >= EXIT_GAP);
        let inside = |x: f64| x >= frame.x0 + EXIT_GAP / 2.0 && x <= frame.x1 - EXIT_GAP / 2.0;
        let x = (0..64)
            .map(|k| {
                exit.0 + (k as f64 / 2.0).ceil() * EXIT_GAP * if k % 2 == 1 { 1.0 } else { -1.0 }
            })
            .find(|&x| inside(x) && free(x))
            .unwrap_or(exit.0);
        // Slide the exit and every trailing point close to the edge with it,
        // so the route still meets the edge square-on instead of sliding
        // along it.
        for p in part
            .iter_mut()
            .rev()
            .take_while(|p| (p.1 - exit.1).abs() < EXIT_GAP)
        {
            p.0 = x;
        }
        used.push(x);
    }
}

struct Stitch<'a> {
    rel: usize,
    locals: &'a [Local],
    offset: &'a [Pt],
    lr: bool,
}

impl Stitch<'_> {
    /// Cluster `c`'s piece of the relation, in root space.
    fn piece(&self, c: usize) -> Option<Vec<Pt>> {
        let pts = self.locals[c].rel_pts.get(&self.rel)?;
        Some(pts.iter().map(|&p| add(p, self.offset[c])).collect())
    }

    fn shift(&self, b: Span, c: usize) -> Span {
        b.shift(self.offset[c])
    }

    /// One endpoint's boundary pieces, innermost first, each running inside
    /// out — from the member (or the inner boundary's exit) to just outside
    /// its frame. `incoming` pieces are stored target-ward and get flipped.
    fn side(
        &self,
        chain: &[usize],
        incoming: bool,
        marks: &mut FrameMarks,
    ) -> Option<Vec<Vec<Pt>>> {
        let mut parts: Vec<Vec<Pt>> = Vec::new();
        for (k, &c) in chain.iter().enumerate() {
            let mut io = self.piece(c)?;
            if incoming {
                io.reverse();
            }
            let inner_exit = parts.last().and_then(|p| p.last()).copied();
            if let (true, Some(exit)) = (k > 0, inner_exit) {
                // This level starts at the inner boundary's centre: continue
                // from that boundary's exit instead, straight out of its band.
                // The exit seeds the cut (it lies inside this frame) and is
                // dropped again, as the inner part already ends there.
                let centre = io.remove(0);
                let toward = io.first().copied().unwrap_or(exit);
                let mut lead = vec![exit];
                lead.extend(self.bridge(c, centre, exit, toward));
                io.splice(0..0, lead);
            }
            let frame = self.shift(self.locals[c].frame, c);
            let mut part = until_exit(&io, frame);
            if let Some(exit) = inner_exit {
                drop_straight(&mut part, exit.0, frame);
            }
            marks.claim_exit(c, &mut part, frame);
            if let Some(h) = self.locals[c].header {
                let slot = marks.detours.entry(c).or_insert(0);
                if avoid_header(&mut part, self.shift(h, c), self.lr, *slot) {
                    *slot += 1;
                }
            }
            push_stub(&mut part, frame, 0.0);
            if k > 0 && inner_exit.is_some() && !part.is_empty() {
                part.remove(0);
            }
            parts.push(part);
        }
        Some(parts)
    }

    /// Points taking a route that leaves a boundary block (centred at
    /// `centre` in cluster `c`'s layout) at `exit` straight out of that
    /// block's layer band before it turns towards `toward` — a frame
    /// shorter than its band would otherwise let the route cut diagonally
    /// across the frames beside it.
    fn bridge(&self, c: usize, centre: Pt, exit: Pt, toward: Pt) -> Vec<Pt> {
        let local = &self.locals[c];
        let oy = self.offset[c].1;
        let Some((cy, half)) = band_at(&local.centers, &local.sizes, centre.1 - oy) else {
            return Vec::new();
        };
        let down = toward.1 >= centre.1;
        let edge = cy + oy + if down { half } else { -half };
        let beyond = if down {
            edge > exit.1 + 0.5
        } else {
            edge < exit.1 - 0.5
        };
        if beyond {
            let e = (exit.0, edge);
            vec![e, depart(e, toward)]
        } else {
            Vec::new()
        }
    }
}

/// The layer band `(centre y, half height)` that `y` falls in, if any.
/// Matched by containment rather than by centre: `bow_opposite_pairs`
/// nudges the dummies of a reversed pair slightly off their layer.
fn band_at(centers: &[Pt], sizes: &[Pt], y: f64) -> Option<(f64, f64)> {
    let (&(_, cy), _) = centers
        .iter()
        .zip(sizes)
        .find(|(c, s)| (c.1 - y).abs() < s.1 / 2.0)?;
    let half = centers
        .iter()
        .zip(sizes)
        .filter(|(c, _)| (c.1 - cy).abs() < 0.5)
        .map(|(_, s)| s.1 / 2.0)
        .fold(0.0, f64::max);
    Some((cy, half))
}

/// Replace every interior waypoint lying on a layer (a dummy node) with a
/// vertical pass through that layer's band, so the spline crosses a layer
/// straight down between its shapes and only bends in the gaps — otherwise
/// it cuts the corner between two dummies across a neighbouring shape.
///
/// The band edges (and frame crossings, see [`until_exit`]) are doubled: a
/// uniform B-spline passes a single control point at `(prev + 4·p + next)/6`,
/// which after a long sideways run lands deep inside the band; a doubled
/// point pulls the curve to within a few pixels of it. An extra point just
/// outside each band edge finishes the sideways run in the gap, so what
/// little pull the far neighbour still has never reaches into the band.
pub(super) fn through_bands(centers: &[Pt], sizes: &[Pt], pts: &[Pt]) -> Vec<Pt> {
    const APPROACH_SHARE: f64 = 0.3;
    const APPROACH_MAX: f64 = 24.0;
    let n = pts.len();
    let mut out = Vec::with_capacity(n * 3);
    for (k, &p) in pts.iter().enumerate() {
        let band = if k == 0 || k + 1 == n {
            None
        } else {
            band_at(centers, sizes, p.1)
        };
        let Some((cy, half)) = band else {
            out.push(p);
            continue;
        };
        let down = if pts[k + 1].1 >= pts[k - 1].1 {
            1.0
        } else {
            -1.0
        };
        let (enter, leave) = (cy - down * half, cy + down * half);
        let approach = ((enter - pts[k - 1].1).abs() * APPROACH_SHARE).min(APPROACH_MAX);
        let depart = ((pts[k + 1].1 - leave).abs() * APPROACH_SHARE).min(APPROACH_MAX);
        out.extend([
            (p.0, enter - down * approach),
            (p.0, enter),
            (p.0, enter),
            (p.0, leave),
            (p.0, leave),
            (p.0, leave + down * depart),
        ]);
    }
    out
}

/// Make a piece that starts or ends on a shape (`leaf[i]`, not a boundary
/// block, whose own exit lies on its frame) run straight out of that
/// shape's whole layer band before it bends: a short shape in a band made
/// tall by a neighbouring boundary would otherwise bend inside the band and
/// cut across that boundary.
pub(super) fn leave_bands(centers: &[Pt], sizes: &[Pt], leaf: &[bool], pts: &[Pt]) -> Vec<Pt> {
    let n = pts.len();
    if n < 2 {
        return pts.to_vec();
    }
    let on_leaf = |p: Pt| {
        centers
            .iter()
            .zip(leaf)
            .any(|(c, &l)| l && (c.0 - p.0).abs() < 0.5 && (c.1 - p.1).abs() < 0.5)
    };
    let edge_of = |p: Pt, toward: Pt| {
        let (cy, half) = band_at(centers, sizes, p.1)?;
        let sign = if toward.1 >= p.1 { 1.0 } else { -1.0 };
        Some((p.0, cy + sign * half))
    };
    let mut out = Vec::with_capacity(n + 4);
    out.push(pts[0]);
    if on_leaf(pts[0]) {
        if let Some(e) = edge_of(pts[0], pts[1]) {
            out.extend([e, depart(e, pts[1])]);
        }
    }
    out.extend_from_slice(&pts[1..n - 1]);
    if on_leaf(pts[n - 1]) {
        if let Some(e) = edge_of(pts[n - 1], pts[n - 2]) {
            out.extend([depart(e, pts[n - 2]), e]);
        }
    }
    out.push(pts[n - 1]);
    out
}

/// A point continuing straight on from the band edge `e` towards `toward`'s
/// layer: the centre, `e` and this point are collinear, so the spline runs
/// straight out of the band, then bends smoothly (an S, never a straight
/// diagonal) in the gap.
fn depart(e: Pt, toward: Pt) -> Pt {
    // Short enough that the middle of the gap stays free for staggered
    // tracks (`ends::spread_ends`).
    const SHARE: f64 = 0.25;
    const MAX: f64 = 20.0;
    let dy = toward.1 - e.1;
    (e.0, e.1 + dy.signum() * (dy.abs() * SHARE).min(MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn through_bands_passes_layers_vertically() {
        // Items on layers y=0 (h 40) and y=100 (h 60); a route with a dummy
        // at (50, 100) on the second layer.
        let centers = [(0.0, 0.0), (200.0, 100.0)];
        let sizes = [(40.0, 40.0), (40.0, 60.0)];
        let out = through_bands(&centers, &sizes, &[(0.0, 0.0), (50.0, 100.0), (0.0, 200.0)]);
        assert_eq!(out.first(), Some(&(0.0, 0.0)));
        assert_eq!(out.last(), Some(&(0.0, 200.0)));
        // The dummy became a vertical run over the band 70..130, doubled at
        // its edges, with approach/depart points outside it.
        let inner: Vec<Pt> = out[1..out.len() - 1].to_vec();
        assert!(inner.iter().all(|p| p.0 == 50.0));
        assert_eq!(inner[1], (50.0, 70.0));
        assert_eq!(inner[2], (50.0, 70.0));
        assert_eq!(inner[3], (50.0, 130.0));
        assert!(inner[0].1 < 70.0 && inner[5].1 > 130.0);
        // A point in a gap (no band) is kept as is.
        let gap = through_bands(&centers, &sizes, &[(0.0, 0.0), (9.0, 50.0), (0.0, 200.0)]);
        assert_eq!(gap, vec![(0.0, 0.0), (9.0, 50.0), (0.0, 200.0)]);
    }

    #[test]
    fn leave_bands_exits_the_layer_before_bending() {
        // A short shape at (0, 0) shares its layer with a 200-tall block, so
        // the band reaches y = 100; the route leaves vertically to there.
        let centers = [(0.0, 0.0), (300.0, 0.0), (100.0, 200.0)];
        let sizes = [(40.0, 40.0), (100.0, 200.0), (40.0, 40.0)];
        let leaf = [true, false, true];
        let out = leave_bands(&centers, &sizes, &leaf, &[(0.0, 0.0), (100.0, 200.0)]);
        assert_eq!(out[1], (0.0, 100.0));
        assert_eq!(out[2], (0.0, 120.0), "then straight on before bending");
        assert_eq!(
            out[out.len() - 2],
            (100.0, 180.0),
            "enters its own band vertically"
        );
        // A block endpoint is left alone: its piece reaches its frame itself.
        let out = leave_bands(&centers, &sizes, &leaf, &[(300.0, 0.0), (100.0, 200.0)]);
        assert_eq!(
            out,
            vec![(300.0, 0.0), (100.0, 160.0), (100.0, 180.0), (100.0, 200.0)]
        );
    }
}
