//! `)cloud(` and `))bang((` outlines. Both hang off one closed stadium (pill)
//! whose caps pass through the label box's corners, so the vertex polygon —
//! and therefore the bumpy / jagged outline built outward from it — always
//! encloses the label and icon. Layout reads [`extent`] so the canvas and the
//! sibling spacing account for the bumps and spikes past the box.

use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::fmt::Write as _;

use crate::parse::MindmapShape;
use crate::svg::builder::fnum;

/// Cloud bump / bang valley pitch, in units of the clamped half-height, so a
/// bump keeps one size whatever the label width.
const CLOUD_PITCH: f64 = 1.5;
const BANG_PITCH: f64 = 1.8;
/// Cloud bump arc radius as a fraction of its chord (just over a semicircle's
/// 0.5, giving a pronounced but not round bump).
const BUMP_RADIUS: f64 = 0.6;
/// How far past its chord a bump bulges: `r - sqrt(r² - (chord/2)²)` per chord.
const BUMP_BULGE: f64 = 0.2683;
/// Bang spike length past the valley ring, in clamped half-height units.
const SPIKE: f64 = 0.6;

/// Half-height clamped so a tall multi-line node does not grow giant bumps and
/// a degenerate one still gets a usable pitch.
fn unit(hh: f64) -> f64 {
    hh.clamp(4.0, 24.0)
}

/// Vertices spaced evenly (at most `pitch` apart) along a stadium around the
/// `hw` × `hh` box centered on `(cx, cy)`.
struct Ring {
    cx: f64,
    cy: f64,
    /// Half-length of the straight top/bottom runs.
    c: f64,
    /// Cap radius.
    a: f64,
    per: f64,
    n: usize,
}

impl Ring {
    fn new(cx: f64, cy: f64, hw: f64, hh: f64, pitch: f64) -> Self {
        let c = (hw - hh).max(0.0);
        // A chord of length ≤ pitch on a circle of radius hypot(r, pitch/2)
        // stays ≥ r from its centre, so padding the corner-touching radius this
        // way keeps every polygon edge outside the box.
        let a = (hw - c).hypot(hh).hypot(pitch / 2.0);
        let per = 4.0 * c + TAU * a;
        let n = ((per / pitch).ceil() as usize).max(8);
        Ring {
            cx,
            cy,
            c,
            a,
            per,
            n,
        }
    }

    /// Point at fraction `k / n` of the perimeter (clockwise from the top-left
    /// end of the top run) and its outward unit normal.
    fn at(&self, k: f64) -> ((f64, f64), (f64, f64)) {
        let (c, a) = (self.c, self.a);
        let mut d = self.per * (k / self.n as f64).rem_euclid(1.0);
        let (x, y, nx, ny) = 'seg: {
            if d < 2.0 * c {
                break 'seg (d - c, -a, 0.0, -1.0);
            }
            d -= 2.0 * c;
            if d < PI * a {
                let t = -FRAC_PI_2 + d / a;
                break 'seg (c + a * t.cos(), a * t.sin(), t.cos(), t.sin());
            }
            d -= PI * a;
            if d < 2.0 * c {
                break 'seg (c - d, a, 0.0, 1.0);
            }
            let t = FRAC_PI_2 + (d - 2.0 * c) / a;
            (-c + a * t.cos(), a * t.sin(), t.cos(), t.sin())
        };
        ((self.cx + x, self.cy + y), (nx, ny))
    }
}

/// Bumpy cloud: outward arcs between consecutive ring vertices.
pub(super) fn cloud_path(cx: f64, cy: f64, hw: f64, hh: f64) -> String {
    let ring = Ring::new(cx, cy, hw, hh, unit(hh) * CLOUD_PITCH);
    let (mut px, mut py) = ring.at(0.0).0;
    let mut d = format!("M{} {}", fnum(px), fnum(py));
    for k in 1..=ring.n {
        let (qx, qy) = ring.at(k as f64).0;
        let r = fnum(((qx - px).hypot(qy - py) * BUMP_RADIUS).max(1.0));
        let _ = write!(d, "A{r} {r} 0 0 1 {} {}", fnum(qx), fnum(qy));
        (px, py) = (qx, qy);
    }
    d.push('Z');
    d
}

/// Jagged burst: valleys on the ring vertices, a spike pushed outward halfway
/// between each pair.
pub(super) fn bang_path(cx: f64, cy: f64, hw: f64, hh: f64) -> String {
    let ring = Ring::new(cx, cy, hw, hh, unit(hh) * BANG_PITCH);
    let spike = unit(hh) * SPIKE;
    let mut d = String::new();
    for k in 0..ring.n {
        let (vx, vy) = ring.at(k as f64).0;
        let ((sx, sy), (nx, ny)) = ring.at(k as f64 + 0.5);
        let cmd = if k == 0 { 'M' } else { 'L' };
        let _ = write!(
            d,
            "{cmd}{} {}L{} {}",
            fnum(vx),
            fnum(vy),
            fnum(sx + nx * spike),
            fnum(sy + ny * spike)
        );
    }
    d.push('Z');
    d
}

/// Half-extents `(x, y)` the cloud / bang outline reaches from the node
/// centre, or `None` for shapes drawn within their box.
pub(super) fn extent(shape: MindmapShape, hw: f64, hh: f64) -> Option<(f64, f64)> {
    let (pitch, past) = match shape {
        MindmapShape::Cloud => {
            let p = unit(hh) * CLOUD_PITCH;
            (p, p * BUMP_BULGE)
        }
        MindmapShape::Bang => (unit(hh) * BANG_PITCH, unit(hh) * SPIKE),
        _ => return None,
    };
    let ring = Ring::new(0.0, 0.0, hw, hh, pitch);
    let r = ring.a + past;
    Some((ring.c + r, r))
}
