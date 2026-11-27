//! Node outlines (and marker discs) as bodies cheap to test a curve's
//! segments against.

use crate::parse::{EdgeCurve, NodeShape};
use crate::svg::builder::curve_basis_samples;

use super::super::edges::EndClip;
use super::super::round::is_round;
use super::super::route::Pt;

/// Steps per basis bezier when testing a curve: finer than the canvas
/// measurement, so a corner cut between two samples is not missed.
pub(super) const SAMPLES: usize = 16;

#[derive(Clone, Copy, PartialEq)]
enum Outline {
    Rect,
    Circle,
    Rhombus,
}

/// A node's outline in a form cheap to test segments against.
pub(super) struct Body {
    pub(super) c: Pt,
    hw: f64,
    hh: f64,
    outline: Outline,
    pub(super) bbox: [f64; 4],
}

impl Body {
    pub(super) fn new(node: &EndClip) -> Option<Self> {
        let (w, h) = node.size;
        let outline = match node.shape {
            Some(NodeShape::Rhombus) => Outline::Rhombus,
            Some(s) if is_round(s) => Outline::Circle,
            _ => Outline::Rect,
        };
        let (hw, hh) = if outline == Outline::Circle {
            (w.max(h) / 2.0, w.max(h) / 2.0)
        } else {
            (w / 2.0, h / 2.0)
        };
        let c = node.center;
        let bbox = [c.0 - hw, c.1 - hh, c.0 + hw, c.1 + hh];
        (hw > 0.0 && hh > 0.0 && bbox.iter().all(|v| v.is_finite())).then_some(Self {
            c,
            hw,
            hh,
            outline,
            bbox,
        })
    }

    /// How far the outline reaches from the centre along unit `dir`.
    pub(super) fn support(&self, dir: Pt) -> f64 {
        let (x, y) = (dir.0.abs() * self.hw, dir.1.abs() * self.hh);
        match self.outline {
            Outline::Rect => x + y,
            Outline::Circle => self.hw,
            Outline::Rhombus => x.max(y),
        }
    }

    pub(super) fn overlaps(&self, b: &[f64; 4]) -> bool {
        b[0] < self.bbox[2] && b[2] > self.bbox[0] && b[1] < self.bbox[3] && b[3] > self.bbox[1]
    }

    /// Whether segment `a`–`b` runs strictly inside the outline (touching
    /// it is fine). Non-finite input never hits: every comparison fails.
    pub(super) fn cuts(&self, a: Pt, b: Pt) -> bool {
        if !self.overlaps(&[a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)]) {
            return false;
        }
        let a = (a.0 - self.c.0, a.1 - self.c.1);
        let d = (b.0 - self.c.0 - a.0, b.1 - self.c.1 - a.1);
        let (hw, hh) = (self.hw, self.hh);
        match self.outline {
            Outline::Circle => {
                let len2 = d.0 * d.0 + d.1 * d.1;
                let t = if len2 > 0.0 {
                    (-(a.0 * d.0 + a.1 * d.1) / len2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                (a.0 + t * d.0).hypot(a.1 + t * d.1) < hw
            }
            Outline::Rect => clips(
                &[
                    (1.0, 0.0, hw),
                    (-1.0, 0.0, hw),
                    (0.0, 1.0, hh),
                    (0.0, -1.0, hh),
                ],
                a,
                d,
            ),
            Outline::Rhombus => {
                let o = hw * hh;
                clips(
                    &[(hh, hw, o), (-hh, hw, o), (hh, -hw, o), (-hh, -hw, o)],
                    a,
                    d,
                )
            }
        }
    }
}

/// Cyrus–Beck: whether `a + t·d`, t ∈ [0, 1], runs inside every half-plane
/// `n·p < o` for a stretch longer than a hair.
fn clips(planes: &[(f64, f64, f64)], a: Pt, d: Pt) -> bool {
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for &(nx, ny, o) in planes {
        let num = o - (nx * a.0 + ny * a.1);
        let den = nx * d.0 + ny * d.1;
        if den == 0.0 {
            if num <= 0.0 {
                return false;
            }
        } else if den > 0.0 {
            t1 = t1.min(num / den);
        } else {
            t0 = t0.max(num / den);
        }
    }
    (t1 - t0) * d.0.hypot(d.1) > 1e-3
}

pub(super) fn unit(a: Pt, b: Pt) -> Option<Pt> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = dx.hypot(dy);
    (l > 1e-9 && l.is_finite()).then(|| (dx / l, dy / l))
}

pub(super) fn dot(a: Pt, b: Pt) -> f64 {
    a.0 * b.0 + a.1 * b.1
}

pub(super) fn dist(a: Pt, b: Pt) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// The points an edge's drawn curve passes through.
pub(super) fn drawn(ctrl: &[Pt], curve: EdgeCurve) -> Vec<Pt> {
    match curve {
        EdgeCurve::Linear => ctrl.to_vec(),
        EdgeCurve::Step => {
            // As `curve_step_path`: each segment turns at its mid-x.
            let mut out: Vec<Pt> = ctrl.iter().take(1).copied().collect();
            for w in ctrl.windows(2) {
                let mx = (w[0].0 + w[1].0) / 2.0;
                out.extend([(mx, w[0].1), (mx, w[1].1), w[1]]);
            }
            out
        }
        _ => curve_basis_samples(ctrl, SAMPLES),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(center: Pt, size: (f64, f64), shape: NodeShape) -> EndClip {
        EndClip {
            center,
            size,
            shape: Some(shape),
        }
    }

    fn rect(center: Pt) -> EndClip {
        clip(center, (60.0, 40.0), NodeShape::Rect)
    }

    #[test]
    fn segment_cuts_each_outline_only_through_its_inside() {
        let r = Body::new(&rect((0.0, 0.0))).expect("rect");
        assert!(r.cuts((-100.0, 0.0), (100.0, 0.0)));
        assert!(!r.cuts((-100.0, 20.0), (100.0, 20.0)), "along the face");
        assert!(!r.cuts((-100.0, 30.0), (100.0, 30.0)));
        let c = Body::new(&clip((0.0, 0.0), (40.0, 40.0), NodeShape::Circle)).expect("circle");
        assert!(c.cuts((-100.0, 19.0), (100.0, 19.0)));
        assert!(!c.cuts((-100.0, 20.0), (100.0, 20.0)), "tangent");
        let d = Body::new(&clip((0.0, 0.0), (80.0, 40.0), NodeShape::Rhombus)).expect("rhombus");
        assert!(d.cuts((-100.0, 0.0), (100.0, 0.0)));
        // The rect's corner region lies outside the rhombus.
        assert!(!d.cuts((25.0, -100.0), (25.0, -10.0)));
        assert!(!r.cuts((f64::NAN, 0.0), (100.0, 0.0)));
        assert!(Body::new(&clip((f64::INFINITY, 0.0), (10.0, 10.0), NodeShape::Rect)).is_none());
    }
}
