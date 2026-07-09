//! Legend row and per-point data labels.

use crate::svg::builder::SvgBuilder;
use crate::svg::metrics::text_width;

/// Draw a centered legend row of colored swatches + series titles just above
/// the plot, starting at `top`.
pub(super) fn draw_legend(
    svg: &mut SvgBuilder,
    entries: &[(usize, &str)],
    color_at: &dyn Fn(usize) -> String,
    width: f64,
    top: f64,
    fg: &str,
) {
    const SWATCH: f64 = 12.0;
    const GAP: f64 = 6.0;
    const ITEM_GAP: f64 = 18.0;
    let entry_w = |t: &str| SWATCH + GAP + text_width(t, 7.0, 12.0);
    let total: f64 = entries.iter().map(|(_, t)| entry_w(t)).sum::<f64>()
        + ITEM_GAP * (entries.len().saturating_sub(1)) as f64;
    let mut x = (width - total) / 2.0;
    for (i, t) in entries {
        svg.rect(
            x,
            top,
            SWATCH,
            SWATCH,
            &format!("fill=\"{}\"", color_at(*i)),
        );
        svg.text(
            x + SWATCH + GAP,
            top + SWATCH - 2.0,
            &format!("text-anchor=\"start\" fill=\"{fg}\" font-size=\"12\""),
            t,
        );
        x += entry_w(t) + ITEM_GAP;
    }
}

/// Draw a per-point data label. Horizontal charts anchor it to the start
/// (right of the point); vertical charts center it above the point.
pub(super) fn draw_point_label(
    svg: &mut SvgBuilder,
    x: f64,
    y: f64,
    horiz: bool,
    fg: &str,
    label: &str,
) {
    let anchor = if horiz { "start" } else { "middle" };
    svg.text(
        x,
        y,
        &format!("text-anchor=\"{anchor}\" fill=\"{fg}\" font-size=\"10\""),
        label,
    );
}
