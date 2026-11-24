//! A polyline with an arc-length table, for walking along a relation's line.

/// A polyline with its cumulative arc lengths, for O(log n) lookups.
pub(super) struct ArcPath<'a> {
    pts: &'a [(f64, f64)],
    cum: Vec<f64>,
}

impl<'a> ArcPath<'a> {
    pub(super) fn new(pts: &'a [(f64, f64)]) -> Self {
        let mut cum = Vec::with_capacity(pts.len());
        let mut acc = 0.0;
        cum.push(0.0);
        for w in pts.windows(2) {
            acc += (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1);
            cum.push(acc);
        }
        Self { pts, cum }
    }

    pub(super) fn total(&self) -> f64 {
        self.cum.last().copied().unwrap_or(0.0)
    }

    /// The point `d` along the path and the unit direction there.
    pub(super) fn at(&self, d: f64) -> ((f64, f64), (f64, f64)) {
        let d = d.clamp(0.0, self.total());
        let i = self
            .cum
            .partition_point(|&c| c < d)
            .clamp(1, self.pts.len() - 1);
        // Skip zero-length segments (doubled control points) for direction.
        let mut j = i;
        while j < self.pts.len() - 1 && self.cum[j] - self.cum[j - 1] < 1e-9 {
            j += 1;
        }
        let (a, b) = (self.pts[j - 1], self.pts[j]);
        let len = (self.cum[j] - self.cum[j - 1]).max(1e-9);
        let t = ((d - self.cum[j - 1]) / len).clamp(0.0, 1.0);
        let p = (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
        ((p), ((b.0 - a.0) / len, (b.1 - a.1) / len))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arc_path_looks_up_points_and_directions() {
        let pts = [(0.0, 0.0), (10.0, 0.0), (10.0, 0.0), (10.0, 20.0)];
        let arc = ArcPath::new(&pts);
        assert_eq!(arc.total(), 30.0);
        assert_eq!(arc.at(5.0), ((5.0, 0.0), (1.0, 0.0)));
        assert_eq!(arc.at(20.0), ((10.0, 10.0), (0.0, 1.0)));
        assert_eq!(arc.at(99.0).0, (10.0, 20.0));
    }
}
