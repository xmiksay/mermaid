use super::*;
use crate::parse::parse;

mod style;

fn parse_flow(s: &str) -> FlowchartDiagram {
    match parse(s).unwrap() {
        crate::parse::Diagram::Flowchart(f) => f,
        _ => panic!("expected flowchart"),
    }
}

/// Bounds `(x0, y0, x1, y1)` of the opaque background rect drawn directly
/// before the `text-anchor=middle` label for `id`.
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

// Parse the 5 vertices of the single `M.. L.. L.. L.. L.. Z` polygon in `svg`.
fn asymmetric_points(svg: &str) -> Vec<(f64, f64)> {
    let d_start = svg
        .match_indices("d=\"M")
        .map(|(i, _)| &svg[i + 3..])
        .find(|s| {
            let end = s.find('"').unwrap();
            s[..end].matches(" L").count() == 4 && s[..end].trim_end().ends_with('Z')
        })
        .expect("no 5-point asymmetric polygon found");
    let end = d_start.find('"').unwrap();
    d_start[..end]
        .replace(['M', 'Z'], " ")
        .replace('L', " ")
        .split_whitespace()
        .map(|t| t.parse::<f64>().unwrap())
        .collect::<Vec<_>>()
        .chunks(2)
        .map(|c| (c[0], c[1]))
        .collect()
}

/// True if any `<path d="…">` value contains a cubic-bezier `C` command.
fn any_bezier_path(svg: &str) -> bool {
    svg.split("d=\"").skip(1).any(|seg| {
        let d = &seg[..seg.find('"').unwrap_or(seg.len())];
        d.contains('C')
    })
}

/// True if any drawn number in `svg` (an attribute value or a path waypoint)
/// is negative — meaning some content landed left of/above the viewBox's
/// `0 0` origin and would be clipped.
fn has_negative_coordinate(svg: &str) -> bool {
    let bytes = svg.as_bytes();
    bytes
        .iter()
        .enumerate()
        .any(|(i, &b)| b == b'-' && bytes.get(i + 1).is_some_and(u8::is_ascii_digit))
}

/// The SVG's `viewBox` width and height.
fn viewbox_size(svg: &str) -> (f64, f64) {
    let start = svg.find("viewBox=\"").unwrap() + "viewBox=\"".len();
    let end = start + svg[start..].find('"').unwrap();
    let nums: Vec<f64> = svg[start..end]
        .split_whitespace()
        .map(|s| s.parse().unwrap())
        .collect();
    (nums[2], nums[3])
}

/// True if every edge-label background rect (the theme's `flow_label_bg`
/// fill) sits fully inside a `width`×`height` canvas.
fn all_label_rects_within_bounds(svg: &str, width: f64, height: f64) -> bool {
    svg.match_indices("fill=\"#e8e8e8\"").all(|(i, _)| {
        let rect_open = svg[..i].rfind("<rect ").unwrap();
        let tag = &svg[rect_open..i];
        let grab = |attr: &str| -> f64 {
            let s = tag.find(attr).unwrap() + attr.len();
            let e = s + tag[s..].find('"').unwrap();
            tag[s..e].parse().unwrap()
        };
        let (x, y, w, h) = (
            grab("x=\""),
            grab("y=\""),
            grab("width=\""),
            grab("height=\""),
        );
        x >= -0.01 && y >= -0.01 && x + w <= width + 0.01 && y + h <= height + 0.01
    })
}

#[test]
fn non_opposite_pair_labels_do_not_overlap() {
    // Two unrelated edges (A->X, B->X — not a reversed pair) whose natural
    // midpoints land at the same row; without the general overlap-avoidance
    // pass their opaque label backgrounds would occlude one another.
    let d = parse_flow("flowchart TD\nA -->|alpha label here| X\nB -->|beta label here| X\n");
    let svg = render(&d, &Theme::default());
    let (ax0, ay0, ax1, ay1) = label_bg_box(&svg, "alpha label here");
    let (bx0, by0, bx1, by1) = label_bg_box(&svg, "beta label here");
    let overlap = ax0 < bx1 && bx0 < ax1 && ay0 < by1 && by0 < ay1;
    assert!(
        !overlap,
        "label backgrounds overlap: alpha ({ax0},{ay0},{ax1},{ay1}) vs beta ({bx0},{by0},{bx1},{by1})",
    );
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
    // Themed cluster frame + centered (normal-weight) label.
    assert!(svg.contains("fill=\"#ffffde\""));
    assert!(svg.contains(">Group<"));
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
fn asymmetric_shape_has_left_notch_and_straight_right_edge() {
    // #251: the odd `>flag]` shape must mirror upstream `rect_left_inv_arrow`:
    // a concave notch on the LEFT edge and a straight vertical right edge.
    let svg = render(&parse_flow("flowchart LR\nE>flag]\n"), &Theme::default());
    let pts = asymmetric_points(&svg);
    assert_eq!(pts.len(), 5, "asymmetric polygon must have 5 vertices");

    let min_x = pts.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let max_x = pts.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
    let cy = pts.iter().map(|p| p.1).sum::<f64>() / pts.len() as f64;

    // Straight right edge: two vertices sit on the rightmost x.
    let on_right = pts.iter().filter(|p| (p.0 - max_x).abs() < 0.01).count();
    assert_eq!(
        on_right, 2,
        "right edge must be a straight vertical segment"
    );

    // Concave notch on the left: exactly one vertex lies strictly inside the
    // left edge (x between the edges) at the vertical mid-line.
    let notch = pts
        .iter()
        .find(|p| p.0 > min_x + 0.01 && p.0 < max_x - 0.01)
        .expect("left edge must have an inward notch vertex");
    assert!(
        (notch.1 - cy).abs() < 0.01,
        "notch tip must sit on the vertical mid-line"
    );
    // The notch is nearer the left edge than the right one.
    assert!(
        notch.0 - min_x < max_x - notch.0,
        "notch must be on the left half of the shape"
    );
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
