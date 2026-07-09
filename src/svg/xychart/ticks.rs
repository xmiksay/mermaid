//! Value-axis "nice" tick computation (d3-style).

/// Round a value domain to "nice" bounds and enumerate round tick values,
/// mirroring d3's `ticks()`/`nice()`: pick a step of 1/2/5 × 10^k so ~10 ticks
/// fit the span, then extend the domain out to the nearest step multiples.
/// Returns `(nice_min, nice_max, ticks)`.
pub(super) fn nice_ticks(vmin: f64, vmax: f64) -> (f64, f64, Vec<f64>) {
    const TARGET: f64 = 10.0;
    let step = tick_step(vmax - vmin, TARGET);
    let lo = (vmin / step).floor() * step;
    let hi = (vmax / step).ceil() * step;
    let count = ((hi - lo) / step).round().max(1.0) as usize;
    let ticks = (0..=count).map(|i| lo + i as f64 * step).collect();
    (lo, hi, ticks)
}

/// The "nice" step size (1/2/5 × 10^k) that fits roughly `count` ticks across
/// `span`, matching d3's `tickStep`.
fn tick_step(span: f64, count: f64) -> f64 {
    let step0 = span.abs() / count.max(1.0);
    let mag = 10f64.powf(step0.log10().floor());
    let error = step0 / mag;
    // d3's thresholds: √50, √10, √2.
    let factor = if error >= 50f64.sqrt() {
        10.0
    } else if error >= 10f64.sqrt() {
        5.0
    } else if error >= 2f64.sqrt() {
        2.0
    } else {
        1.0
    };
    factor * mag
}
