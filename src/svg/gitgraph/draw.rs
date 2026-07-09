//! gitGraph commit glyphs, tag shapes, elbow joins, and auto-id hashing.

use std::fmt::Write as _;

use crate::svg::builder::{fnum, SvgBuilder};
use crate::svg::metrics;

use super::{COMMIT_R, ELBOW_R, TAG_FILL, TAG_STROKE};

/// Merge commit: two concentric circles (an outer disc with an inner ring),
/// distinct from a plain commit.
pub(super) fn draw_merge_glyph(svg: &mut SvgBuilder, x: f64, y: f64, color: &str) {
    svg.circle(
        x,
        y,
        COMMIT_R,
        &format!("fill=\"{color}\" stroke=\"#fff\" stroke-width=\"2\""),
    );
    svg.circle(
        x,
        y,
        COMMIT_R / 2.0,
        &format!("fill=\"#fff\" stroke=\"{color}\" stroke-width=\"1.5\""),
    );
}

/// Deterministic 7-hex digest of a commit's sequence number — mimics upstream's
/// `<seq>-<hash>` auto commit ids (e.g. `0-f56b5f2`) without a real RNG.
pub(super) fn seq_hash(seq: usize) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in (seq as u64).to_le_bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:07x}", h & 0x0fff_ffff)
}

/// Rounded right-angle join from parent `(px,py)` to child `(nx,ny)`. In the
/// horizontal layout it drops (changes lane) at the parent column, then runs to
/// the child; the vertical layout runs to the child lane, then advances in time.
pub(super) fn elbow_path(px: f64, py: f64, nx: f64, ny: f64, horizontal: bool) -> String {
    let mut s = String::new();
    let r = ELBOW_R;
    if horizontal {
        let vdir = (ny - py).signum();
        let hdir = (nx - px).signum();
        let _ = write!(
            s,
            "M{} {}L{} {}Q{} {} {} {}L{} {}",
            fnum(px),
            fnum(py),
            fnum(px),
            fnum(ny - r * vdir),
            fnum(px),
            fnum(ny),
            fnum(px + r * hdir),
            fnum(ny),
            fnum(nx),
            fnum(ny),
        );
    } else {
        let hdir = (nx - px).signum();
        let vdir = (ny - py).signum();
        let _ = write!(
            s,
            "M{} {}L{} {}Q{} {} {} {}L{} {}",
            fnum(px),
            fnum(py),
            fnum(nx - r * hdir),
            fnum(py),
            fnum(nx),
            fnum(py),
            fnum(nx),
            fnum(py + r * vdir),
            fnum(nx),
            fnum(ny),
        );
    }
    s
}

/// A tag-shaped label (upstream's yellow luggage tag) centered at `(cx, cy)`:
/// a rounded body with a pointed left edge, a punch hole, and the tag text.
pub(super) fn draw_tag(
    svg: &mut SvgBuilder,
    cx: f64,
    cy: f64,
    label: &str,
    text_color: &str,
    hole: &str,
    font_size: f64,
) {
    let tw = metrics::text_width(label, 7.0, font_size).max(8.0);
    let body_w = tw + 14.0;
    let point_w = 8.0;
    let th = 18.0;
    let total = body_w + point_w;
    let tip = cx - total / 2.0;
    let body_l = tip + point_w;
    let body_r = tip + total;
    let top = cy - th / 2.0;
    let bot = cy + th / 2.0;
    let mut path = String::new();
    let _ = write!(
        path,
        "M{} {}L{} {}L{} {}L{} {}L{} {}Z",
        fnum(tip),
        fnum(cy),
        fnum(body_l),
        fnum(top),
        fnum(body_r),
        fnum(top),
        fnum(body_r),
        fnum(bot),
        fnum(body_l),
        fnum(bot),
    );
    svg.path(
        &path,
        &format!("fill=\"{TAG_FILL}\" stroke=\"{TAG_STROKE}\" stroke-width=\"1\""),
    );
    svg.circle(body_l + 4.0, cy, 2.0, &format!("fill=\"{hole}\""));
    svg.text(
        (body_l + body_r) / 2.0 + 2.0,
        cy + 3.5,
        &format!("text-anchor=\"middle\" fill=\"{text_color}\" font-size=\"11\""),
        label,
    );
}

/// Cherry-pick commit: a disc carrying the two-cherry glyph (upstream's
/// dedicated cherry-pick marker).
pub(super) fn draw_cherry_pick_glyph(svg: &mut SvgBuilder, x: f64, y: f64, color: &str, fg: &str) {
    svg.circle(
        x,
        y,
        COMMIT_R,
        &format!("fill=\"{color}\" stroke=\"#fff\" stroke-width=\"2\""),
    );
    let cherry = "fill=\"#fff\"";
    svg.circle(x - 3.0, y + 2.0, 2.5, cherry);
    svg.circle(x + 3.0, y + 2.0, 2.5, cherry);
    let stem = &format!("stroke=\"{fg}\" stroke-width=\"1\"");
    svg.line(x - 3.0, y + 2.0, x + 4.0, y - 4.0, stem);
    svg.line(x + 3.0, y + 2.0, x - 4.0, y - 4.0, stem);
}
