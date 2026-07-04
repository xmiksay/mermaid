use super::*;
use crate::parse::parse;

fn parse_flow(s: &str) -> FlowchartDiagram {
    match parse(s).unwrap() {
        crate::parse::Diagram::Flowchart(f) => f,
        _ => panic!("expected flowchart"),
    }
}

#[test]
fn renders_basic_td() {
    let svg = render(
        &parse_flow("flowchart TD\nA --> B --> C\n"),
        &Theme::default(),
    );
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("A"));
    assert!(svg.contains("C"));
}

#[test]
fn frontmatter_title_is_drawn() {
    let d = parse_flow("---\ntitle: My Flow\n---\nflowchart TD\nA --> B\n");
    assert_eq!(d.title.as_deref(), Some("My Flow"));
    let svg = render(&d, &Theme::default());
    assert!(svg.contains(">My Flow</text>"));
    assert!(svg.contains("font-weight=\"bold\""));
}

#[test]
fn edge_label_appears() {
    let svg = render(
        &parse_flow("flowchart TD\nA -->|yes| B\n"),
        &Theme::default(),
    );
    assert!(svg.contains(">yes<"));
}

#[test]
fn dotted_edge_uses_dasharray() {
    let svg = render(&parse_flow("flowchart TD\nA -.-> B\n"), &Theme::default());
    assert!(svg.contains("stroke-dasharray=\"2 4\""));
}

#[test]
fn invisible_link_draws_no_edge_path() {
    // The invisible edge must add no drawn path: the SVG for `A ~~~ B` plus
    // one visible edge has the same edge-path count as the visible edge
    // alone.
    let with_inv = render(
        &parse_flow("flowchart TD\nA --> B\nA ~~~ C\nC --> B\n"),
        &Theme::default(),
    );
    let without_inv = render(
        &parse_flow("flowchart TD\nA --> B\nC --> B\n"),
        &Theme::default(),
    );
    assert_eq!(
        with_inv.matches("<path").count(),
        without_inv.matches("<path").count(),
    );
}

#[test]
fn circle_head_marker_used() {
    let svg = render(&parse_flow("flowchart TD\nA --o B\n"), &Theme::default());
    assert!(svg.contains("arrow-circle"));
}

#[test]
fn cross_head_marker_used() {
    let svg = render(&parse_flow("flowchart TD\nA --x B\n"), &Theme::default());
    assert!(svg.contains("arrow-cross"));
}

#[test]
fn solid_no_arrow_omits_marker() {
    let svg = render(&parse_flow("flowchart TD\nA --- B\n"), &Theme::default());
    assert_eq!(svg.matches("marker-end=").count(), 0);
}

#[test]
fn bidirectional_edge_emits_start_and_end_markers() {
    let svg = render(&parse_flow("flowchart LR\nA <--> B\n"), &Theme::default());
    assert!(svg.contains("marker-start=\"url(#arrow-filled)\""));
    assert!(svg.contains("marker-end=\"url(#arrow-filled)\""));
}

#[test]
fn subgraph_frame_drawn() {
    let svg = render(
        &parse_flow("flowchart TD\nA --> B\nsubgraph S [Group]\nB --> C\nend\n"),
        &Theme::default(),
    );
    // Themed cluster frame + centered bold label.
    assert!(svg.contains("fill=\"#ffffde\""));
    assert!(svg.contains("font-weight=\"bold\""));
    assert!(svg.contains(">Group<"));
}

/// Centre `(x, y)` of the single-line node label `id` (`text-anchor
/// middle` places the `<text>` x at the node's centre).
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
fn subgraph_local_direction_transposes_members() {
    // Under TD the chain A→B stacks vertically (same x, B below A).
    let td = render(
        &parse_flow("flowchart TD\nsubgraph S\nA --> B\nend\n"),
        &Theme::default(),
    );
    let (ax, ay) = label_center(&td, "A");
    let (bx, by) = label_center(&td, "B");
    assert!((ax - bx).abs() < 1.0, "TD members should share a column");
    assert!(by > ay, "TD flows top-to-bottom");

    // `direction LR` inside the subgraph lays the same members side by side.
    let lr = render(
        &parse_flow("flowchart TD\nsubgraph S\ndirection LR\nA --> B\nend\n"),
        &Theme::default(),
    );
    let (ax, ay) = label_center(&lr, "A");
    let (bx, by) = label_center(&lr, "B");
    assert!((ay - by).abs() < 1.0, "LR members should share a row");
    assert!(bx > ax, "LR flows left-to-right");
}

#[test]
fn edge_to_subgraph_id_routes_to_box() {
    let svg = render(
        &parse_flow("flowchart TD\nsubgraph SG [Group]\nA --> B\nend\nC --> SG\n"),
        &Theme::default(),
    );
    // Cluster frame titled by its label, no phantom `SG` node, and the C→SG
    // edge is drawn (an arrow-headed path) rather than silently dropped.
    assert!(svg.contains("fill=\"#ffffde\""));
    assert!(svg.contains(">Group</text>"));
    assert!(!svg.contains(">SG</text>"));
    assert!(svg.contains("marker-end=\"url(#arrow-filled)\""));
}

/// Grab the `rx` value of the first `<rect>` whose text label is `id`.
fn node_rect_rx(svg: &str, id: &str) -> f64 {
    // The node rect is emitted just before its label; find the label, then
    // the nearest preceding `rx="…"`.
    let label = format!(">{id}</text>");
    let end = svg.find(&label).unwrap_or_else(|| panic!("no label {id}"));
    let rx_at = svg[..end].rfind("rx=\"").unwrap() + 4;
    let e = rx_at + svg[rx_at..].find('"').unwrap();
    svg[rx_at..e].parse::<f64>().unwrap()
}

#[test]
fn round_and_stadium_render_differently() {
    // Round `()` is a small-radius rect; stadium `([])` is a full pill.
    let svg = render(
        &parse_flow("flowchart LR\nR(round) --> S([stadium])\n"),
        &Theme::default(),
    );
    let round_rx = node_rect_rx(&svg, "round");
    let stadium_rx = node_rect_rx(&svg, "stadium");
    assert_eq!(round_rx, 5.0, "round is a small-radius rect");
    assert!(stadium_rx > round_rx, "stadium is a pill (rx = h/2)");
}

#[test]
fn subgraph_style_directive_colors_frame() {
    let svg = render(
        &parse_flow(
            "flowchart TD\nsubgraph S [Group]\nA --> B\nend\nstyle S fill:#f9f,stroke:#111\n",
        ),
        &Theme::default(),
    );
    assert!(svg.contains("fill=\"#f9f\""));
    assert!(svg.contains("stroke=\"#111\""));
}

#[test]
fn all_asymmetric_shapes_render() {
    let svg = render(&parse_flow(
        "flowchart TD\nA[/par/] --> B[\\palt\\]\nB --> C[/trap\\]\nC --> D[\\tralt/]\nD --> E>flag]\n",
    ), &Theme::default());
    assert!(svg.starts_with("<svg"));
}

#[test]
fn node_label_br_splits_into_lines() {
    let svg = render(
        &parse_flow("flowchart TB\nPX[\"line one<br/>line two<br/>line three\"]\n"),
        &Theme::default(),
    );
    // Lines stacked as <tspan>s, none containing literal <br> markup.
    assert_eq!(svg.matches("line one").count(), 1);
    assert_eq!(svg.matches("line three").count(), 1);
    assert_eq!(svg.matches("<tspan").count(), 3);
    assert!(!svg.contains("&lt;br"));
    assert!(!svg.contains("<br"));
}

#[test]
fn inline_style_carries_across_br_in_node_label() {
    // #221: a tag opened before a `<br>` must keep styling the text after it.
    let svg = render(
        &parse_flow("flowchart TD\nA[\"<b>line1<br>line2</b>\"]\n"),
        &Theme::default(),
    );
    assert_eq!(svg.matches("font-weight=\"bold\"").count(), 2);
}

#[test]
fn empty_flowchart_still_valid_svg() {
    let svg = render(&FlowchartDiagram::default(), &Theme::default());
    assert!(svg.starts_with("<svg"));
}

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

/// True if any `<path d="…">` value contains a cubic-bezier `C` command.
fn any_bezier_path(svg: &str) -> bool {
    svg.split("d=\"").skip(1).any(|seg| {
        let d = &seg[..seg.find('"').unwrap_or(seg.len())];
        d.contains('C')
    })
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
    // A single short edge clips to 2 points → straight M..L.., no curve.
    let svg = render(&parse_flow("flowchart TD\na --> b\n"), &Theme::default());
    assert!(!any_bezier_path(&svg));
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
fn edge_attr_curve_step_is_honored() {
    // `curve: step` renders orthogonal steps — no cubic bezier on the skip edge.
    let svg = render(
        &parse_flow("flowchart TD\na --> b --> c --> d\na e1@--> d\ne1@{ curve: step }\n"),
        &Theme::default(),
    );
    assert!(svg.starts_with("<svg"));
    assert!(!any_bezier_path(&svg));
}
