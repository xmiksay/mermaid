//! Every edge's drawn lines and every subgraph frame border, gathered once
//! and patched as edges move, and how long a candidate curve runs along
//! them.

use std::collections::{BTreeMap, HashMap};

use super::super::route::Pt;
use super::body::dot;
use super::scene::segment_box;

/// Closer than this to another line counts as running along it.
const NEAR: f64 = 6.0;
/// A stretch along another line this long is a defect, not a crossing.
const RUN: f64 = 30.0;
/// The owner of a frame border: no edge.
pub(super) const FRAME: usize = usize::MAX;

/// One line: start, unit direction, length, bounding box grown by `NEAR`.
type Seg = (Pt, Pt, f64, [f64; 4]);

fn seg((p, q): (Pt, Pt)) -> Option<Seg> {
    let (dx, dy) = (q.0 - p.0, q.1 - p.1);
    let len = (dx * dx + dy * dy).sqrt();
    (len > 1e-9 && len.is_finite()).then(|| (p, (dx / len, dy / len), len, segment_box(p, q, NEAR)))
}

/// Smallest grid cell side.
const CELL: f64 = 32.0;
/// Most cells in the grid; a larger diagram gets larger cells.
const MAX_GRID: f64 = 262_144.0;
/// Most cells one line may cover before every lookup tests it.
const MAX_CELLS: usize = 256;

/// The lines, each owned by the edge drawing it (`FRAME` for a border), in
/// a grid over the diagram: lines are short and many, and an edge that
/// moves only adds its new ones. Anything past the grid's edge lands in
/// its border cells, for lines and lookups alike.
pub(super) struct Lines {
    segs: Vec<Seg>,
    owner: Vec<usize>,
    dead: Vec<bool>,
    by_owner: BTreeMap<usize, Vec<usize>>,
    origin: Pt,
    cell: f64,
    cols: usize,
    rows: usize,
    /// The first lines, cell by cell: cell `c` holds
    /// `first[start[c]..start[c + 1]]` (placed in one go: tens of thousands
    /// of short lines).
    start: Vec<usize>,
    first: Vec<usize>,
    /// Lines added since, by cell.
    added: HashMap<usize, Vec<usize>>,
    /// Lines covering too many cells.
    wide: Vec<usize>,
}

impl Lines {
    pub(super) fn new(segs: Vec<(usize, (Pt, Pt))>) -> Self {
        let all: Vec<(usize, Seg)> = segs
            .into_iter()
            .filter_map(|(o, s)| Some((o, seg(s)?)))
            .collect();
        let inf = f64::INFINITY;
        let b = all.iter().fold([inf, inf, -inf, -inf], |m, (_, s)| {
            [
                m[0].min(s.3[0]),
                m[1].min(s.3[1]),
                m[2].max(s.3[2]),
                m[3].max(s.3[3]),
            ]
        });
        let (w, h) = (b[2] - b[0], b[3] - b[1]);
        let finite = w.is_finite() && h.is_finite();
        let cell = if finite {
            CELL.max((w * h / MAX_GRID).sqrt()).max(w.max(h) / MAX_GRID)
        } else {
            CELL
        };
        let dim = |len: f64| if finite { (len / cell) as usize + 1 } else { 1 };
        let (cols, rows) = (dim(w), dim(h));
        let mut lines = Self {
            segs: Vec::new(),
            owner: Vec::new(),
            dead: Vec::new(),
            by_owner: BTreeMap::new(),
            origin: if finite { (b[0], b[1]) } else { (0.0, 0.0) },
            cell,
            cols,
            rows,
            start: Vec::new(),
            first: Vec::new(),
            added: HashMap::new(),
            wide: Vec::new(),
        };
        // Counted, then placed.
        let covers: Vec<_> = all.iter().map(|(_, s)| lines.cover(&s.3)).collect();
        let mut start = vec![0usize; cols * rows + 1];
        for c in covers.iter().flat_map(|c| lines.cells(*c)) {
            if let Some(n) = start.get_mut(c + 1) {
                *n += 1;
            }
        }
        for c in 1..start.len() {
            start[c] += start[c - 1];
        }
        let mut next = start.clone();
        let mut first = vec![0; start.last().copied().unwrap_or(0)];
        for (i, cover) in covers.iter().enumerate() {
            for c in lines.cells(*cover) {
                if let Some(n) = next.get_mut(c) {
                    if let Some(slot) = first.get_mut(*n) {
                        *slot = i;
                    }
                    *n += 1;
                }
            }
        }
        (lines.start, lines.first) = (start, first);
        lines.wide = (0..covers.len()).filter(|&i| covers[i].is_none()).collect();
        let n = all.len();
        lines.owner = all.iter().map(|(o, _)| *o).collect();
        lines.segs = all.into_iter().map(|(_, s)| s).collect();
        lines.dead = vec![false; n];
        // Lines come grouped by owner.
        let mut i = 0;
        while let Some(&o) = lines.owner.get(i) {
            let j = (i..n).find(|&j| lines.owner[j] != o).unwrap_or(n);
            lines.by_owner.entry(o).or_default().extend(i..j);
            i = j;
        }
        lines
    }

    /// The cells box `b` covers, clamped to the grid; `None` past
    /// `MAX_CELLS` (or for non-finite input).
    fn cover(&self, b: &[f64; 4]) -> Option<(usize, usize, usize, usize)> {
        let at = |v: f64, o: f64, n: usize| {
            let c = ((v - o) / self.cell).floor();
            c.is_finite()
                .then(|| c.max(0.0).min(n as f64 - 1.0) as usize)
        };
        let (x0, x1) = (
            at(b[0], self.origin.0, self.cols)?,
            at(b[2], self.origin.0, self.cols)?,
        );
        let (y0, y1) = (
            at(b[1], self.origin.1, self.rows)?,
            at(b[3], self.origin.1, self.rows)?,
        );
        ((x1 - x0 + 1) * (y1 - y0 + 1) <= MAX_CELLS).then_some((x0, x1, y0, y1))
    }

    /// The cells of a cover, row by row.
    fn cells(
        &self,
        cover: Option<(usize, usize, usize, usize)>,
    ) -> impl Iterator<Item = usize> + '_ {
        cover.into_iter().flat_map(move |(x0, x1, y0, y1)| {
            (y0..=y1).flat_map(move |y| (x0..=x1).map(move |x| y * self.cols + x))
        })
    }

    fn push(&mut self, owner: usize, sg: Seg) {
        let i = self.segs.len();
        self.segs.push(sg);
        self.owner.push(owner);
        self.dead.push(false);
        self.by_owner.entry(owner).or_default().push(i);
    }

    fn add(&mut self, owner: usize, sg: Seg) {
        let i = self.segs.len();
        let cover = self.cover(&sg.3);
        if cover.is_none() {
            self.wide.push(i);
        }
        let cells: Vec<usize> = self.cells(cover).collect();
        for c in cells {
            self.added.entry(c).or_default().push(i);
        }
        self.push(owner, sg);
    }

    /// Edge `edge` is now drawn through `samples`.
    pub(super) fn moved(&mut self, edge: usize, samples: &[Pt]) {
        for i in self.by_owner.remove(&edge).unwrap_or_default() {
            if let Some(d) = self.dead.get_mut(i) {
                *d = true;
            }
        }
        for w in samples.windows(2) {
            if let Some(sg) = seg((w[0], w[1])) {
                self.add(edge, sg);
            }
        }
    }

    /// Live lines of edges other than `edge` whose box (grown by `NEAR`)
    /// may hold `m`: those in its cell, and the wide ones.
    fn at(&self, edge: usize, m: Pt) -> impl Iterator<Item = usize> + '_ {
        let c = self
            .cover(&[m.0, m.1, m.0, m.1])
            .map(|(x, _, y, _)| y * self.cols + x);
        let first = c
            .and_then(|c| Some(*self.start.get(c)?..*self.start.get(c + 1)?))
            .and_then(|r| self.first.get(r));
        first
            .into_iter()
            .flatten()
            .chain(c.and_then(|c| self.added.get(&c)).into_iter().flatten())
            .chain(&self.wide)
            .copied()
            .filter(move |&i| {
                !self.dead.get(i).copied().unwrap_or(true)
                    && self.owner.get(i).is_some_and(|&o| o != edge)
            })
    }

    /// Whether the sample segment with middle `m` and direction `u` runs
    /// beside line `i`: within `NEAR` of it, nearly parallel.
    fn beside(&self, i: usize, m: Pt, u: Pt) -> bool {
        let Some(&(p, d, len, bx)) = self.segs.get(i) else {
            return false;
        };
        if !(bx[0] <= m.0 && m.0 <= bx[2] && bx[1] <= m.1 && m.1 <= bx[3]) {
            return false;
        }
        let rel = (m.0 - p.0, m.1 - p.1);
        let t = dot(rel, d).max(0.0).min(len);
        let off = (rel.0 - d.0 * t, rel.1 - d.1 * t);
        off.0 * off.0 + off.1 * off.1 <= NEAR * NEAR && dot(u, d).abs() > 0.9
    }

    /// How long the sampled curve of `edge` runs along another edge's line
    /// or a frame border (within `NEAR`, nearly parallel), counting only
    /// stretches of at least `RUN`. Each segment looks up only the lines in
    /// its middle's grid cell.
    pub(super) fn along(&self, edge: usize, samples: &[Pt]) -> f64 {
        let (mut total, mut run) = (0.0, 0.0);
        for w in samples.windows(2) {
            let (a, b) = (w[0], w[1]);
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let len2 = dx * dx + dy * dy;
            let beside = len2 > 0.0 && {
                let len = len2.sqrt();
                let (m, u) = (((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0), (dx / len, dy / len));
                self.at(edge, m).any(|i| self.beside(i, m, u))
            };
            if beside {
                run += len2.sqrt();
            } else {
                if run >= RUN {
                    total += run;
                }
                run = 0.0;
            }
        }
        if run >= RUN {
            total += run;
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_along_a_line_counts_a_crossing_does_not() {
        let lines = Lines::new(vec![(1, ((0.0, 0.0), (0.0, 200.0)))]);
        let walk = |x0: f64, y0: f64, x1: f64, y1: f64| -> Vec<Pt> {
            (0..=40)
                .map(|k| {
                    let t = f64::from(k) / 40.0;
                    (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t)
                })
                .collect()
        };
        // 3px beside it for 150px: one long stretch.
        let along = lines.along(0, &walk(3.0, 20.0, 3.0, 170.0));
        assert!((along - 150.0).abs() < 8.0, "{along}");
        // Straight across: nothing.
        assert_eq!(lines.along(0, &walk(-100.0, 100.0, 100.0, 100.0)), 0.0);
        // Its own lines do not count.
        assert_eq!(lines.along(1, &walk(3.0, 20.0, 3.0, 170.0)), 0.0);
        // Beside it for less than `RUN`: nothing either.
        assert_eq!(lines.along(0, &walk(3.0, 20.0, 3.0, 40.0)), 0.0);
    }
}
