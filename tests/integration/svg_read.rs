//! Reading a rendered SVG back into node outlines, edge paths and marker
//! sizes, for the geometric edge tests.

use std::collections::HashMap;

pub(crate) type Pt = (f64, f64);

pub(crate) fn attr(tag: &str, name: &str) -> Option<f64> {
    let key = format!(" {name}=\"");
    let s = tag.find(&key)? + key.len();
    let e = s + tag[s..].find('"')?;
    tag[s..e].parse().ok()
}

fn str_attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let s = tag.find(&key)? + key.len();
    let e = s + tag[s..].find('"')?;
    Some(&tag[s..e])
}

pub(crate) fn tags<'a>(svg: &'a str, open: &'a str) -> impl Iterator<Item = &'a str> + 'a {
    svg.match_indices(open).map(move |(i, _)| {
        let end = i + svg[i..].find('>').expect("closed tag");
        &svg[i..end]
    })
}

fn numbers(d: &str) -> Vec<Pt> {
    let v: Vec<f64> = d
        .split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
        .filter_map(|t| t.parse().ok())
        .collect();
    v.chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| (c[0], c[1]))
        .collect()
}

/// The drawing, without `<defs>`.
pub(crate) fn body(svg: &str) -> &str {
    &svg[svg.find("</defs>").map_or(0, |i| i + 7)..]
}

/// The `viewBox` width and height.
pub(crate) fn view_box(svg: &str) -> (f64, f64) {
    let open = &svg[..svg.find('>').expect("svg tag")];
    let vb = str_attr(open, "viewBox").expect("viewBox");
    let v: Vec<f64> = vb.split(' ').map(|t| t.parse().expect("number")).collect();
    (v[2], v[3])
}

/// A marker def's reach back along the edge and width across it, both in
/// user units for a 1px stroke.
pub(crate) fn markers(svg: &str) -> HashMap<String, (f64, f64)> {
    tags(svg, "<marker ")
        .filter_map(|t| {
            let vb = str_attr(t, "viewBox")?;
            let vb_w: f64 = vb.split(' ').nth(2)?.parse().ok()?;
            let (w, h) = (attr(t, "markerWidth")?, attr(t, "markerHeight")?);
            let id = str_attr(t, "id")?.to_string();
            Some((id, (attr(t, "refX")? * w / vb_w, h)))
        })
        .collect()
}

/// One edge as drawn.
pub(crate) struct Drawn {
    /// Path points, bezier control points included.
    pub(crate) raw: Vec<Pt>,
    /// The path flattened (each cubic sampled at 32 steps).
    pub(crate) curve: Vec<Pt>,
    /// Marker ids at the start and the end.
    pub(crate) marks: [Option<String>; 2],
    pub(crate) stroke: f64,
}

/// Every edge path: unfilled, with a stroke width (node outlines are filled).
pub(crate) fn drawn(svg: &str) -> Vec<Drawn> {
    tags(body(svg), "<path d=\"")
        .filter(|t| t.contains("fill=\"none\"") && t.contains("stroke-width=\""))
        .map(|t| {
            let d = &t[9..9 + t[9..].find('"').expect("d")];
            let mark = |k: &str| {
                let v = str_attr(t, k)?;
                Some(
                    v.trim_start_matches("url(#")
                        .trim_end_matches(')')
                        .to_string(),
                )
            };
            Drawn {
                raw: numbers(d),
                curve: flatten(d),
                marks: [mark("marker-start"), mark("marker-end")],
                // An unusable width (`inf`, `NaN`, negative) falls back to
                // SVG's initial value, as a browser draws it.
                stroke: attr(t, "stroke-width").map_or(1.5, |w| {
                    if w.is_finite() && w >= 0.0 {
                        w
                    } else {
                        1.0
                    }
                }),
            }
        })
        .filter(|d| d.raw.len() >= 2)
        .collect()
}

fn flatten(d: &str) -> Vec<Pt> {
    let mut out: Vec<Pt> = Vec::new();
    let starts: Vec<usize> = d.match_indices(['M', 'L', 'C']).map(|(i, _)| i).collect();
    for (k, &i) in starts.iter().enumerate() {
        let pts = numbers(&d[i + 1..starts.get(k + 1).copied().unwrap_or(d.len())]);
        match (&d[i..=i], out.last().copied()) {
            ("C", Some(p0)) if pts.len() == 3 => {
                let (c1, c2, p) = (pts[0], pts[1], pts[2]);
                for s in 1..=32 {
                    let t = s as f64 / 32.0;
                    let u = 1.0 - t;
                    let (a, b, c, e) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    out.push((
                        a * p0.0 + b * c1.0 + c * c2.0 + e * p.0,
                        a * p0.1 + b * c1.1 + c * c2.1 + e * p.1,
                    ));
                }
            }
            _ => out.extend(pts),
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Kind {
    Rect,
    Circle,
    Rhombus,
}

/// A node outline: its bounding box `(x0, y0, x1, y1)` and shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Node {
    pub(crate) b: (f64, f64, f64, f64),
    pub(crate) kind: Kind,
}

impl Node {
    pub(crate) fn center(&self) -> Pt {
        ((self.b.0 + self.b.2) / 2.0, (self.b.1 + self.b.3) / 2.0)
    }

    /// Whether `p` lies on the outline (within 1px).
    pub(crate) fn on_outline(&self, p: Pt) -> bool {
        let (cx, cy) = self.center();
        let (hw, hh) = ((self.b.2 - self.b.0) / 2.0, (self.b.3 - self.b.1) / 2.0);
        let (dx, dy) = ((p.0 - cx).abs(), (p.1 - cy).abs());
        match self.kind {
            Kind::Circle => (dx.hypot(dy) - hw).abs() < 1.0,
            Kind::Rhombus => {
                let scale = hw.hypot(hh) / (hw * hh);
                ((dx / hw + dy / hh - 1.0) / scale).abs() < 1.0
            }
            Kind::Rect => {
                (dx <= hw + 1.0 && (dy - hh).abs() < 1.0)
                    || (dy <= hh + 1.0 && (dx - hw).abs() < 1.0)
            }
        }
    }
}

/// The node outlines: purple-stroked filled rects (touching ones — class
/// compartments, ER cells — merged into one box), fork/join bars, circles
/// larger than a cardinality glyph, and rhombus paths. A composite state's
/// title band is part of its frame, not a node.
pub(crate) fn nodes(svg: &str) -> Vec<Node> {
    let b = body(svg);
    let frames: Vec<(f64, f64, f64)> = tags(b, "<rect ")
        .filter(|t| t.contains("fill=\"none\""))
        .filter_map(|t| Some((attr(t, "x")?, attr(t, "y")?, attr(t, "width")?)))
        .collect();
    let mut rects: Vec<(f64, f64, f64, f64)> = tags(b, "<rect ")
        .filter(|t| {
            (t.contains("stroke=\"#9370DB\"") && !t.contains("fill=\"none\""))
                || (t.contains("fill=\"#333\"") && t.contains("stroke=\"none\""))
        })
        .filter_map(|t| {
            let (x, y, w) = (attr(t, "x")?, attr(t, "y")?, attr(t, "width")?);
            (!frames.contains(&(x, y, w))).then_some((x, y, x + w, y + attr(t, "height")?))
        })
        .collect();
    // Merge touching rects until stable.
    let touch = |a: &(f64, f64, f64, f64), o: &(f64, f64, f64, f64)| {
        a.0 <= o.2 + 0.5 && o.0 <= a.2 + 0.5 && a.1 <= o.3 + 0.5 && o.1 <= a.3 + 0.5
    };
    let mut merged = true;
    while merged {
        merged = false;
        'outer: for i in 0..rects.len() {
            for j in i + 1..rects.len() {
                if touch(&rects[i], &rects[j]) {
                    let (a, o) = (rects[i], rects[j]);
                    rects[i] = (a.0.min(o.0), a.1.min(o.1), a.2.max(o.2), a.3.max(o.3));
                    rects.remove(j);
                    merged = true;
                    break 'outer;
                }
            }
        }
    }
    let mut out: Vec<Node> = rects
        .into_iter()
        .map(|b| Node {
            b,
            kind: Kind::Rect,
        })
        .collect();
    for t in tags(b, "<circle ") {
        let (Some(cx), Some(cy), Some(r)) = (attr(t, "cx"), attr(t, "cy"), attr(t, "r")) else {
            continue;
        };
        if r > 6.0
            && !out
                .iter()
                .any(|n| n.kind == Kind::Circle && n.center() == (cx, cy))
        {
            let b = (cx - r, cy - r, cx + r, cy + r);
            out.push(Node {
                b,
                kind: Kind::Circle,
            });
        }
    }
    for t in tags(b, "<path d=\"M").filter(|t| t.contains("Z\" fill=\"#")) {
        let p = numbers(&t[9..9 + t[9..].find('"').expect("d")]);
        if p.len() == 4 {
            let xs = p.iter().map(|q| q.0);
            let ys = p.iter().map(|q| q.1);
            let b = (
                xs.clone().fold(f64::INFINITY, f64::min),
                ys.clone().fold(f64::INFINITY, f64::min),
                xs.fold(f64::NEG_INFINITY, f64::max),
                ys.fold(f64::NEG_INFINITY, f64::max),
            );
            out.push(Node {
                b,
                kind: Kind::Rhombus,
            });
        }
    }
    out
}

/// The node whose outline `p` lies on, if exactly one.
pub(crate) fn node_at(nodes: &[Node], p: Pt) -> Option<usize> {
    let mut hits = nodes.iter().enumerate().filter(|(_, n)| n.on_outline(p));
    let first = hits.next()?.0;
    hits.next().is_none().then_some(first)
}

/// Every edge-geometry repro under `tests/fixtures/edges/`, by file name.
pub(crate) fn fixtures() -> Vec<(String, String)> {
    let dir = format!("{}/tests/fixtures/edges", env!("CARGO_MANIFEST_DIR"));
    let mut out: Vec<(String, String)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{dir}: {e}"))
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "mmd"))
        .map(|p| {
            let name = p.file_name().expect("name").to_string_lossy().into_owned();
            (name, std::fs::read_to_string(&p).expect("read fixture"))
        })
        .collect();
    out.sort();
    out
}

pub(crate) fn fixture(name: &str) -> String {
    let path = format!(
        "{}/tests/fixtures/edges/{name}.mmd",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// An edge label: its background `(x0, y0, x1, y1)` and its text.
pub(crate) type Label = ((f64, f64, f64, f64), String);

/// Every edge label: an `#e8e8e8` background and the text drawn after it.
pub(crate) fn labels(svg: &str) -> Vec<Label> {
    let b = body(svg);
    b.match_indices("<rect ")
        .filter_map(|(i, _)| {
            let t = &b[i..i + b[i..].find('>')?];
            if !t.contains("fill=\"#e8e8e8\"") {
                return None;
            }
            let (x, y) = (attr(t, "x")?, attr(t, "y")?);
            let rest = &b[i..];
            let text = &rest[rest.find("<text")?..];
            let text = &text[..text.find("</text>")?];
            let mut plain = String::new();
            let mut inside = false;
            for c in text.chars() {
                match c {
                    '<' => inside = true,
                    '>' => inside = false,
                    _ if !inside => plain.push(c),
                    _ => {}
                }
            }
            Some(((x, y, x + attr(t, "width")?, y + attr(t, "height")?), plain))
        })
        .collect()
}
