//! Mermaid v11 `@{ shape: … }` node geometries kept out of `nodes.rs` so the
//! main node file stays small. Each shape draws a distinct outline; `fill_attr`
//! carries the resolved fill/stroke and `stroke` is the plain stroke colour used
//! for the seam/decoration lines.

use crate::parse::NodeShape;
use crate::svg::builder::{fnum, SvgBuilder};

/// Draw one of the v11 geometries. Any shape without a bespoke branch falls back
/// to a plain rectangle so content is never dropped.
pub(super) fn draw(
    svg: &mut SvgBuilder,
    shape: NodeShape,
    (x, y, w, h): (f64, f64, f64, f64),
    (cx, cy): (f64, f64),
    fill_attr: &str,
    stroke: &str,
) {
    let seam = format!("fill=\"none\" stroke=\"{stroke}\" stroke-width=\"1.5\"");
    match shape {
        NodeShape::NotchedRect => {
            let n = (h / 4.0).min(14.0);
            svg.path(
                &format!(
                    "M{} {} L{} {} L{} {} L{} {} L{} {} Z",
                    fnum(x + n),
                    fnum(y),
                    fnum(x + w),
                    fnum(y),
                    fnum(x + w),
                    fnum(y + h),
                    fnum(x),
                    fnum(y + h),
                    fnum(x),
                    fnum(y + n),
                ),
                fill_attr,
            );
        }
        NodeShape::Document => {
            svg.path(&document_path(x, y, w, h, 6.0), fill_attr);
        }
        NodeShape::MultiDocument => {
            let o = 5.0;
            svg.path(&document_path(x + o, y + o, w - o, h - o, 5.0), fill_attr);
            svg.path(&document_path(x, y, w - o, h - o, 5.0), fill_attr);
        }
        NodeShape::TaggedDocument => {
            let a = 6.0;
            svg.path(&document_path(x, y, w, h, a), fill_attr);
            // Folded corner tag at the bottom-right.
            let t = 12.0;
            svg.path(
                &format!(
                    "M{} {} L{} {} L{} {} Z",
                    fnum(x + w - t),
                    fnum(y + h - a),
                    fnum(x + w),
                    fnum(y + h - a),
                    fnum(x + w),
                    fnum(y + h - a - t),
                ),
                &format!("fill=\"{stroke}\" stroke=\"none\""),
            );
        }
        NodeShape::LightningBolt => {
            let pts = [
                (0.50, 0.0),
                (0.10, 0.55),
                (0.42, 0.55),
                (0.28, 1.0),
                (0.90, 0.40),
                (0.55, 0.40),
                (0.72, 0.0),
            ];
            svg.path(&poly(x, y, w, h, &pts), fill_attr);
        }
        NodeShape::Hourglass => {
            // Two triangles meeting apex-to-apex at the centre.
            svg.path(
                &format!(
                    "M{} {} L{} {} L{} {} L{} {} L{} {} L{} {} Z",
                    fnum(x),
                    fnum(y),
                    fnum(x + w),
                    fnum(y),
                    fnum(cx),
                    fnum(cy),
                    fnum(x + w),
                    fnum(y + h),
                    fnum(x),
                    fnum(y + h),
                    fnum(cx),
                    fnum(cy),
                ),
                fill_attr,
            );
        }
        NodeShape::Comment => {
            // Curly braces flanking the label; no body fill.
            let q = 6.0;
            svg.path(&brace_path(x + q, y, y + h, q, 1.0), &seam);
            svg.path(&brace_path(x + w - q, y, y + h, q, -1.0), &seam);
        }
        NodeShape::Delay => {
            let r = h / 2.0;
            svg.path(
                &format!(
                    "M{} {} L{} {} A{} {} 0 0 1 {} {} L{} {} Z",
                    fnum(x),
                    fnum(y),
                    fnum(x + w - r),
                    fnum(y),
                    fnum(r),
                    fnum(r),
                    fnum(x + w - r),
                    fnum(y + h),
                    fnum(x),
                    fnum(y + h),
                ),
                fill_attr,
            );
        }
        NodeShape::DirectAccessStorage => {
            // Horizontal cylinder: rounded end-caps plus a front seam arc.
            let rx = 10.0;
            svg.path(
                &format!(
                    "M{} {} L{} {} A{} {} 0 0 1 {} {} L{} {} A{} {} 0 0 1 {} {} Z",
                    fnum(x + rx),
                    fnum(y),
                    fnum(x + w - rx),
                    fnum(y),
                    fnum(rx),
                    fnum(h / 2.0),
                    fnum(x + w - rx),
                    fnum(y + h),
                    fnum(x + rx),
                    fnum(y + h),
                    fnum(rx),
                    fnum(h / 2.0),
                    fnum(x + rx),
                    fnum(y),
                ),
                fill_attr,
            );
            svg.path(
                &format!(
                    "M{} {} A{} {} 0 0 0 {} {}",
                    fnum(x + rx),
                    fnum(y),
                    fnum(rx),
                    fnum(h / 2.0),
                    fnum(x + rx),
                    fnum(y + h),
                ),
                &seam,
            );
        }
        NodeShape::LinedCylinder => {
            cylinder(svg, x, y, w, h, fill_attr, &seam);
            // Extra seam ring below the top cap marks the disk.
            svg.path(
                &format!(
                    "M{} {} A{} {} 0 0 0 {} {}",
                    fnum(x),
                    fnum(y + 16.0),
                    fnum(w / 2.0),
                    fnum(8.0),
                    fnum(x + w),
                    fnum(y + 16.0),
                ),
                &seam,
            );
        }
        NodeShape::LinedProcess => {
            svg.rect(x, y, w, h, fill_attr);
            svg.line(x + 8.0, y, x + 8.0, y + h, &seam);
        }
        NodeShape::DividedProcess => {
            svg.rect(x, y, w, h, fill_attr);
            svg.line(x, y + 14.0, x + w, y + 14.0, &seam);
        }
        NodeShape::WindowPane => {
            svg.rect(x, y, w, h, fill_attr);
            svg.line(x + 12.0, y, x + 12.0, y + h, &seam);
            svg.line(x, y + 12.0, x + w, y + 12.0, &seam);
        }
        NodeShape::Triangle => {
            svg.path(
                &format!(
                    "M{} {} L{} {} L{} {} Z",
                    fnum(cx),
                    fnum(y),
                    fnum(x + w),
                    fnum(y + h),
                    fnum(x),
                    fnum(y + h),
                ),
                fill_attr,
            );
        }
        NodeShape::FlippedTriangle => {
            svg.path(
                &format!(
                    "M{} {} L{} {} L{} {} Z",
                    fnum(x),
                    fnum(y),
                    fnum(x + w),
                    fnum(y),
                    fnum(cx),
                    fnum(y + h),
                ),
                fill_attr,
            );
        }
        NodeShape::FilledCircle => {
            let r = w.min(h) / 2.0;
            svg.circle(
                cx,
                cy,
                r,
                &format!("fill=\"{stroke}\" stroke=\"{stroke}\" stroke-width=\"1.5\""),
            );
        }
        NodeShape::CrossedCircle => {
            let r = w.max(h) / 2.0;
            svg.circle(cx, cy, r, fill_attr);
            let d = r / std::f64::consts::SQRT_2;
            svg.line(cx - d, cy - d, cx + d, cy + d, &seam);
            svg.line(cx - d, cy + d, cx + d, cy - d, &seam);
        }
        NodeShape::PaperTape => {
            let a = 5.0;
            svg.path(
                &format!(
                    "M{} {} C{} {} {} {} {} {} L{} {} C{} {} {} {} {} {} Z",
                    fnum(x),
                    fnum(y + a),
                    fnum(x + 0.35 * w),
                    fnum(y - a),
                    fnum(x + 0.65 * w),
                    fnum(y + 3.0 * a),
                    fnum(x + w),
                    fnum(y + a),
                    fnum(x + w),
                    fnum(y + h - a),
                    fnum(x + 0.65 * w),
                    fnum(y + h + a),
                    fnum(x + 0.35 * w),
                    fnum(y + h - 3.0 * a),
                    fnum(x),
                    fnum(y + h - a),
                ),
                fill_attr,
            );
        }
        NodeShape::StoredData => {
            let c = 12.0;
            svg.path(
                &format!(
                    "M{} {} L{} {} Q{} {} {} {} L{} {} Q{} {} {} {} Z",
                    fnum(x),
                    fnum(y),
                    fnum(x + w),
                    fnum(y),
                    fnum(x + w - c),
                    fnum(cy),
                    fnum(x + w),
                    fnum(y + h),
                    fnum(x),
                    fnum(y + h),
                    fnum(x + c),
                    fnum(cy),
                    fnum(x),
                    fnum(y),
                ),
                fill_attr,
            );
        }
        _ => {
            svg.rect(x, y, w, h, &format!("{fill_attr} rx=\"4\""));
        }
    }
}

/// A rectangle with a wavy bottom edge, shared by the document family.
fn document_path(x: f64, y: f64, w: f64, h: f64, a: f64) -> String {
    format!(
        "M{} {} L{} {} L{} {} C{} {} {} {} {} {} Z",
        fnum(x),
        fnum(y),
        fnum(x + w),
        fnum(y),
        fnum(x + w),
        fnum(y + h - a),
        fnum(x + 0.75 * w),
        fnum(y + h),
        fnum(x + 0.25 * w),
        fnum(y + h - 2.0 * a),
        fnum(x),
        fnum(y + h - a),
    )
}

/// A curly brace: a vertical run at `bx` with a mid spike pointing `dir` (±1).
fn brace_path(bx: f64, top: f64, bot: f64, q: f64, dir: f64) -> String {
    let mid = (top + bot) / 2.0;
    format!(
        "M{} {} Q{} {} {} {} L{} {} Q{} {} {} {} Q{} {} {} {} L{} {} Q{} {} {} {}",
        fnum(bx + dir * q),
        fnum(top),
        fnum(bx),
        fnum(top),
        fnum(bx),
        fnum(top + q),
        fnum(bx),
        fnum(mid - q),
        fnum(bx),
        fnum(mid),
        fnum(bx - dir * q),
        fnum(mid),
        fnum(bx),
        fnum(mid),
        fnum(bx),
        fnum(mid + q),
        fnum(bx),
        fnum(bot - q),
        fnum(bx),
        fnum(bot),
        fnum(bx + dir * q),
        fnum(bot),
    )
}

/// Vertical cylinder outline (top ellipse cap, body, front seam), reused by the
/// disk shape which stacks an extra seam ring on top.
fn cylinder(svg: &mut SvgBuilder, x: f64, y: f64, w: f64, h: f64, fill_attr: &str, seam: &str) {
    svg.rect(x, y + 8.0, w, h - 16.0, fill_attr);
    svg.path(
        &format!(
            "M{} {} A{} {} 0 0 0 {} {} A{} {} 0 0 0 {} {}",
            fnum(x),
            fnum(y + 8.0),
            fnum(w / 2.0),
            fnum(8.0),
            fnum(x + w),
            fnum(y + 8.0),
            fnum(w / 2.0),
            fnum(8.0),
            fnum(x),
            fnum(y + 8.0),
        ),
        fill_attr,
    );
    svg.path(
        &format!(
            "M{} {} A{} {} 0 0 0 {} {}",
            fnum(x),
            fnum(y + h - 8.0),
            fnum(w / 2.0),
            fnum(8.0),
            fnum(x + w),
            fnum(y + h - 8.0),
        ),
        seam,
    );
}

/// Build a closed path from fractional `(fx, fy)` points inside the `w`×`h` box.
fn poly(x: f64, y: f64, w: f64, h: f64, pts: &[(f64, f64)]) -> String {
    let mut d = String::new();
    for (i, (fx, fy)) in pts.iter().enumerate() {
        let cmd = if i == 0 { 'M' } else { 'L' };
        d.push_str(&format!("{cmd}{} {} ", fnum(x + fx * w), fnum(y + fy * h)));
    }
    d.push('Z');
    d
}
