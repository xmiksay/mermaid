//! A static index over boxes: bands about one box thick, as rows and as
//! columns, each band sorted along the other axis, so a lookup is a binary
//! search per band crossed.

use std::cell::OnceCell;
use std::collections::BTreeMap;

/// Smallest band.
const MIN_BAND: f64 = 8.0;
/// Most bands across one axis; a larger scene gets wider bands.
const MAX_BANDS: f64 = 65_536.0;
/// Most bands one box may cross before it skips the index.
const MAX_CROSS: usize = 64;

/// One band: `(lo, hi, body)` on the other axis, sorted by `lo`, and the
/// band's longest `hi - lo`.
type Band = (Vec<(f64, f64, usize)>, f64);

/// Bodies in bands across one axis (`axis`: 0 bands by x, 1 by y), each band
/// sorted along the other axis, so a lookup is a binary search per band.
#[derive(Default)]
pub(super) struct Bands {
    axis: usize,
    origin: f64,
    size: f64,
    bands: Vec<Band>,
    /// Bodies crossing too many bands, near every box.
    wide: Vec<usize>,
}

impl Bands {
    fn new(items: &[(usize, [f64; 4])], axis: usize) -> Self {
        let (lo, hi) = items
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |m, (_, b)| {
                (m.0.min(b[axis]), m.1.max(b[axis + 2]))
            });
        let len = hi - lo;
        let mut out = Self {
            axis,
            ..Self::default()
        };
        if !len.is_finite() {
            out.wide = items.iter().map(|&(i, _)| i).collect();
            return out;
        }
        // Bands about one body thick: a segment then meets few bodies it
        // does not overlap, even beside a dense rank.
        let mut sides: Vec<f64> = items.iter().map(|(_, b)| b[axis + 2] - b[axis]).collect();
        sides.sort_by(f64::total_cmp);
        let median = sides.get(sides.len() / 2).copied().unwrap_or(MIN_BAND);
        (out.origin, out.size) = (lo, median.max(MIN_BAND).max(len / MAX_BANDS));
        out.bands = vec![(Vec::new(), 0.0); (len / out.size) as usize + 1];
        let other = 1 - axis;
        for &(i, b) in items {
            match out.cross(b[axis], b[axis + 2]) {
                Some((first, last)) => {
                    for (band, reach) in out.bands.iter_mut().take(last + 1).skip(first) {
                        band.push((b[other], b[other + 2], i));
                        *reach = reach.max(b[other + 2] - b[other]);
                    }
                }
                None => out.wide.push(i),
            }
        }
        for (band, _) in &mut out.bands {
            band.sort_by(|p, q| p.0.total_cmp(&q.0));
        }
        out
    }

    /// The bands `lo..=hi` crosses, clipped to the index; `None` past
    /// `MAX_CROSS` bands or for non-finite input.
    fn cross(&self, lo: f64, hi: f64) -> Option<(usize, usize)> {
        // Half-open bands: a box ending exactly on a band's start stays out
        // of it, so a rank's boxes and the gap below it share no band.
        let at = |v: f64| (v - self.origin) / self.size;
        let a = at(lo).floor().max(0.0);
        let b = (at(hi).ceil() - 1.0).min(self.bands.len() as f64 - 1.0);
        if !(a.is_finite() && b.is_finite()) || b - a >= MAX_CROSS as f64 {
            return None;
        }
        Some((a as usize, (b.max(a)) as usize))
    }

    /// How many bands the box `b` crosses, if it can be looked up.
    fn cost(&self, b: &[f64; 4]) -> Option<usize> {
        let (lo, hi) = (b[self.axis], b[self.axis + 2]);
        if hi < self.origin || lo > self.origin + self.size * self.bands.len() as f64 {
            return Some(0);
        }
        self.cross(lo, hi).map(|(a, z)| z - a + 1)
    }

    /// Bodies that may overlap the box `b`, possibly repeated.
    fn near(&self, b: &[f64; 4], out: &mut Vec<usize>) {
        out.extend(&self.wide);
        let other = 1 - self.axis;
        let (qlo, qhi) = (b[other], b[other + 2]);
        let (lo, hi) = (b[self.axis], b[self.axis + 2]);
        if hi < self.origin || lo > self.origin + self.size * self.bands.len() as f64 {
            return;
        }
        let Some((first, last)) = self.cross(lo, hi) else {
            return;
        };
        for (band, reach) in self.bands.iter().take(last + 1).skip(first) {
            let start = band.partition_point(|e| e.0 < qlo - reach);
            let end = band.partition_point(|e| e.0 <= qhi);
            out.extend(band.get(start..end).into_iter().flatten().map(|e| e.2));
        }
    }
}

/// Below this many boxes a plain list beats building bands.
const SMALL: usize = 48;

/// Boxes indexed in rows and in columns; a box is looked up in whichever
/// it crosses fewer of.
pub(super) enum Index {
    /// Few boxes: every lookup gets them all.
    List(Vec<usize>),
    Bands {
        rows: Bands,
        cols: Bands,
    },
}

impl Index {
    pub(super) fn new(items: &[(usize, [f64; 4])]) -> Self {
        if items.len() <= SMALL {
            return Self::List(items.iter().map(|&(i, _)| i).collect());
        }
        Self::Bands {
            rows: Bands::new(items, 1),
            cols: Bands::new(items, 0),
        }
    }

    /// Adds to `out` the boxes that may overlap `b`, possibly repeated;
    /// false when `b` is too large to look up.
    pub(super) fn near(&self, b: &[f64; 4], out: &mut Vec<usize>) -> bool {
        match self {
            Self::List(all) => out.extend(all),
            Self::Bands { rows, cols } => match (rows.cost(b), cols.cost(b)) {
                (Some(r), Some(c)) if c < r => cols.near(b, out),
                (Some(_), _) => rows.near(b, out),
                (None, Some(_)) => cols.near(b, out),
                (None, None) => return false,
            },
        }
        true
    }
}

/// Items with an owning edge, indexed and patched as edges move: a moved
/// edge's old items die and its new ones go into a second index, built when
/// first looked up and folded into the main one once it holds a quarter as
/// many.
pub(super) struct Layer<T> {
    pub(super) items: Vec<T>,
    pub(super) owner: Vec<usize>,
    dead: Vec<bool>,
    area: fn(&T) -> [f64; 4],
    by_owner: BTreeMap<usize, Vec<usize>>,
    index: Index,
    fresh: Vec<usize>,
    fresh_index: OnceCell<Index>,
}

impl<T> Layer<T> {
    pub(super) fn new(items: Vec<(usize, T)>, area: fn(&T) -> [f64; 4]) -> Self {
        let (owner, items): (Vec<usize>, Vec<T>) = items.into_iter().unzip();
        let mut by_owner: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (i, &o) in owner.iter().enumerate() {
            by_owner.entry(o).or_default().push(i);
        }
        let mut layer = Self {
            dead: vec![false; items.len()],
            items,
            owner,
            area,
            by_owner,
            index: Index::List(Vec::new()),
            fresh: Vec::new(),
            fresh_index: OnceCell::new(),
        };
        let all: Vec<usize> = (0..layer.items.len()).collect();
        layer.index = layer.index_of(&all);
        layer
    }

    fn index_of(&self, ids: &[usize]) -> Index {
        let boxes: Vec<(usize, [f64; 4])> = ids
            .iter()
            .filter(|&&i| !self.dead.get(i).copied().unwrap_or(true))
            .filter_map(|&i| Some((i, (self.area)(self.items.get(i)?))))
            .collect();
        Index::new(&boxes)
    }

    /// Live items that may overlap `b`, possibly repeated.
    pub(super) fn near(&self, b: &[f64; 4], out: &mut Vec<usize>) {
        out.clear();
        if !self.index.near(b, out) {
            out.clear();
            out.extend(0..self.items.len());
        } else if !self.fresh.is_empty() {
            let fresh = self.fresh_index.get_or_init(|| self.index_of(&self.fresh));
            if !fresh.near(b, out) {
                out.extend(&self.fresh);
            }
        }
        out.retain(|&i| !self.dead.get(i).copied().unwrap_or(true));
    }

    /// `edge`'s items are now `new`.
    pub(super) fn replace(&mut self, edge: usize, new: Vec<T>) {
        for i in self.by_owner.remove(&edge).unwrap_or_default() {
            if let Some(d) = self.dead.get_mut(i) {
                *d = true;
            }
        }
        let mut mine = Vec::with_capacity(new.len());
        for t in new {
            mine.push(self.items.len());
            self.fresh.push(self.items.len());
            self.items.push(t);
            self.owner.push(edge);
            self.dead.push(false);
        }
        self.by_owner.insert(edge, mine);
        self.fresh_index = OnceCell::new();
        if self.fresh.len() * 4 > self.items.len() {
            let all: Vec<usize> = (0..self.items.len()).collect();
            self.index = self.index_of(&all);
            self.fresh.clear();
        }
    }
}
