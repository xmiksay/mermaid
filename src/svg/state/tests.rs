use super::*;
use crate::parse::parse;

fn build(s: &str) -> StateDiagram {
    match parse(s).unwrap() {
        crate::parse::Diagram::State(s) => s,
        _ => panic!("not state"),
    }
}

#[test]
fn renders_full_lifecycle() {
    let d = build("stateDiagram-v2\n[*] --> Idle\nIdle --> Running: go\nRunning --> [*]\n");
    let svg = render(&d, &Theme::default());
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains(">Idle<"));
    assert!(svg.contains(">Running<"));
    assert!(svg.contains(">go<"));
}

#[test]
fn start_and_end_drawn() {
    let d = build("stateDiagram-v2\n[*] --> A\nA --> [*]\n");
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("<circle"));
}

#[test]
fn style_applies_to_normal_state() {
    let d = build("stateDiagram-v2\n[*] --> A\nstyle A fill:#abc\n");
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("fill=\"#abc\""));
}

#[test]
fn history_states_rendered() {
    let d = build("stateDiagram-v2\nstate A {\n[*] --> B\nB --> [H]\n[H*] --> C\n}\n");
    let svg = render(&d, &Theme::default());
    assert!(svg.contains(">H<"));
    assert!(svg.contains(">H*<"));
}

#[test]
fn classdef_applies_to_state() {
    let d = build("stateDiagram-v2\n[*] --> A\nclassDef foo fill:#abc\nclass A foo\n");
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("fill=\"#abc\""));
}

#[test]
fn click_wraps_state_in_anchor() {
    let d = build("stateDiagram-v2\n[*] --> A\nclick A href \"https://x.test\"\n");
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("<a href=\"https://x.test\">"));
}

/// Bounds `(x, y, w, h)` of the composite frame rect (the solid
/// purple-bordered `rx="5"` rect drawn before its title band).
fn frame_rect(svg: &str) -> (f64, f64, f64, f64) {
    let key = "rx=\"5\"";
    let kpos = svg.find(key).expect("no composite frame");
    let open = svg[..kpos].rfind("<rect ").unwrap();
    let tag = &svg[open..kpos];
    let grab = |attr: &str| {
        let s = tag.find(attr).unwrap() + attr.len();
        let e = s + tag[s..].find('"').unwrap();
        tag[s..e].parse::<f64>().unwrap()
    };
    (
        grab("x=\""),
        grab("y=\""),
        grab("width=\""),
        grab("height=\""),
    )
}

/// Centre `(x, y)` of the `text-anchor=middle` label for `id`.
fn label_center(svg: &str, id: &str) -> (f64, f64) {
    let needle = format!(">{id}</text>");
    let end = svg.find(&needle).unwrap_or_else(|| panic!("no label {id}"));
    let open = svg[..end].rfind("<text ").unwrap();
    let tag = &svg[open..end];
    let grab = |attr: &str| {
        let s = tag.find(attr).unwrap() + attr.len();
        let e = s + tag[s..].find('"').unwrap();
        tag[s..e].parse::<f64>().unwrap()
    };
    (grab("x=\""), grab("y=\""))
}

#[test]
fn composite_frame_contains_its_members() {
    // Regression for #63: the composite's children must be laid out *inside*
    // its frame, not on a detached part of the canvas.
    let d = build("stateDiagram-v2\n[*] --> A\nstate A {\n[*] --> a1\n}\n");
    let svg = render(&d, &Theme::default());
    let (fx, fy, fw, fh) = frame_rect(&svg);
    let (ax, ay) = label_center(&svg, "a1");
    assert!(
        ax > fx && ax < fx + fw && ay > fy && ay < fy + fh,
        "member a1 ({ax},{ay}) must sit inside the frame ({fx},{fy},{fw},{fh})",
    );
    // The external `[*] --> A` transition still draws an arrow to the frame.
    assert!(svg.contains("marker-end=\"url(#state-arrow)\""));
}

#[test]
fn composite_not_drawn_as_standalone_node() {
    // The composite id `A` must not also be drawn as a small normal-state
    // rounded rect (the detached artifact the issue describes).
    let d = build("stateDiagram-v2\n[*] --> A\nstate A {\n[*] --> a1\n}\n");
    let svg = render(&d, &Theme::default());
    // Exactly one composite frame (one bold title), and its single member
    // `a1` is the only rounded normal-state node (`rx="10"`). A second would
    // mean `A` was also emitted as a standalone node. Frames use `rx="5"`.
    assert_eq!(svg.matches("font-weight=\"bold\"").count(), 1);
    assert_eq!(svg.matches("rx=\"10\"").count(), 1);
}

#[test]
fn composite_frame_not_clipped_by_top_edge() {
    // Regression for #242: a composite whose members sit at the top of the
    // layout must keep its title band inside the canvas — the frame top,
    // header included, stays at or below y=0 rather than being clipped.
    let d = build(
        "stateDiagram-v2\n[*] --> Idle\nstate Workflow {\n[*] --> Step1\nStep1 --> [*]\n}\nIdle --> Workflow\n",
    );
    let svg = render(&d, &Theme::default());
    let (_, fy, _, _) = frame_rect(&svg);
    assert!(fy >= 0.0, "composite frame top {fy} clipped above viewBox");
}

#[test]
fn composite_uses_solid_border_and_title_band() {
    // Issue #242 styling: solid (not dashed) frame plus a filled title band.
    let d = build("stateDiagram-v2\n[*] --> A\nstate A {\n[*] --> a1\n}\n");
    let svg = render(&d, &Theme::default());
    assert!(!svg.contains("stroke-dasharray=\"5 3\""));
    // Purple border and lavender title band, from the theme node colors.
    assert!(svg.contains("stroke=\"#9370DB\""));
    assert!(svg.contains("fill=\"#ECECFF\""));
}

#[test]
fn parallel_regions_stacked_with_divider() {
    // Two concurrent regions must render in disjoint vertical bands with a
    // dashed divider between them, not interleaved into one blob.
    let d =
        build("stateDiagram-v2\nstate Active {\n[*] --> NumLockOff\n--\n[*] --> CapsLockOff\n}\n");
    let svg = render(&d, &Theme::default());
    // One dashed region divider inside the frame.
    assert_eq!(svg.matches("stroke-dasharray=\"3 3\"").count(), 1);
    // The two regions' states sit in separate vertical bands.
    let (_, up) = label_center(&svg, "NumLockOff");
    let (_, down) = label_center(&svg, "CapsLockOff");
    assert!(
        (up - down).abs() > 20.0,
        "regions overlap vertically: {up} vs {down}",
    );
}

#[test]
fn opposite_transitions_render_distinctly() {
    // Regression for #241: `Idle --> Running` and `Running --> Idle` used to
    // collapse onto one segment, hiding the second label under the first.
    // Now caught by the general pairwise label overlap-avoidance pass
    // (`src/svg/label_layout.rs`), which nudges any two colliding labels
    // apart — not just a hand-coded opposite-pair special case.
    let d = build("stateDiagram-v2\nIdle --> Running : start\nRunning --> Idle : stop\n");
    let svg = render(&d, &Theme::default());
    // Two arrowheads, one per direction.
    assert_eq!(svg.matches("marker-end=\"url(#state-arrow)\"").count(), 2);
    // The two labels no longer share an anchor.
    let (sx, sy) = label_center(&svg, "start");
    let (tx, ty) = label_center(&svg, "stop");
    assert!(
        (sx - tx).abs() > 1.0 || (sy - ty).abs() > 1.0,
        "labels overlap: start ({sx},{sy}) vs stop ({tx},{ty})",
    );
}

/// Bounds `(x0, y0, x1, y1)` of the opaque background rect drawn directly
/// before the `text-anchor=middle` label for `id` (the `edgeLabelBackground`
/// rect, `fill="#fff" stroke="none"`).
fn label_bg_box(svg: &str, id: &str) -> (f64, f64, f64, f64) {
    let text = svg.find(&format!(">{id}</text>")).unwrap();
    let rect_open = svg[..text].rfind("<rect ").unwrap();
    let rect_end = rect_open + svg[rect_open..].find("/>").unwrap();
    let tag = &svg[rect_open..rect_end];
    let grab = |attr: &str| {
        let s = tag.find(attr).unwrap() + attr.len();
        let e = s + tag[s..].find('"').unwrap();
        tag[s..e].parse::<f64>().unwrap()
    };
    let (x, y, w, h) = (
        grab("x=\""),
        grab("y=\""),
        grab("width=\""),
        grab("height=\""),
    );
    (x, y, x + w, y + h)
}

#[test]
fn opposite_pair_label_backgrounds_do_not_overlap() {
    // Regression for #312: the opaque background of one opposite-pair label
    // occluded the other ("sta… stop"). The general pairwise overlap pass
    // (`label_layout::layout_edge_labels`) detects the collision and nudges
    // each label along its own arc, so the two background rects must be
    // disjoint — this is no longer a state-specific special case.
    let d = build("stateDiagram-v2\nIdle --> Running : start\nRunning --> Idle : stop\n");
    let svg = render(&d, &Theme::default());
    let (ax0, ay0, ax1, ay1) = label_bg_box(&svg, "start");
    let (bx0, by0, bx1, by1) = label_bg_box(&svg, "stop");
    let overlap = ax0 < bx1 && bx0 < ax1 && ay0 < by1 && by0 < ay1;
    assert!(
        !overlap,
        "label backgrounds overlap: start ({ax0},{ay0},{ax1},{ay1}) vs stop ({bx0},{by0},{bx1},{by1})",
    );
}

#[test]
fn pseudo_states_visible_on_dark_theme() {
    // Start/end dots used a hardcoded #333, near-invisible on the dark bg.
    let d = build("stateDiagram-v2\n[*] --> A\nA --> [*]\n");
    let svg = render(&d, &Theme::dark());
    assert!(svg.contains("fill=\"#E0E0E0\""));
    assert!(!svg.contains("#333"));
}

/// `(width, height)` from the root `<svg>` element's `viewBox`.
fn view_box(svg: &str) -> (f64, f64) {
    let s = svg.find("viewBox=\"").unwrap() + "viewBox=\"".len();
    let e = s + svg[s..].find('"').unwrap();
    let mut parts = svg[s..e]
        .split_whitespace()
        .map(|p| p.parse::<f64>().unwrap());
    let (_x0, _y0) = (parts.next().unwrap(), parts.next().unwrap());
    (parts.next().unwrap(), parts.next().unwrap())
}

/// Bounds `(x0, y0, x1, y1)` of the note rect (`fill="#FFF5AD"`).
fn note_rect(svg: &str) -> (f64, f64, f64, f64) {
    let key = "fill=\"#FFF5AD\"";
    let kpos = svg.find(key).expect("no note box");
    let open = svg[..kpos].rfind("<rect ").unwrap();
    let tag = &svg[open..kpos];
    let grab = |attr: &str| {
        let s = tag.find(attr).unwrap() + attr.len();
        let e = s + tag[s..].find('"').unwrap();
        tag[s..e].parse::<f64>().unwrap()
    };
    let (x, y, w, h) = (
        grab("x=\""),
        grab("y=\""),
        grab("width=\""),
        grab("height=\""),
    );
    (x, y, x + w, y + h)
}

/// Bounds of every composite frame's border rect (`fill="none"`, `rx="5"`),
/// in draw order — nested composites are pushed onto `d.composites` inner
/// first, so `rects[0]` is the innermost frame here. Scans `<rect ` tags
/// forward only (never searches backward from a later element, e.g. a
/// transition's `fill="none"` path, which could otherwise re-match an
/// already-visited rect and loop without `start` ever advancing).
fn all_frame_border_rects(svg: &str) -> Vec<(f64, f64, f64, f64)> {
    let mut rects = Vec::new();
    let mut start = 0;
    while let Some(rel) = svg[start..].find("<rect ") {
        let open = start + rel;
        let close = open + svg[open..].find("/>").unwrap();
        let tag = &svg[open..close];
        start = close + 2;
        if tag.contains("fill=\"none\"") && tag.contains("rx=\"5\"") {
            let grab = |attr: &str| {
                let s = tag.find(attr).unwrap() + attr.len();
                let e = s + tag[s..].find('"').unwrap();
                tag[s..e].parse::<f64>().unwrap()
            };
            let (x, y, w, h) = (
                grab("x=\""),
                grab("y=\""),
                grab("width=\""),
                grab("height=\""),
            );
            rects.push((x, y, x + w, y + h));
        }
    }
    rects
}

#[test]
fn note_box_does_not_crop_at_canvas_edges() {
    // Regression: `note left of`/`note right of` boxes are sized from their
    // (potentially long) text and positioned off the target state, so a note
    // near the diagram's edge can extend past the canvas the node-only sizing
    // pass computed. The canvas must grow/shift so the whole note fits.
    let d = build(
        "stateDiagram-v2\n[*] --> A\n\
         note left of A: this note's text is deliberately long enough that its measured box would spill past the left edge of a canvas sized from node positions alone\n",
    );
    let svg = render(&d, &Theme::default());
    let (width, height) = view_box(&svg);
    let (nx0, ny0, nx1, ny1) = note_rect(&svg);
    assert!(nx0 >= 0.0, "note left edge {nx0} clipped by canvas");
    assert!(ny0 >= 0.0, "note top edge {ny0} clipped by canvas");
    assert!(
        nx1 <= width,
        "note right edge {nx1} exceeds canvas width {width}"
    );
    assert!(
        ny1 <= height,
        "note bottom edge {ny1} exceeds canvas height {height}"
    );
}

#[test]
fn nested_composite_title_band_clears_parent_title_band() {
    // Regression: when a composite's very first (topmost) child is itself a
    // composite, the outer frame's box used to be measured straight through
    // to the deepest leaf states, so the inner frame's title band landed
    // right at the outer frame's own title band instead of below it.
    let d = build("stateDiagram-v2\nstate Outer {\nstate Inner {\n[*] --> A\n}\n}\n");
    let svg = render(&d, &Theme::default());
    let rects = all_frame_border_rects(&svg);
    assert_eq!(rects.len(), 2, "expected inner + outer composite frames");
    let (_, inner_y, _, _) = rects[0];
    let (_, outer_y, _, _) = rects[1];
    assert!(
        inner_y - outer_y >= 20.0,
        "inner frame top {inner_y} too close to outer frame top {outer_y}; \
         nested title bands would overlap",
    );
}
