//! d3-shape curve ports: cubic B-spline, linear, and orthogonal step paths.

use std::fmt::Write as _;

use super::fnum;

/// One drawing command of a d3 `curveBasis` path.
enum BasisSeg {
    Move((f64, f64)),
    Line((f64, f64)),
    Cubic((f64, f64), (f64, f64), (f64, f64)),
}

/// Faithful port of d3-shape `curveBasis` (open cubic B-spline) as a command
/// list. Endpoints are exact: the path starts at `pts[0]` and ends at
/// `pts[n-1]`, so node-boundary clipping and arrow markers still line up.
///   `0` pts → nothing         `1` pt → a moveTo
///   `2` pts → moveTo + lineTo (straight; short edges stay straight)
///   `≥3`    → moveTo(p0); lineTo((5p0+p1)/6); a bezier per interior point;
///             a closing bezier; a final lineTo(last point).
fn basis_segments(pts: &[(f64, f64)]) -> Vec<BasisSeg> {
    let n = pts.len();
    let mut segs = Vec::with_capacity(n + 2);
    match n {
        0 => return segs,
        1 | 2 => {
            segs.push(BasisSeg::Move(pts[0]));
            if n == 2 {
                segs.push(BasisSeg::Line(pts[1]));
            }
            return segs;
        }
        _ => {}
    }

    fn bezier(p0: (f64, f64), p1: (f64, f64), p: (f64, f64)) -> BasisSeg {
        BasisSeg::Cubic(
            ((2.0 * p0.0 + p1.0) / 3.0, (2.0 * p0.1 + p1.1) / 3.0),
            ((p0.0 + 2.0 * p1.0) / 3.0, (p0.1 + 2.0 * p1.1) / 3.0),
            (
                (p0.0 + 4.0 * p1.0 + p.0) / 6.0,
                (p0.1 + 4.0 * p1.1 + p.1) / 6.0,
            ),
        )
    }

    let mut p0 = (f64::NAN, f64::NAN);
    let mut p1 = (f64::NAN, f64::NAN);
    for (i, &p) in pts.iter().enumerate() {
        match i {
            0 => segs.push(BasisSeg::Move(p)),
            1 => { /* d3 stores the point only; emits nothing */ }
            2 => {
                segs.push(BasisSeg::Line((
                    (5.0 * p0.0 + p1.0) / 6.0,
                    (5.0 * p0.1 + p1.1) / 6.0,
                )));
                segs.push(bezier(p0, p1, p));
            }
            _ => segs.push(bezier(p0, p1, p)),
        }
        p0 = p1;
        p1 = p;
    }

    // d3 Basis.lineEnd for an open curve with ≥3 points: one final bezier using
    // the incoming point = the last point, then a lineTo to the exact last
    // point. After the loop p1 = last and p0 = second-to-last.
    segs.push(bezier(p0, p1, p1));
    segs.push(BasisSeg::Line(p1));
    segs
}

/// [`basis_segments`] as an SVG path string.
pub(crate) fn curve_basis_path(pts: &[(f64, f64)]) -> String {
    let mut s = String::new();
    for seg in basis_segments(pts) {
        let _ = match seg {
            BasisSeg::Move((x, y)) => write!(s, "M{} {}", fnum(x), fnum(y)),
            BasisSeg::Line((x, y)) => write!(s, "L{} {}", fnum(x), fnum(y)),
            BasisSeg::Cubic(c1, c2, p) => write!(
                s,
                "C{} {} {} {} {} {}",
                fnum(c1.0),
                fnum(c1.1),
                fnum(c2.0),
                fnum(c2.1),
                fnum(p.0),
                fnum(p.1),
            ),
        };
    }
    s
}

/// The `curveBasis` curve through `pts` flattened to a polyline (each bezier
/// sampled at `BASIS_SAMPLES` steps) — what a renderer tests label placement
/// against, since the drawn curve departs from its control polygon.
pub(crate) fn curve_basis_points(pts: &[(f64, f64)]) -> Vec<(f64, f64)> {
    const BASIS_SAMPLES: usize = 8;
    let mut out: Vec<(f64, f64)> = Vec::new();
    for seg in basis_segments(pts) {
        match seg {
            BasisSeg::Move(p) | BasisSeg::Line(p) => out.push(p),
            BasisSeg::Cubic(c1, c2, p) => {
                let Some(&p0) = out.last() else { continue };
                for k in 1..=BASIS_SAMPLES {
                    let t = k as f64 / BASIS_SAMPLES as f64;
                    let u = 1.0 - t;
                    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    out.push((
                        a * p0.0 + b * c1.0 + c * c2.0 + d * p.0,
                        a * p0.1 + b * c1.1 + c * c2.1 + d * p.1,
                    ));
                }
            }
        }
    }
    out
}

/// Straight-segment path through the waypoints (d3 `curveLinear`): a `moveTo`
/// the first point, a `lineTo` each remaining one.
pub(crate) fn curve_linear_path(pts: &[(f64, f64)]) -> String {
    let mut s = String::new();
    for (i, &(x, y)) in pts.iter().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        let _ = write!(s, "{cmd}{} {}", fnum(x), fnum(y));
    }
    s
}

/// Orthogonal right-angle path through the waypoints (d3 `curveStep`, `t=0.5`):
/// each segment turns at the mid-x between its endpoints. A final `lineTo`
/// reaches the true last point so the arrow marker still lands on the node.
pub(crate) fn curve_step_path(pts: &[(f64, f64)]) -> String {
    let n = pts.len();
    let mut s = String::new();
    if n == 0 {
        return s;
    }
    let (mut px, mut py) = pts[0];
    let _ = write!(s, "M{} {}", fnum(px), fnum(py));
    for &(x, y) in &pts[1..] {
        let mx = (px + x) / 2.0;
        let _ = write!(s, "L{} {}L{} {}", fnum(mx), fnum(py), fnum(mx), fnum(y));
        px = x;
        py = y;
    }
    if n > 1 {
        let _ = write!(s, "L{} {}", fnum(px), fnum(py));
    }
    s
}
