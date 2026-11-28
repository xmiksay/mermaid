//! Reading a rendered flowchart's nodes back by structure, not colour: the
//! nodes are the shapes drawn after the edge paths, one node per run of
//! shapes closed by its text (an edge label's lone unstroked rect is not a
//! node), so `classDef`/`style` nodes (`fill:none` ones too), any font size
//! and every theme count. Each keeps its real outline
//! where that is cheap — rect (with its corner radius), circle,
//! straight-sided polygon — and its bounding box otherwise.

use crate::svg_read::{attr, body, Pt};

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Outline {
    Rect,
    Circle,
    /// Closed straight-sided path (rhombus, hexagon, parallelogram, …).
    Polygon(Vec<Pt>),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FlowNode {
    /// Bounding box `(x0, y0, x1, y1)`.
    pub(crate) b: (f64, f64, f64, f64),
    pub(crate) outline: Outline,
    /// A rect's corner radius.
    pub(crate) rx: f64,
}

fn dist_to_segment(p: Pt, a: Pt, b: Pt) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}

impl FlowNode {
    pub(crate) fn center(&self) -> Pt {
        ((self.b.0 + self.b.2) / 2.0, (self.b.1 + self.b.3) / 2.0)
    }

    /// Strictly inside the outline, more than `inset` px in from it.
    pub(crate) fn inside(&self, p: Pt, inset: f64) -> bool {
        let (x0, y0, x1, y1) = self.b;
        if !(p.0 > x0 + inset && p.0 < x1 - inset && p.1 > y0 + inset && p.1 < y1 - inset) {
            return false;
        }
        match &self.outline {
            Outline::Rect => true,
            Outline::Circle => {
                let c = self.center();
                (p.0 - c.0).hypot(p.1 - c.1) < (x1 - x0) / 2.0 - inset
            }
            Outline::Polygon(pts) => {
                let edges = || pts.iter().zip(pts.iter().cycle().skip(1));
                let crossings = edges()
                    .filter(|(a, b)| {
                        (a.1 > p.1) != (b.1 > p.1)
                            && p.0 < a.0 + (p.1 - a.1) * (b.0 - a.0) / (b.1 - a.1)
                    })
                    .count();
                crossings % 2 == 1 && edges().all(|(a, b)| dist_to_segment(p, *a, *b) > inset)
            }
        }
    }

    /// Signed distance from `p` to the outline: negative inside.
    pub(crate) fn sdist(&self, p: Pt) -> f64 {
        let (x0, y0, x1, y1) = self.b;
        let c = self.center();
        match &self.outline {
            Outline::Circle => (p.0 - c.0).hypot(p.1 - c.1) - (x1 - x0) / 2.0,
            Outline::Rect => {
                let r = self.rx.min((x1 - x0) / 2.0).min((y1 - y0) / 2.0).max(0.0);
                let q = (
                    (p.0 - c.0).abs() - ((x1 - x0) / 2.0 - r),
                    (p.1 - c.1).abs() - ((y1 - y0) / 2.0 - r),
                );
                q.0.max(0.0).hypot(q.1.max(0.0)) + q.0.max(q.1).min(0.0) - r
            }
            Outline::Polygon(pts) => {
                let edges = || pts.iter().zip(pts.iter().cycle().skip(1));
                let d = edges()
                    .map(|(a, b)| dist_to_segment(p, *a, *b))
                    .fold(f64::INFINITY, f64::min);
                if self.inside(p, 0.0) {
                    -d
                } else {
                    d
                }
            }
        }
    }

    /// Whether `p` is within `tol` of the bounding box's border.
    pub(crate) fn near_box(&self, p: Pt, tol: f64) -> bool {
        let (x0, y0, x1, y1) = self.b;
        p.0 > x0 - tol && p.0 < x1 + tol && p.1 > y0 - tol && p.1 < y1 + tol
    }
}

/// The numbers of an attribute-free path `d`, grouped per command letter.
fn commands(d: &str) -> Vec<(char, Vec<f64>)> {
    let mut out: Vec<(char, Vec<f64>)> = Vec::new();
    let mut num = String::new();
    let flush = |num: &mut String, out: &mut Vec<(char, Vec<f64>)>| {
        if let (Ok(v), Some(last)) = (num.parse::<f64>(), out.last_mut()) {
            last.1.push(v);
        }
        num.clear();
    };
    for ch in d.chars() {
        if ch.is_ascii_alphabetic() && ch != 'e' {
            flush(&mut num, &mut out);
            out.push((ch, Vec::new()));
        } else if ch == ' ' || ch == ',' {
            flush(&mut num, &mut out);
        } else if ch == '-' && !num.is_empty() && !num.ends_with('e') {
            flush(&mut num, &mut out);
            num.push(ch);
        } else {
            num.push(ch);
        }
    }
    flush(&mut num, &mut out);
    out
}

/// One drawn shape: its bounding box and outline.
fn shape(tag: &str) -> Option<FlowNode> {
    if tag.starts_with("<rect ") {
        let (x, y) = (attr(tag, "x")?, attr(tag, "y")?);
        let b = (x, y, x + attr(tag, "width")?, y + attr(tag, "height")?);
        return Some(FlowNode {
            b,
            outline: Outline::Rect,
            rx: attr(tag, "rx").unwrap_or(0.0),
        });
    }
    if tag.starts_with("<circle ") || tag.starts_with("<ellipse ") {
        let (cx, cy) = (attr(tag, "cx")?, attr(tag, "cy")?);
        let (rx, ry) = match attr(tag, "r") {
            Some(r) => (r, r),
            None => (attr(tag, "rx")?, attr(tag, "ry")?),
        };
        let b = (cx - rx, cy - ry, cx + rx, cy + ry);
        let outline = if rx == ry {
            Outline::Circle
        } else {
            Outline::Rect
        };
        return Some(FlowNode {
            b,
            outline,
            rx: 0.0,
        });
    }
    let d = tag.strip_prefix("<path d=\"")?;
    let cmds = commands(&d[..d.find('"')?]);
    let mut pts: Vec<Pt> = Vec::new();
    let mut straight = true;
    for (c, v) in &cmds {
        match c {
            'M' | 'L' => pts.extend(v.chunks_exact(2).map(|p| (p[0], p[1]))),
            'Z' => {}
            'C' => {
                straight = false;
                pts.extend(v.chunks_exact(2).map(|p| (p[0], p[1])));
            }
            'A' => {
                straight = false;
                for a in v.chunks_exact(7) {
                    // The renderer's arcs are half-ellipses bulging across
                    // their chord: pad the end across the chord only.
                    let (rx, ry, x, y) = (a[0].abs(), a[1].abs(), a[5], a[6]);
                    let from = pts.last().copied().unwrap_or((x, y));
                    let len = (x - from.0).hypot(y - from.1).max(1e-9);
                    let (px, py) = (
                        rx * ((y - from.1) / len).abs(),
                        ry * ((x - from.0) / len).abs(),
                    );
                    pts.extend([
                        (x - px, y - py),
                        (x + px, y + py),
                        (from.0 - px, from.1 - py),
                        (from.0 + px, from.1 + py),
                    ]);
                }
            }
            _ => return None,
        }
    }
    let inf = f64::INFINITY;
    let b = pts.iter().fold((inf, inf, -inf, -inf), |m, p| {
        (m.0.min(p.0), m.1.min(p.1), m.2.max(p.0), m.3.max(p.1))
    });
    let closed = cmds.last().is_some_and(|(c, _)| *c == 'Z');
    let outline = if straight && closed && pts.len() >= 3 {
        Outline::Polygon(pts)
    } else {
        Outline::Rect
    };
    (b.0 < b.2 && b.1 < b.3).then_some(FlowNode {
        b,
        outline,
        rx: 0.0,
    })
}

fn union(a: &FlowNode, b: &FlowNode) -> FlowNode {
    let concentric =
        a.outline == Outline::Circle && b.outline == Outline::Circle && a.center() == b.center();
    FlowNode {
        b: (
            a.b.0.min(b.b.0),
            a.b.1.min(b.b.1),
            a.b.2.max(b.b.2),
            a.b.3.max(b.b.3),
        ),
        outline: if concentric {
            Outline::Circle
        } else {
            Outline::Rect
        },
        rx: 0.0,
    }
}

fn touches(a: &FlowNode, b: &FlowNode) -> bool {
    a.b.0 <= b.b.2 + 0.5 && b.b.0 <= a.b.2 + 0.5 && a.b.1 <= b.b.3 + 0.5 && b.b.1 <= a.b.3 + 0.5
}

fn is_edge(tag: &str) -> bool {
    tag.starts_with("<path ") && tag.contains("fill=\"none\"") && tag.contains("stroke-width=\"")
}

/// Every flowchart node outline, in drawing order.
pub(crate) fn flow_nodes(svg: &str) -> Vec<FlowNode> {
    let b = body(svg);
    let tags: Vec<&str> = b
        .match_indices('<')
        .filter_map(|(i, _)| Some(&b[i..i + b[i..].find('>')?]))
        .filter(|t| !t.starts_with("</"))
        .collect();
    // Nodes follow the edge block (and the edge labels drawn after it).
    let Some(first) = tags.iter().position(|t| is_edge(t)) else {
        return Vec::new();
    };
    let after = first
        + tags[first..]
            .iter()
            .position(|t| !is_edge(t) && !t.starts_with("<animate"))
            .unwrap_or(tags.len() - first);
    let mut out = Vec::new();
    // The shapes so far of the run being read, and whether it is a lone
    // unstroked rect: an edge label's background (every node shape is
    // drawn with a stroke width, whatever its style).
    let mut pending: Option<FlowNode> = None;
    let mut label = false;
    for t in &tags[after..] {
        if t.starts_with("<text") {
            // A text closes the run of shapes before it.
            if let Some(node) = pending.take() {
                if !label {
                    out.push(node);
                }
            }
        } else if let Some(s) = shape(t) {
            let bare = t.starts_with("<rect ") && !t.contains("stroke-width=\"");
            pending = Some(match pending.take() {
                Some(p) if touches(&p, &s) => {
                    label = false;
                    union(&p, &s)
                }
                Some(p) => {
                    if !label {
                        out.push(p);
                    }
                    label = bare;
                    s
                }
                None => {
                    label = bare;
                    s
                }
            });
        }
    }
    if !label {
        out.extend(pending);
    }
    out
}

/// The node an edge tip lands on: the one whose box (2px out) holds it,
/// the nearest centre on a tie.
pub(crate) fn node_of_tip(nodes: &[FlowNode], p: Pt) -> Option<usize> {
    nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.near_box(p, 2.0))
        .min_by(|(_, a), (_, b)| {
            let d = |n: &FlowNode| (n.center().0 - p.0).hypot(n.center().1 - p.1);
            d(a).total_cmp(&d(b))
        })
        .map(|(i, _)| i)
}

/// Nodes are told from edge labels by structure, not by font size: a label
/// is a lone unstroked rect, a node's shape always has a stroke width.
#[test]
fn edge_labels_are_told_from_nodes_whatever_the_font_size() {
    let svg = concat!(
        "<svg><defs></defs>",
        "<path d=\"M0 0L100 0\" fill=\"none\" stroke-width=\"1.5\"/>",
        "<rect x=\"40\" y=\"-9\" width=\"20\" height=\"18\" fill=\"#e8e8e8\" stroke=\"none\"/>",
        "<text font-size=\"12\">lbl</text>",
        "<rect x=\"0\" y=\"20\" width=\"40\" height=\"20\" fill=\"#ececff\" stroke=\"#9370db\" stroke-width=\"1.5\"/>",
        "<text font-size=\"12\">n</text>",
        "</svg>"
    );
    let ns = flow_nodes(svg);
    assert_eq!(ns.len(), 1, "{ns:?}");
    assert_eq!(ns[0].b, (0.0, 20.0, 40.0, 40.0));
}
