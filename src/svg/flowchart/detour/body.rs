//! Node outlines (and marker discs) as bodies cheap to test a curve's
//! segments against.

use crate::parse::{EdgeCurve, NodeShape};
use crate::svg::builder::curve_basis_samples_by;

use super::super::edges::EndClip;
use super::super::round::is_round;
use super::super::route::Pt;
use super::scene::bbox;

/// The fewest steps per basis bezier when testing a curve, as many as the
/// canvas measurement takes.
pub(super) const SAMPLES: usize = 8;
/// The most steps per basis bezier.
const MAX_SAMPLES: usize = 64;
/// The longest chord between two samples of a long bezier.
const STEP: f64 = 8.0;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Outline {
    Rect,
    Circle,
    Rhombus,
}

/// A node's outline in a form cheap to test segments against.
#[derive(Clone, Copy, Debug)]
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

    /// The outline grown by `m` on every side.
    pub(super) fn grown(&self, m: f64) -> Self {
        // A rhombus scales about its centre so each face moves out by `m`.
        let k = 1.0 + m * (1.0 / (self.hw * self.hw) + 1.0 / (self.hh * self.hh)).sqrt();
        let (hw, hh) = match self.outline {
            Outline::Rhombus => (self.hw * k, self.hh * k),
            _ => (self.hw + m, self.hh + m),
        };
        let c = self.c;
        Self {
            hw,
            hh,
            bbox: [c.0 - hw, c.1 - hh, c.0 + hw, c.1 + hh],
            ..*self
        }
    }

    /// The outline's corners, each with the way out of it (a circle's are
    /// those of the octagon around it).
    pub(super) fn corners(&self) -> Vec<(Pt, Pt)> {
        if self.outline == Outline::Circle {
            // The octagon around it, each corner leaving radially.
            let r = self.hw / (std::f64::consts::PI / 8.0).cos();
            return (0..8)
                .map(|k| {
                    let a = f64::from(k) * std::f64::consts::PI / 4.0;
                    let u = (a.cos(), a.sin());
                    ((self.c.0 + r * u.0, self.c.1 + r * u.1), u)
                })
                .collect();
        }
        self.polygon()
            .into_iter()
            .map(|v| {
                let out = ((v.0 - self.c.0).signum(), (v.1 - self.c.1).signum());
                let snap = |o: f64, d: f64| if d.abs() < 1e-9 { 0.0 } else { o };
                (
                    v,
                    (snap(out.0, v.0 - self.c.0), snap(out.1, v.1 - self.c.1)),
                )
            })
            .collect()
    }

    /// The outline as a polygon (a circle as the 16-gon around it).
    fn polygon(&self) -> Vec<Pt> {
        let (c, hw, hh) = (self.c, self.hw, self.hh);
        match self.outline {
            Outline::Rect => vec![
                (c.0 - hw, c.1 - hh),
                (c.0 + hw, c.1 - hh),
                (c.0 + hw, c.1 + hh),
                (c.0 - hw, c.1 + hh),
            ],
            Outline::Rhombus => vec![
                (c.0, c.1 - hh),
                (c.0 + hw, c.1),
                (c.0, c.1 + hh),
                (c.0 - hw, c.1),
            ],
            Outline::Circle => {
                let r = hw / (std::f64::consts::PI / 16.0).cos();
                (0..16)
                    .map(|k| {
                        let a = f64::from(k) * std::f64::consts::PI / 8.0;
                        (c.0 + r * a.cos(), c.1 + r * a.sin())
                    })
                    .collect()
            }
        }
    }

    /// How far the outline reaches past the line through `mid` along unit
    /// `d`, toward unit normal `n`, within the stretch `±w` of `mid` along
    /// `d` — how far a curve crossing there has to move toward `n` to pass
    /// it. `None` when the outline misses that stretch.
    pub(super) fn reach(&self, mid: Pt, d: Pt, n: Pt, w: f64) -> Option<f64> {
        let rel = |p: Pt| (p.0 - mid.0, p.1 - mid.1);
        let mut poly = self.polygon();
        // Sutherland–Hodgman against `s·along ≤ w` for both signs.
        for s in [1.0, -1.0] {
            let f = |p: Pt| s * dot(rel(p), d) - w;
            let mut out = Vec::with_capacity(poly.len() + 2);
            for (k, &a) in poly.iter().enumerate() {
                let b = poly.get(k + 1).or(poly.first()).copied().unwrap_or(a);
                let (fa, fb) = (f(a), f(b));
                if fa <= 0.0 {
                    out.push(a);
                }
                if (fa < 0.0) != (fb < 0.0) && fa != fb {
                    let t = fa / (fa - fb);
                    out.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
                }
            }
            poly = out;
            if poly.is_empty() {
                return None;
            }
        }
        poly.iter().map(|&p| dot(rel(p), n)).reduce(f64::max)
    }

    pub(super) fn overlaps(&self, b: &[f64; 4]) -> bool {
        b[0] < self.bbox[2] && b[2] > self.bbox[0] && b[1] < self.bbox[3] && b[3] > self.bbox[1]
    }

    /// Whether `p` lies inside the outline.
    pub(super) fn contains(&self, p: Pt) -> bool {
        let (dx, dy) = ((p.0 - self.c.0).abs(), (p.1 - self.c.1).abs());
        match self.outline {
            Outline::Rect => dx < self.hw && dy < self.hh,
            Outline::Circle => dx.hypot(dy) < self.hw,
            Outline::Rhombus => dx / self.hw + dy / self.hh < 1.0,
        }
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

/// The points an edge's drawn curve passes through: on a basis curve, a
/// bezier that may come near something (`near`, given its control box) gets
/// enough samples that no chord is longer than `STEP` (so a cut of a node's
/// corner cannot hide between samples on a long sweep), any other `SAMPLES`;
/// each bezier sampled on its own so the stretch a detour leaves alone
/// samples the same.
pub(super) fn drawn(ctrl: &[Pt], curve: EdgeCurve, near: &dyn Fn(&[f64; 4]) -> bool) -> Vec<Pt> {
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
        _ => curve_basis_samples_by(ctrl, |c| {
            // A bezier stays inside its control points' box.
            if !near(&bbox(c)) {
                return SAMPLES;
            }
            // The control polygon is never shorter than the curve.
            let span: f64 = c.windows(2).map(|w| dist(w[0], w[1])).sum();
            ((span / STEP).ceil() as usize).clamp(SAMPLES, MAX_SAMPLES)
        }),
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

    #[test]
    fn a_grown_rhombus_moves_each_face_out_by_the_margin() {
        let d = Body::new(&clip((0.0, 0.0), (80.0, 40.0), NodeShape::Rhombus)).expect("rhombus");
        let g = d.grown(6.0);
        // The outward unit normal of the upper-right face.
        let n = (20.0 / 40.0_f64.hypot(20.0), 40.0 / 40.0_f64.hypot(20.0));
        // The face's midpoint (20, -10) on the original, pushed out along it.
        let out = |k: f64| (20.0 + n.0 * k, -10.0 - n.1 * k);
        assert!(g.contains(out(5.9)));
        assert!(!g.contains(out(6.1)));
    }

    #[test]
    fn a_circle_has_the_corners_of_the_octagon_round_it() {
        let c = Body::new(&clip((0.0, 0.0), (40.0, 40.0), NodeShape::Circle)).expect("circle");
        let corners = c.corners();
        assert_eq!(corners.len(), 8);
        for (v, out) in corners {
            // Outside the circle, leaving radially.
            assert!(dist(v, (0.0, 0.0)) > 20.0 && !c.contains(v));
            assert!((out.0 * v.1 - out.1 * v.0).abs() < 1e-9);
        }
    }

    #[test]
    fn a_bezier_near_nothing_is_sampled_coarsely() {
        let ctrl = [(0.0, 0.0), (400.0, 0.0), (800.0, 0.0), (1200.0, 0.0)];
        let fine = drawn(&ctrl, EdgeCurve::Basis, &|_| true);
        let coarse = drawn(&ctrl, EdgeCurve::Basis, &|_| false);
        assert!(coarse.len() < fine.len());
        // The same curve: ends and every coarse sample's bezier ends agree.
        assert_eq!(coarse.first(), fine.first());
        assert_eq!(coarse.last(), fine.last());
    }
}
