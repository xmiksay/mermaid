//! d3-shape curve ports: cubic B-spline, linear, and orthogonal step paths.

use std::fmt::Write as _;

use super::fnum;

/// Faithful port of d3-shape `curveBasis` (open cubic B-spline) to an SVG path.
///
/// Endpoints are exact: the path starts at `pts[0]` and ends at `pts[n-1]`, so
/// node-boundary clipping and arrow markers still line up.
///   `0` pts → `""`            `1` pt → `"M x y"`
///   `2` pts → `"M x0 y0L x1 y1"` (straight; short edges stay straight)
///   `≥3`    → moveTo(p0); lineTo((5p0+p1)/6); a bezier per interior point;
///             a closing bezier; a final lineTo(last point).
pub(crate) fn curve_basis_path(pts: &[(f64, f64)]) -> String {
    let n = pts.len();
    let mut s = String::new();
    if n == 0 {
        return s;
    }
    if n == 1 {
        let _ = write!(s, "M{} {}", fnum(pts[0].0), fnum(pts[0].1));
        return s;
    }
    if n == 2 {
        let _ = write!(
            s,
            "M{} {}L{} {}",
            fnum(pts[0].0),
            fnum(pts[0].1),
            fnum(pts[1].0),
            fnum(pts[1].1),
        );
        return s;
    }

    fn bezier(s: &mut String, x0: f64, y0: f64, x1: f64, y1: f64, x: f64, y: f64) {
        let _ = write!(
            s,
            "C{} {} {} {} {} {}",
            fnum((2.0 * x0 + x1) / 3.0),
            fnum((2.0 * y0 + y1) / 3.0),
            fnum((x0 + 2.0 * x1) / 3.0),
            fnum((y0 + 2.0 * y1) / 3.0),
            fnum((x0 + 4.0 * x1 + x) / 6.0),
            fnum((y0 + 4.0 * y1 + y) / 6.0),
        );
    }

    let (mut x0, mut y0) = (f64::NAN, f64::NAN);
    let (mut x1, mut y1) = (f64::NAN, f64::NAN);

    for (i, &(x, y)) in pts.iter().enumerate() {
        match i {
            0 => {
                let _ = write!(s, "M{} {}", fnum(x), fnum(y));
            }
            1 => { /* d3 stores the point only; emits nothing */ }
            2 => {
                let _ = write!(
                    s,
                    "L{} {}",
                    fnum((5.0 * x0 + x1) / 6.0),
                    fnum((5.0 * y0 + y1) / 6.0)
                );
                bezier(&mut s, x0, y0, x1, y1, x, y);
            }
            _ => bezier(&mut s, x0, y0, x1, y1, x, y),
        }
        x0 = x1;
        y0 = y1;
        x1 = x;
        y1 = y;
    }

    // d3 Basis.lineEnd for an open curve with ≥3 points: one final bezier using
    // the incoming point = the last point, then a lineTo to the exact last
    // point. After the loop (x1,y1)=last and (x0,y0)=second-to-last.
    bezier(&mut s, x0, y0, x1, y1, x1, y1);
    let _ = write!(s, "L{} {}", fnum(x1), fnum(y1));

    s
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
