use super::*;

#[test]
fn inline_style_overrides_theme_fill() {
    let svg = render(
        &parse_flow("flowchart TD\nA --> B\nstyle A fill:#f9f\n"),
        &Theme::default(),
    );
    assert!(svg.contains("fill=\"#f9f\""));
}

#[test]
fn classdef_applied_via_class() {
    let svg = render(
        &parse_flow("flowchart TD\nA --> B\nclassDef foo fill:#0f0\nclass A foo\n"),
        &Theme::default(),
    );
    assert!(svg.contains("fill=\"#0f0\""));
}

#[test]
fn default_classdef_styles_unclassed_node() {
    let svg = render(
        &parse_flow("flowchart TD\nA --> B\nclassDef default fill:#eee\n"),
        &Theme::default(),
    );
    assert!(svg.contains("fill=\"#eee\""));
}

#[test]
fn link_style_overrides_edge_stroke() {
    let svg = render(
        &parse_flow("flowchart TD\nA --> B\nlinkStyle 0 stroke:#ff3,stroke-width:4px\n"),
        &Theme::default(),
    );
    assert!(svg.contains("stroke=\"#ff3\""));
    assert!(svg.contains("stroke-width=\"4\""));
}

#[test]
fn color_prop_sets_label_fill() {
    let svg = render(
        &parse_flow("flowchart TD\nA --> B\nstyle A color:#fff\n"),
        &Theme::default(),
    );
    assert!(svg.contains("fill=\"#fff\""));
}

#[test]
fn font_weight_style_and_opacity_pass_through() {
    let svg = render(
        &parse_flow(
            "flowchart TD\nA --> B\nstyle A font-weight:bold,font-style:italic,opacity:0.5\n",
        ),
        &Theme::default(),
    );
    assert!(svg.contains("font-weight=\"bold\""));
    assert!(svg.contains("font-style=\"italic\""));
    assert!(svg.contains("opacity=\"0.5\""));
}

#[test]
fn curved_edges_use_bezier() {
    // The skip edge a→d spans multiple layers, so it routes through ≥3
    // waypoints and emits a cubic-bezier `C` command in its path.
    let svg = render(
        &parse_flow("flowchart TD\na --> b --> c --> d\na --> d\n"),
        &Theme::default(),
    );
    assert!(any_bezier_path(&svg));
}

#[test]
fn click_href_wraps_node_in_anchor() {
    let svg = render(
        &parse_flow("flowchart TD\nA-->B\nclick A \"https://example.com\" \"go\"\n"),
        &Theme::default(),
    );
    assert!(svg.contains("<a href=\"https://example.com\">"));
    assert!(svg.contains("<title>go</title>"));
    assert!(svg.contains("</a>"));
}

#[test]
fn click_href_target_renders_attribute() {
    let svg = render(
        &parse_flow("flowchart TD\nA-->B\nclick A href \"http://x\" \"t\" _blank\n"),
        &Theme::default(),
    );
    assert!(svg.contains("target=\"_blank\""));
}

#[test]
fn click_callback_emits_onclick() {
    let svg = render(
        &parse_flow("flowchart TD\nA-->B\nclick A doThing \"hint\"\n"),
        &Theme::default(),
    );
    assert!(svg.contains("onclick=\"doThing()\""));
    assert!(svg.contains("class=\"clickable\""));
    assert!(svg.contains("<title>hint</title>"));
}

#[test]
fn non_clickable_node_has_no_anchor() {
    let svg = render(&parse_flow("flowchart TD\nA-->B\n"), &Theme::default());
    assert!(!svg.contains("<a "));
    assert!(!svg.contains("onclick"));
}

#[test]
fn v11_shapes_render_distinct_geometry() {
    // Every listed v11 shape must render without panicking and produce its own
    // outline (not a plain rect fallback). We assert a few signature marks.
    let src = "flowchart TD\n\
               A@{ shape: doc } --> B@{ shape: hourglass }\n\
               B --> C@{ shape: tri, label: \"t\" }\n\
               C --> D@{ shape: cross-circ }\n\
               D --> E@{ shape: delay }\n\
               E --> F@{ shape: comment, label: \"c\" }\n";
    let svg = render(&parse_flow(src), &Theme::default());
    assert!(svg.starts_with("<svg"));
    // Document's wavy bottom is a cubic (`C`) path.
    assert!(any_bezier_path(&svg));
    // cross-circ draws a circle plus a diagonal cross.
    assert!(svg.contains("<circle"));
    // Comment draws no body fill, only brace strokes (quadratics).
    assert!(svg.contains("Q"));
}

#[test]
fn adjacent_layer_edge_stays_straight() {
    // A single short edge whose endpoints share a column clips to 2 aligned
    // points → straight M..L.., no curve.
    let svg = render(&parse_flow("flowchart TD\na --> b\n"), &Theme::default());
    assert!(!any_bezier_path(&svg));
}

#[test]
fn offset_adjacent_edge_curves() {
    // #331: an adjacent-layer edge between horizontally offset nodes leaves and
    // re-enters along the flow axis as a gentle basis curve (cubic `C`), not a
    // straight diagonal. `a` fans out to two side-by-side children, so `a -> b`
    // and `a -> c` are both offset.
    let svg = render(
        &parse_flow("flowchart TD\na --> b\na --> c\n"),
        &Theme::default(),
    );
    assert!(any_bezier_path(&svg));
}

#[test]
fn edge_attr_animate_emits_smil_animation() {
    let svg = render(
        &parse_flow("flowchart TD\nA e1@--> B\ne1@{ animate: true }\n"),
        &Theme::default(),
    );
    assert!(svg.contains("<animate attributeName=\"stroke-dashoffset\""));
    assert!(svg.contains("repeatCount=\"indefinite\""));
    // Needs a dash pattern to make the flow visible.
    assert!(svg.contains("stroke-dasharray=\"8 8\""));
}

#[test]
fn link_style_interpolate_linear_removes_curve() {
    // The skip edge a→d curves under the default basis...
    let basis = render(
        &parse_flow("flowchart TD\na --> b --> c --> d\na --> d\n"),
        &Theme::default(),
    );
    assert!(any_bezier_path(&basis));
    // ...and becomes straight segments (no cubic `C`) under linear interpolate.
    let linear = render(
        &parse_flow(
            "flowchart TD\na --> b --> c --> d\na --> d\nlinkStyle default interpolate linear\n",
        ),
        &Theme::default(),
    );
    assert!(!any_bezier_path(&linear));
}

#[test]
fn subgraph_with_local_direction_stays_within_canvas() {
    // #368: a subgraph whose local `direction` differs from the diagram's
    // global one can transpose its members further left than any other
    // node's left edge; the canvas's left offset must expand to cover the
    // subgraph frame (with its own padding) too, or the frame/label/nodes
    // clip at x=0.
    let src = "flowchart TD\n\
               Intake --> Router\n\
               Router --> Retry1\n\
               Router --> Step1\n\
               subgraph Fallback\n\
               direction LR\n\
               Retry1 --> Retry2\n\
               end\n\
               subgraph Pipeline\n\
               direction LR\n\
               Step1 --> Step2 --> Step3\n\
               end\n\
               Step3 --> Archive\n";
    let svg = render(&parse_flow(src), &Theme::default());
    assert!(
        !has_negative_coordinate(&svg),
        "content rendered outside the canvas (negative coordinate found)"
    );
}

#[test]
fn nested_subgraph_transposed_by_outer_local_direction_stays_within_outer_frame() {
    // An outer subgraph's local `direction` (differing from the diagram's
    // global one) must rigidly carry every recursively-nested descendant
    // node along when it transposes, not just its own direct members —
    // otherwise the outer frame (sized from recursive membership) and the
    // inner subgraph's actual node positions (previously moved only if a
    // direct member) disagree, leaving the inner subgraph's nodes outside
    // the outer frame it's supposed to be nested inside.
    let src = "flowchart TD\n\
               subgraph Backend\n\
               direction LR\n\
               Auth --> Users\n\
               subgraph Storage\n\
               direction TB\n\
               Users --> Cache\n\
               Cache --> DB\n\
               end\n\
               end\n";
    let svg = render(&parse_flow(src), &Theme::default());
    let (bx0, by0, bx1, by1) = label_bg_box(&svg, "Backend");
    for id in ["Auth", "Users", "Cache", "DB"] {
        let (cx, cy) = label_center(&svg, id);
        assert!(
            cx >= bx0 && cx <= bx1 && cy >= by0 && cy <= by1,
            "{id} at ({cx}, {cy}) falls outside Backend's frame ({bx0}, {by0})-({bx1}, {by1})"
        );
    }
}

#[test]
fn back_edge_label_stays_within_canvas_on_the_right() {
    // #368: a routed back-edge (e.g. `Retry? --> Start`) curves around to the
    // right of every node in the sugiyama layout; its label must expand the
    // canvas width, not just node right edges, or the label's opaque
    // background/text clip at the canvas's right edge.
    let src = "flowchart TD\n\
               Start{Start} -->|yes| A{Continue?}\n\
               Start -->|no| Stop[Stop]\n\
               A -->|yes| B{Retry?}\n\
               A -->|no| End1[Done]\n\
               B -->|yes| Start\n\
               B -->|no| End2[Abort]\n";
    let svg = render(&parse_flow(src), &Theme::default());
    let (width, height) = viewbox_size(&svg);
    assert!(
        all_label_rects_within_bounds(&svg, width, height),
        "an edge label rendered outside the canvas"
    );
}

#[test]
fn edge_attr_curve_step_is_honored() {
    // `curve: step` renders the multi-rank skip edge as orthogonal right-angle
    // segments — a path with several `L` commands and no cubic bezier `C`.
    // (Sibling basis edges may curve; this asserts the step edge specifically.)
    let svg = render(
        &parse_flow("flowchart TD\na --> b --> c --> d\na e1@--> d\ne1@{ curve: step }\n"),
        &Theme::default(),
    );
    assert!(svg.starts_with("<svg"));
    let has_step_edge = svg.split("d=\"").skip(1).any(|seg| {
        let d = &seg[..seg.find('"').unwrap_or(seg.len())];
        !d.contains('C') && d.matches('L').count() >= 4
    });
    assert!(
        has_step_edge,
        "step edge should be orthogonal (many Ls, no C)"
    );
}
