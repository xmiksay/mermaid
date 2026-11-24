//! The obstacles a relation label must keep clear of, in a uniform grid
//! ([`Obstacles`]) so each candidate spot (`rel_place`) only tests what is
//! near it.

use std::collections::HashMap;

use super::super::label_layout::{rect_overlap_area, segment_rect_overlap};
use super::outline::Rect;

/// A line segment obstacle: a piece of another relation's drawn line or a
/// boundary frame's border.
pub(super) type Segment = ((f64, f64), (f64, f64));

/// An already-placed label is inflated
/// by this much before it becomes an obstacle, so the search stops at a
/// visible gap rather than the first merely-touching spot (#399).
pub(super) const LABEL_CLEARANCE: f64 = 6.0;

/// Shapes are inflated by this much so a label never sits flush on a border.
pub(super) const SHAPE_CLEARANCE: f64 = 3.0;

/// Grid cell size of the obstacle index.
const CELL: f64 = 64.0;

pub(super) fn inflate(rect: Rect, m: f64) -> Rect {
    (rect.0 - m, rect.1 - m, rect.2 + 2.0 * m, rect.3 + 2.0 * m)
}

/// Who a segment belongs to: a relation (by plan index) or a frame border.
pub(super) type Owner = Option<usize>;

/// Rects and owned segments bucketed in a uniform grid.
#[derive(Default)]
pub(super) struct Obstacles {
    rects: Vec<Rect>,
    rect_cells: HashMap<(i64, i64), Vec<usize>>,
    /// Rects a label avoids if it can, but covers before it covers a hard
    /// one: line ends near their arrowheads, boundary borders.
    firm: Vec<Rect>,
    firm_cells: HashMap<(i64, i64), Vec<usize>>,
    segs: Vec<(Segment, Owner)>,
    seg_cells: HashMap<(i64, i64), Vec<usize>>,
    /// Corners of everything added: `(x0, y0, x1, y1)`.
    extent: Option<(f64, f64, f64, f64)>,
}

fn cells(r: Rect) -> impl Iterator<Item = (i64, i64)> {
    let cx0 = (r.0 / CELL).floor() as i64;
    let cy0 = (r.1 / CELL).floor() as i64;
    let cx1 = ((r.0 + r.2) / CELL).floor() as i64;
    let cy1 = ((r.1 + r.3) / CELL).floor() as i64;
    (cx0..=cx1).flat_map(move |x| (cy0..=cy1).map(move |y| (x, y)))
}

fn seg_box((a, b): Segment) -> Rect {
    let (x0, y0) = (a.0.min(b.0), a.1.min(b.1));
    (x0, y0, (a.0 - b.0).abs(), (a.1 - b.1).abs())
}

impl Obstacles {
    fn grow(&mut self, r: Rect) {
        let (x0, y0, x1, y1) = (r.0, r.1, r.0 + r.2, r.1 + r.3);
        self.extent = Some(match self.extent {
            None => (x0, y0, x1, y1),
            Some(e) => (e.0.min(x0), e.1.min(y0), e.2.max(x1), e.3.max(y1)),
        });
    }

    /// The box around every obstacle added so far.
    pub(super) fn extent(&self) -> Option<(f64, f64, f64, f64)> {
        self.extent
    }

    pub(super) fn add_rect(&mut self, r: Rect) {
        self.grow(r);
        let i = self.rects.len();
        self.rects.push(r);
        for c in cells(r) {
            self.rect_cells.entry(c).or_default().push(i);
        }
    }

    pub(super) fn add_firm_rect(&mut self, r: Rect) {
        let i = self.firm.len();
        self.firm.push(r);
        for c in cells(r) {
            self.firm_cells.entry(c).or_default().push(i);
        }
    }

    pub(super) fn add_segment(&mut self, s: Segment, owner: Owner) {
        self.grow(seg_box(s));
        let i = self.segs.len();
        self.segs.push((s, owner));
        for c in cells(seg_box(s)) {
            self.seg_cells.entry(c).or_default().push(i);
        }
    }

    fn near(map: &HashMap<(i64, i64), Vec<usize>>, r: Rect) -> Vec<usize> {
        let mut out: Vec<usize> = cells(r)
            .filter_map(|c| map.get(&c))
            .flatten()
            .copied()
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Total area of `r` covered by obstacle rects.
    pub(super) fn covered(&self, r: Rect) -> f64 {
        Self::near(&self.rect_cells, r)
            .into_iter()
            .map(|i| rect_overlap_area(r, self.rects[i]))
            .sum()
    }

    /// Total area of `r` covered by firm rects.
    pub(super) fn firmly_covered(&self, r: Rect) -> f64 {
        Self::near(&self.firm_cells, r)
            .into_iter()
            .map(|i| rect_overlap_area(r, self.firm[i]))
            .sum()
    }

    /// Length of foreign line running through `r`.
    pub(super) fn crossed(&self, r: Rect, foreign: &dyn Fn(Owner) -> bool) -> f64 {
        Self::near(&self.seg_cells, r)
            .into_iter()
            .filter(|&i| foreign(self.segs[i].1))
            .map(|i| segment_rect_overlap(self.segs[i].0 .0, self.segs[i].0 .1, r))
            .sum()
    }

    /// Whether a foreign line passes within `d` of `p`.
    pub(super) fn foreign_within(
        &self,
        p: (f64, f64),
        d: f64,
        foreign: &dyn Fn(Owner) -> bool,
    ) -> bool {
        let probe = (p.0 - d, p.1 - d, 2.0 * d, 2.0 * d);
        Self::near(&self.seg_cells, probe)
            .into_iter()
            .filter(|&i| foreign(self.segs[i].1))
            .any(|i| seg_distance(p, self.segs[i].0) < d)
    }
}

fn seg_distance(p: (f64, f64), (a, b): Segment) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 < 1e-12 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    };
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obstacles_report_what_they_cover() {
        let mut o = Obstacles::default();
        o.add_rect((0.0, 0.0, 10.0, 10.0));
        o.add_firm_rect((20.0, 0.0, 10.0, 10.0));
        o.add_segment(((0.0, 50.0), (100.0, 50.0)), Some(1));
        assert_eq!(o.covered((5.0, 5.0, 10.0, 10.0)), 25.0);
        assert_eq!(o.firmly_covered((25.0, 5.0, 10.0, 10.0)), 25.0);
        assert_eq!(o.crossed((10.0, 40.0, 20.0, 20.0), &|_| true), 20.0);
        assert_eq!(o.crossed((10.0, 40.0, 20.0, 20.0), &|w| w != Some(1)), 0.0);
        assert_eq!(o.extent(), Some((0.0, 0.0, 100.0, 50.0)));
    }
}
