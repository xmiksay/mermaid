//! Built-in categorical color palettes. Split out of `theme/mod.rs` to keep
//! each file under the size cap.
//!
//! Upstream Mermaid derives three *distinct* categorical scales from a theme's
//! primary/secondary/tertiary colors, and different diagram kinds read
//! different ones:
//!
//! - [`CSCALE_DEFAULT`] — the generic `cScale0..11` scale (journey, timeline,
//!   sankey, radar, packet, kanban, quadrant, xychart, treemap).
//! - [`PIE_DEFAULT`] — the `pie1..12` scale (pie charts).
//! - [`GIT_DEFAULT`] — the `git0..7` lane scale (gitGraph).
//!
//! The default-theme values here are the exact colors upstream's `khroma`
//! pipeline computes from `primaryColor #ECECFF` / `secondaryColor #ffffde`
//! (hue-rotated + lightness-adjusted, the `cScale` set additionally darkened
//! 10% and the `git` set darkened 25%). The dark/forest/neutral themes keep a
//! single hand-tuned palette shared
//! across all three scales — deriving via the upstream formulas would drive
//! their already-dark or achromatic primaries to black/monochrome.

use super::Str;
use std::borrow::Cow;

/// Generic `cScale0..11` categorical scale for the default theme.
pub(super) const CSCALE_DEFAULT: [Str; 12] = [
    Cow::Borrowed("#B9B9FF"),
    Cow::Borrowed("#FFFFAB"),
    Cow::Borrowed("#E8FFB9"),
    Cow::Borrowed("#DCB9FF"),
    Cow::Borrowed("#FFB9FF"),
    Cow::Borrowed("#FFB9DC"),
    Cow::Borrowed("#FFB9B9"),
    Cow::Borrowed("#FFDCB9"),
    Cow::Borrowed("#DCFFB9"),
    Cow::Borrowed("#B9FFDC"),
    Cow::Borrowed("#B9FFFF"),
    Cow::Borrowed("#B9DCFF"),
];

/// `pie1..12` slice scale for the default theme.
pub(super) const PIE_DEFAULT: [Str; 12] = [
    Cow::Borrowed("#ECECFF"),
    Cow::Borrowed("#FFFFDE"),
    Cow::Borrowed("#B5FF20"),
    Cow::Borrowed("#B9B9FF"),
    Cow::Borrowed("#FFFF45"),
    Cow::Borrowed("#D7FF86"),
    Cow::Borrowed("#FF86FF"),
    Cow::Borrowed("#20FFFF"),
    Cow::Borrowed("#FF2020"),
    Cow::Borrowed("#FF20FF"),
    Cow::Borrowed("#20FF8F"),
    Cow::Borrowed("#FF5353"),
];

/// `git0..7` lane scale for the default theme. Upstream's `theme-default.js`
/// darkens each raw base color (`#ECECFF`, `#FFFFDE`, …) by 25% lightness in
/// light mode before use; without that step the whole graph renders washed
/// out (issue #309). These are the darkened lane colors.
pub(super) const GIT_DEFAULT: [Str; 8] = [
    Cow::Borrowed("#6D6DFF"),
    Cow::Borrowed("#FFFF5E"),
    Cow::Borrowed("#D1FF6D"),
    Cow::Borrowed("#6DB2FF"),
    Cow::Borrowed("#6DFFFF"),
    Cow::Borrowed("#6DFFB2"),
    Cow::Borrowed("#FF6DFF"),
    Cow::Borrowed("#FF6D6D"),
];

/// Shared dark-theme palette (all three scales). Light pastels on the dark
/// background stay legible where the upstream darken-10% derivation would not.
pub(super) const PALETTE_DARK: [Str; 10] = [
    Cow::Borrowed("#7CB5FF"),
    Cow::Borrowed("#A6D88A"),
    Cow::Borrowed("#FFD980"),
    Cow::Borrowed("#FF8888"),
    Cow::Borrowed("#8FD8F2"),
    Cow::Borrowed("#5BC09A"),
    Cow::Borrowed("#FF9B6E"),
    Cow::Borrowed("#B58CE0"),
    Cow::Borrowed("#FF9CDA"),
    Cow::Borrowed("#8FE0BA"),
];

/// Shared forest-theme palette (all three scales).
pub(super) const PALETTE_FOREST: [Str; 10] = [
    Cow::Borrowed("#4E8A4E"),
    Cow::Borrowed("#7BAA5A"),
    Cow::Borrowed("#A8C870"),
    Cow::Borrowed("#D7E0A0"),
    Cow::Borrowed("#A8C8A8"),
    Cow::Borrowed("#3A6B3A"),
    Cow::Borrowed("#6BA66B"),
    Cow::Borrowed("#C0D8A0"),
    Cow::Borrowed("#7AA070"),
    Cow::Borrowed("#5C8C5C"),
];

/// Shared neutral-theme palette (all three scales).
pub(super) const PALETTE_NEUTRAL: [Str; 10] = [
    Cow::Borrowed("#444"),
    Cow::Borrowed("#666"),
    Cow::Borrowed("#888"),
    Cow::Borrowed("#AAA"),
    Cow::Borrowed("#555"),
    Cow::Borrowed("#777"),
    Cow::Borrowed("#999"),
    Cow::Borrowed("#BBB"),
    Cow::Borrowed("#5E5E5E"),
    Cow::Borrowed("#7E7E7E"),
];
