use super::super::parse;
use super::*;

fn node<'a>(d: &'a FlowchartDiagram, id: &str) -> &'a FlowNode {
    d.nodes.iter().find(|n| n.id == id).unwrap()
}

#[test]
fn all_shapes_basic() {
    let d = parse(
        "flowchart TD\n\
             A[r] --> B(round)\n\
             B --> C((circle))\n\
             C --> D(((dbl)))\n\
             D --> E{rh}\n\
             E --> F{{hex}}\n\
             F --> G[[sub]]\n\
             G --> H[(cyl)]\n\
             H --> I([sta])\n",
    )
    .unwrap();
    let shapes: Vec<_> = d.nodes.iter().map(|n| (n.id.clone(), n.shape)).collect();
    assert!(shapes.contains(&("A".into(), NodeShape::Rect)));
    assert!(shapes.contains(&("B".into(), NodeShape::Round)));
    assert!(shapes.contains(&("C".into(), NodeShape::Circle)));
    assert!(shapes.contains(&("D".into(), NodeShape::DoubleCircle)));
    assert!(shapes.contains(&("E".into(), NodeShape::Rhombus)));
    assert!(shapes.contains(&("F".into(), NodeShape::Hexagon)));
    assert!(shapes.contains(&("G".into(), NodeShape::Subroutine)));
    assert!(shapes.contains(&("H".into(), NodeShape::Cylinder)));
    assert!(shapes.contains(&("I".into(), NodeShape::Stadium)));
}

#[test]
fn at_shape_and_label() {
    let d = parse("flowchart TD\nA@{ shape: rounded, label: \"Hi there\" } --> B\n").unwrap();
    let a = node(&d, "A");
    assert_eq!(a.shape, NodeShape::Round);
    assert_eq!(a.text, "Hi there");
    assert_eq!(d.edges.len(), 1);
    assert_eq!(d.edges[0].to, "B");
}

#[test]
fn at_shape_aliases_map_to_variants() {
    let d = parse(
        "flowchart TD\n\
             A@{ shape: diam } --> B@{ shape: cyl }\n\
             B --> C@{ shape: circle }\n\
             C --> E@{ shape: hex }\n\
             E --> F@{ shape: lean-r }\n\
             F --> G@{ shape: lean-l }\n\
             G --> H@{ shape: trap-b }\n\
             H --> I@{ shape: trap-t }\n\
             I --> J@{ shape: dbl-circ }\n\
             J --> K@{ shape: stadium }\n\
             K --> L@{ shape: subproc }\n",
    )
    .unwrap();
    let map: HashMap<_, _> = d.nodes.iter().map(|n| (n.id.clone(), n.shape)).collect();
    assert_eq!(map["A"], NodeShape::Rhombus);
    assert_eq!(map["B"], NodeShape::Cylinder);
    assert_eq!(map["C"], NodeShape::Circle);
    assert_eq!(map["E"], NodeShape::Hexagon);
    assert_eq!(map["F"], NodeShape::Parallelogram);
    assert_eq!(map["G"], NodeShape::ParallelogramAlt);
    assert_eq!(map["H"], NodeShape::Trapezoid);
    assert_eq!(map["I"], NodeShape::TrapezoidAlt);
    assert_eq!(map["J"], NodeShape::DoubleCircle);
    assert_eq!(map["K"], NodeShape::Stadium);
    assert_eq!(map["L"], NodeShape::Subroutine);
}

#[test]
fn at_unknown_shape_falls_back_to_rect() {
    // A name with no variant (and any unknown name) falls back to Rect
    // rather than erroring, and the label is preserved.
    let d = parse("flowchart TD\nA@{ shape: text, label: \"kept\" } --> B\n").unwrap();
    let a = node(&d, "A");
    assert_eq!(a.shape, NodeShape::Rect);
    assert_eq!(a.text, "kept");
}

#[test]
fn at_v11_shape_names_map_to_variants() {
    // Each v11 name (and a representative alias) maps to its own variant.
    let cases = [
        ("notch-rect", NodeShape::NotchedRect),
        ("card", NodeShape::NotchedRect),
        ("doc", NodeShape::Document),
        ("docs", NodeShape::MultiDocument),
        ("tag-doc", NodeShape::TaggedDocument),
        ("bolt", NodeShape::LightningBolt),
        ("hourglass", NodeShape::Hourglass),
        ("comment", NodeShape::Comment),
        ("delay", NodeShape::Delay),
        ("das", NodeShape::DirectAccessStorage),
        ("lin-cyl", NodeShape::LinedCylinder),
        ("lin-rect", NodeShape::LinedProcess),
        ("div-rect", NodeShape::DividedProcess),
        ("win-pane", NodeShape::WindowPane),
        ("tri", NodeShape::Triangle),
        ("flip-tri", NodeShape::FlippedTriangle),
        ("f-circ", NodeShape::FilledCircle),
        ("cross-circ", NodeShape::CrossedCircle),
        ("paper-tape", NodeShape::PaperTape),
        ("bow-rect", NodeShape::StoredData),
    ];
    for (name, expected) in cases {
        let d = parse(&format!("flowchart TD\nA@{{ shape: {name} }} --> B\n")).unwrap();
        assert_eq!(node(&d, "A").shape, expected, "shape name {name:?}");
    }
}

#[test]
fn at_icon_form_drops_shape_keeps_label() {
    let d = parse("flowchart TD\nA@{ icon: \"fa:bell\", label: \"Alarm\" } --> B\n").unwrap();
    let a = node(&d, "A");
    assert_eq!(a.shape, NodeShape::Rect);
    assert_eq!(a.text, "Alarm");
}

#[test]
fn at_label_only_keeps_default_shape() {
    let d = parse("flowchart TD\nA@{ label: \"only\" }\n").unwrap();
    let a = node(&d, "A");
    assert_eq!(a.shape, NodeShape::Rect);
    assert_eq!(a.text, "only");
}

#[test]
fn asymmetric_shapes() {
    let d = parse(
        "flowchart TD\nA[/par/] --> B[\\paralt\\]\nB --> C[/trap\\]\nC --> D[\\trapalt/]\nD --> E>flag]\n",
    )
    .unwrap();
    let map: HashMap<_, _> = d.nodes.iter().map(|n| (n.id.clone(), n.shape)).collect();
    assert_eq!(map["A"], NodeShape::Parallelogram);
    assert_eq!(map["B"], NodeShape::ParallelogramAlt);
    assert_eq!(map["C"], NodeShape::Trapezoid);
    assert_eq!(map["D"], NodeShape::TrapezoidAlt);
    assert_eq!(map["E"], NodeShape::Asymmetric);
}

#[test]
fn at_node_decl_still_works_when_not_an_edge_id() {
    // A standalone `A@{ … }` with no known edge id still declares node A.
    let d = parse("flowchart TD\nA@{ shape: circle, label: \"hi\" }\n").unwrap();
    let a = node(&d, "A");
    assert_eq!(a.shape, NodeShape::Circle);
    assert_eq!(a.text, "hi");
}

#[test]
fn dashed_node_ids_parse() {
    // Upstream NODE_STRING allows `-`/`/` inside an id; the dash only stops
    // the id when it begins an arrow.
    let d = parse("flowchart LR\na-node --> b-node\nx/y --> z\n").unwrap();
    for id in ["a-node", "b-node", "x/y", "z"] {
        assert!(d.nodes.iter().any(|n| n.id == id), "missing node {id}");
    }
    assert!(d
        .edges
        .iter()
        .any(|e| e.from == "a-node" && e.to == "b-node"));
    assert!(d.edges.iter().any(|e| e.from == "x/y" && e.to == "z"));
}

#[test]
fn quoted_label_may_contain_shape_closer() {
    let d = parse("flowchart LR\nA[\"a ] b\"] --> B(\"call (x)\")\n").unwrap();
    assert_eq!(node(&d, "A").text, "a ] b");
    assert_eq!(node(&d, "B").text, "call (x)");
    assert_eq!(d.edges.len(), 1);
}

#[test]
fn percent_in_quoted_label_is_not_a_comment() {
    let d = parse("flowchart LR\nA[\"100%% sure\"] --> B\n").unwrap();
    assert_eq!(node(&d, "A").text, "100%% sure");
    assert_eq!(d.edges.len(), 1);
}

#[test]
fn triple_colon_shorthand() {
    let d = parse("flowchart TD\nA:::foo --> B\n").unwrap();
    assert_eq!(node(&d, "A").classes, vec!["foo".to_string()]);
    assert_eq!(d.edges.len(), 1);
}

#[test]
fn triple_colon_keeps_shape_and_text() {
    let d = parse("flowchart TD\nA[hello]:::foo --> B\n").unwrap();
    let a = node(&d, "A");
    assert_eq!(a.classes, vec!["foo".to_string()]);
    assert_eq!(a.text, "hello");
    assert_eq!(a.shape, NodeShape::Rect);
}
