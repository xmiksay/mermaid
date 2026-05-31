//! C4 diagram renderer. Grid layout of elements; boundaries draw a dashed
//! rectangle enclosing members.

use std::collections::BTreeMap;

use crate::parse::{C4Diagram, C4Element, C4ElementKind, C4Relation};

use super::builder::{escape, SvgBuilder};
use super::theme::Theme;

const PAD: f64 = 30.0;
const BOX_W: f64 = 180.0;
const BOX_H: f64 = 100.0;
const GAP_X: f64 = 60.0;
const GAP_Y: f64 = 40.0;
const COLS: usize = 3;
const TITLE_GAP: f64 = 32.0;

#[derive(Clone)]
struct Laid {
    el: C4Element,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    members: Vec<Laid>,
}

pub(crate) fn render(d: &C4Diagram, theme: &Theme) -> String {
    let fg = theme.fg;
    let title_h = if d.title.is_some() { TITLE_GAP } else { 0.0 };

    // Lay out top-level: grid 3 columns. Boundaries are sized to fit members.
    let (laid, total_w, total_h) = layout_group(&d.elements, PAD, PAD + title_h + 10.0);

    let width = (PAD * 2.0 + total_w).max(400.0);
    let height = (PAD * 2.0 + title_h + total_h + 30.0).max(200.0);
    let mut svg = SvgBuilder::new(width, height);

    if let Some(t) = &d.title {
        svg.text(
            width / 2.0,
            PAD + 18.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"18\" font-weight=\"bold\""),
            t,
        );
    }
    let _ = d.kind; // kind affects subtitle; omitted for brevity

    // Resolve aliases to centers (recursively).
    let mut centers: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    fn collect(laid: &[Laid], out: &mut BTreeMap<String, (f64, f64)>) {
        for l in laid {
            out.insert(l.el.alias.clone(), (l.x + l.w / 2.0, l.y + l.h / 2.0));
            collect(&l.members, out);
        }
    }
    collect(&laid, &mut centers);

    // Relations.
    for r in &d.relations {
        draw_rel(r, &centers, &mut svg, theme);
    }

    // Boxes.
    for l in &laid {
        draw_element(l, &mut svg, theme);
    }

    svg.finish()
}

fn layout_group(elements: &[C4Element], origin_x: f64, origin_y: f64) -> (Vec<Laid>, f64, f64) {
    let mut laid = Vec::new();
    let mut cur_x = origin_x;
    let mut cur_y = origin_y;
    let mut row_h: f64 = 0.0;
    let mut max_x: f64 = 0.0;
    let mut col = 0usize;

    for el in elements {
        let (w, h, members) = if el.boundary_kind.is_some() {
            let (members, mw, mh) = layout_group(&el.members, cur_x + 12.0, cur_y + 36.0);
            ((mw + 24.0).max(BOX_W), (mh + 48.0).max(BOX_H), members)
        } else {
            (BOX_W, BOX_H, Vec::new())
        };
        laid.push(Laid {
            el: el.clone(),
            x: cur_x,
            y: cur_y,
            w,
            h,
            members,
        });
        cur_x += w + GAP_X;
        row_h = row_h.max(h);
        max_x = max_x.max(cur_x - GAP_X);
        col += 1;
        if col >= COLS {
            col = 0;
            cur_x = origin_x;
            cur_y += row_h + GAP_Y;
            row_h = 0.0;
        }
    }
    let total_w = max_x - origin_x;
    let total_h = if col == 0 {
        cur_y - origin_y
    } else {
        cur_y - origin_y + row_h
    };
    (laid, total_w, total_h)
}

fn draw_element(l: &Laid, svg: &mut SvgBuilder, theme: &Theme) {
    let fg = theme.fg;
    let stroke = theme.flow_node_stroke;
    if l.el.boundary_kind.is_some() {
        svg.rect(l.x, l.y, l.w, l.h,
            &format!("fill=\"none\" stroke=\"{stroke}\" stroke-width=\"1.5\" stroke-dasharray=\"6 4\" rx=\"4\""));
        svg.text(
            l.x + 8.0,
            l.y + 18.0,
            &format!("fill=\"{fg}\" font-size=\"12\" font-weight=\"bold\""),
            &format!("[Boundary] {}", l.el.label),
        );
        for m in &l.members {
            draw_element(m, svg, theme);
        }
        return;
    }
    let (fill, label_kind) = c4_style(l.el.kind, l.el.external, theme);
    svg.rect(
        l.x,
        l.y,
        l.w,
        l.h,
        &format!("fill=\"{fill}\" stroke=\"{stroke}\" stroke-width=\"1.5\" rx=\"6\""),
    );
    svg.text(
        l.x + l.w / 2.0,
        l.y + 16.0,
        &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"10\" font-style=\"italic\""),
        label_kind,
    );
    svg.text(
        l.x + l.w / 2.0,
        l.y + 36.0,
        &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"13\" font-weight=\"bold\""),
        &l.el.label,
    );
    if let Some(t) = &l.el.technology {
        svg.text(
            l.x + l.w / 2.0,
            l.y + 54.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"10\""),
            &format!("[{}]", escape(t)),
        );
    }
    if let Some(d) = &l.el.descr {
        svg.text(
            l.x + l.w / 2.0,
            l.y + l.h - 14.0,
            &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"11\""),
            &truncate(d, 28),
        );
    }
}

fn draw_rel(
    r: &C4Relation,
    centers: &BTreeMap<String, (f64, f64)>,
    svg: &mut SvgBuilder,
    theme: &Theme,
) {
    let fg = theme.fg;
    let stroke = theme.flow_edge_stroke;
    let (Some(a), Some(b)) = (centers.get(&r.from), centers.get(&r.to)) else {
        return;
    };
    svg.line(
        a.0,
        a.1,
        b.0,
        b.1,
        &format!("stroke=\"{stroke}\" stroke-width=\"1.5\" stroke-dasharray=\"4 3\""),
    );
    let mx = (a.0 + b.0) / 2.0;
    let my = (a.1 + b.1) / 2.0;
    svg.rect(
        mx - 60.0,
        my - 11.0,
        120.0,
        22.0,
        &format!(
            "fill=\"{}\" stroke=\"{stroke}\" stroke-width=\"0.5\" rx=\"3\"",
            theme.flow_label_bg
        ),
    );
    svg.text(
        mx,
        my + 4.0,
        &format!("text-anchor=\"middle\" fill=\"{fg}\" font-size=\"10\""),
        &truncate(&r.label, 24),
    );
}

fn c4_style(kind: C4ElementKind, external: bool, theme: &Theme) -> (&'static str, &'static str) {
    let (fill, label) = match kind {
        C4ElementKind::Person => ("#08427B", "<<person>>"),
        C4ElementKind::System => ("#1168BD", "<<system>>"),
        C4ElementKind::SystemDb => ("#1168BD", "<<system database>>"),
        C4ElementKind::SystemQueue => ("#1168BD", "<<system queue>>"),
        C4ElementKind::Container => ("#438DD5", "<<container>>"),
        C4ElementKind::ContainerDb => ("#438DD5", "<<container db>>"),
        C4ElementKind::ContainerQueue => ("#438DD5", "<<container queue>>"),
        C4ElementKind::Component => ("#85BBF0", "<<component>>"),
        C4ElementKind::ComponentDb => ("#85BBF0", "<<component db>>"),
        C4ElementKind::ComponentQueue => ("#85BBF0", "<<component queue>>"),
        C4ElementKind::Node => ("#666", "<<node>>"),
    };
    let _ = theme;
    if external {
        ("#888", label)
    } else {
        (fill, label)
    }
}

fn truncate(s: &str, n: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= n {
        s.to_string()
    } else {
        let mut out: String = chars[..n.saturating_sub(1)].iter().collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::C4Kind;

    #[test]
    fn produces_svg() {
        let d = C4Diagram {
            kind: C4Kind::Context,
            title: Some("Sys".into()),
            elements: vec![C4Element {
                kind: C4ElementKind::Person,
                alias: "u".into(),
                label: "User".into(),
                descr: Some("Person".into()),
                technology: None,
                external: false,
                boundary_alias: None,
                boundary_label: None,
                boundary_kind: None,
                members: vec![],
            }],
            relations: vec![],
        };
        let svg = render(&d, &Theme::default());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains(">User<"));
        assert!(svg.contains(">Sys<"));
    }
}
