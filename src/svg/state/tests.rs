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
    // occluded the other ("sta… stop"). Each label is staggered along its
    // own arc, so the two background rects must be disjoint.
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
