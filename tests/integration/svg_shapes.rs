//! Reading node shapes back as boxes: cluster frames and the bounding box of
//! every filled node shape, whatever its outline.

use crate::svg_read::{attr, body, tags, Kind, Node, Pt};

/// Subgraph cluster frames, which an edge to a cluster ends on.
pub(crate) fn frames(svg: &str) -> Vec<Node> {
    tags(body(svg), "<rect ")
        .filter(|t| t.contains("stroke-width=\"1\"") && t.contains("rx=\"6\""))
        .filter_map(|t| {
            let (x, y) = (attr(t, "x")?, attr(t, "y")?);
            Some(Node {
                b: (x, y, x + attr(t, "width")?, y + attr(t, "height")?),
                kind: Kind::Rect,
            })
        })
        .collect()
}

/// Bounding boxes `(x0, y0, x1, y1)` of every filled node shape — rects,
/// circles, ellipses and filled paths of any outline — leaving out cluster
/// frames, label backgrounds and notes. An arc's radii pad its end point, so
/// a box can only be larger than the shape.
pub(crate) fn shape_boxes(svg: &str) -> Vec<(f64, f64, f64, f64)> {
    let b = body(svg);
    let filled = |t: &str| {
        let frame = t.contains("stroke-width=\"1\"") && t.contains("rx=\"6\"");
        let label_or_note = t.contains("#e8e8e8") || t.to_ascii_lowercase().contains("#fff5ad");
        t.contains(" fill=\"") && !t.contains("fill=\"none\"") && !frame && !label_or_note
    };
    let mut out = Vec::new();
    for t in tags(b, "<rect ").filter(|t| filled(t)) {
        if let (Some(x), Some(y), Some(w), Some(h)) = (
            attr(t, "x"),
            attr(t, "y"),
            attr(t, "width"),
            attr(t, "height"),
        ) {
            out.push((x, y, x + w, y + h));
        }
    }
    for t in tags(b, "<circle ")
        .chain(tags(b, "<ellipse "))
        .filter(|t| filled(t))
    {
        let (Some(cx), Some(cy)) = (attr(t, "cx"), attr(t, "cy")) else {
            continue;
        };
        let r = attr(t, "r");
        let (rx, ry) = (
            r.or(attr(t, "rx")).unwrap_or(0.0),
            r.or(attr(t, "ry")).unwrap_or(0.0),
        );
        out.push((cx - rx, cy - ry, cx + rx, cy + ry));
    }
    for t in tags(b, "<path d=\"").filter(|t| filled(t)) {
        let d = &t[9..9 + t[9..].find('"').expect("d")];
        let mut pts: Vec<Pt> = Vec::new();
        let starts: Vec<usize> = d
            .match_indices(|c: char| c.is_ascii_alphabetic())
            .map(|(i, _)| i)
            .collect();
        for (k, &i) in starts.iter().enumerate() {
            let nums: Vec<f64> = d[i + 1..starts.get(k + 1).copied().unwrap_or(d.len())]
                .split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
                .filter_map(|n| n.parse().ok())
                .collect();
            if &d[i..=i] == "A" && nums.len() == 7 {
                let (rx, ry, x, y) = (nums[0], nums[1], nums[5], nums[6]);
                pts.extend([(x - rx, y - ry), (x + rx, y + ry)]);
            } else {
                pts.extend(
                    nums.chunks(2)
                        .filter(|c| c.len() == 2)
                        .map(|c| (c[0], c[1])),
                );
            }
        }
        if !pts.is_empty() {
            let xs = pts.iter().map(|p| p.0);
            let ys = pts.iter().map(|p| p.1);
            out.push((
                xs.clone().fold(f64::INFINITY, f64::min),
                ys.clone().fold(f64::INFINITY, f64::min),
                xs.fold(f64::NEG_INFINITY, f64::max),
                ys.fold(f64::NEG_INFINITY, f64::max),
            ));
        }
    }
    out
}
