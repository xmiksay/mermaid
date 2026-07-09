//! Built-in service/group icon glyphs and Iconify-name fallback captions.

use std::fmt::Write as _;

use crate::svg::builder::{fnum, SvgBuilder};

/// Draws the `size`-px icon glyph for `kind` at `(x, y)`. Returns `true` when the
/// name maps to a built-in glyph, `false` when it's unrecognized (the caller then
/// renders the raw name as a caption so the icon identity survives).
pub(super) fn draw_arch_icon(
    svg: &mut SvgBuilder,
    kind: &str,
    x: f64,
    y: f64,
    size: f64,
    stroke: &str,
    fill: &str,
) -> bool {
    let s = size / 32.0;
    let (paths, recognized): (&[&str], bool) = match kind {
        "database" | "db" => (
            &[
                "M4 8 C4 4 28 4 28 8 L28 24 C28 28 4 28 4 24 Z",
                "M4 8 C4 12 28 12 28 8",
                "M4 13 C4 17 28 17 28 13",
            ],
            true,
        ),
        "disk" => (
            &[
                "M16 4 A12 12 0 1 0 16 28 A12 12 0 1 0 16 4 Z",
                "M16 11 A5 5 0 1 0 16 21 A5 5 0 1 0 16 11 Z",
                "M15 15.5 A1 1 0 1 0 17 16.5 A1 1 0 1 0 15 15.5 Z",
            ],
            true,
        ),
        "server" => (
            &[
                "M3 5 H29 V13 H3 Z",
                "M3 16 H29 V24 H3 Z",
                "M6 9 H9 M6 20 H9",
                "M24 9 H26 M24 20 H26",
            ],
            true,
        ),
        "cloud" => (
            &["M9 24 C4 24 3 17 9 16 C9 11 16 9 18 14 C22 11 27 14 25 18 C30 19 28 24 24 24 Z"],
            true,
        ),
        "internet" | "globe" => (
            &[
                "M16 4 A12 12 0 1 0 16 28 A12 12 0 1 0 16 4 Z",
                "M4 16 H28",
                "M16 4 C9 11 9 21 16 28",
                "M16 4 C23 11 23 21 16 28",
            ],
            true,
        ),
        "queue" | "kafka" => (
            &["M4 10 H28 V22 H4 Z", "M10 10 V22 M16 10 V22 M22 10 V22"],
            true,
        ),
        _ => (&["M6 6 H26 V26 H6 Z"], false),
    };
    let _ = write!(
        svg.body,
        "<g transform=\"translate({x} {y}) scale({s})\" fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.5\" stroke-linejoin=\"round\" stroke-linecap=\"round\">",
        x = fnum(x),
        y = fnum(y),
        s = fnum(s),
    );
    for p in paths {
        let _ = write!(svg.body, "<path d=\"{p}\"/>");
    }
    svg.raw("</g>");
    recognized
}

/// Shortens an Iconify-style icon name for the fallback caption: keeps the
/// segment after the last `:` (`logos:aws-lambda` → `aws-lambda`) and caps the
/// length so a long name can't overflow the service box.
pub(super) fn truncate_icon_name(name: &str) -> String {
    let short = name.rsplit(':').next().unwrap_or(name);
    const MAX: usize = 16;
    if short.chars().count() > MAX {
        let head: String = short.chars().take(MAX - 1).collect();
        format!("{head}…")
    } else {
        short.to_string()
    }
}
