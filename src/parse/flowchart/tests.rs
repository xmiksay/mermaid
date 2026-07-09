use super::super::ast::{FlowDirection, FlowNode};
use super::*;

fn node<'a>(d: &'a FlowchartDiagram, id: &str) -> &'a FlowNode {
    d.nodes.iter().find(|n| n.id == id).unwrap()
}

#[test]
fn subgraph_tracked_in_ast() {
    let d = parse(
        "flowchart TD\nA --> B\nsubgraph S1 [Group One]\nB --> C\nsubgraph S2\nC --> D\nend\nend\nA --> E\n",
    )
    .unwrap();
    assert_eq!(d.subgraphs.len(), 2);
    let s1 = d.subgraphs.iter().find(|s| s.id == "S1").unwrap();
    let s2 = d.subgraphs.iter().find(|s| s.id == "S2").unwrap();
    assert_eq!(s1.label, "Group One");
    assert!(s1.node_ids.contains(&"B".to_string()) || s1.node_ids.contains(&"C".to_string()));
    assert!(s2.node_ids.contains(&"D".to_string()) || s2.node_ids.contains(&"C".to_string()));
    assert!(s1.child_subgraph_ids.contains(&"S2".to_string()));
}

#[test]
fn symbol_directions() {
    for (src, dir) in [
        ("graph >\nA --> B\n", FlowDirection::LeftRight),
        ("graph <\nA --> B\n", FlowDirection::RightLeft),
        ("graph ^\nA --> B\n", FlowDirection::BottomTop),
        ("graph v\nA --> B\n", FlowDirection::TopDown),
    ] {
        assert_eq!(parse(src).unwrap().direction, dir, "src: {src}");
    }
}

#[test]
fn flowchart_elk_header() {
    let d = crate::parse("flowchart-elk LR\nA --> B\n").unwrap();
    let crate::Diagram::Flowchart(f) = d else {
        panic!("expected flowchart");
    };
    assert_eq!(f.direction, FlowDirection::LeftRight);
    assert!(f.edges.iter().any(|e| e.from == "A" && e.to == "B"));
}

#[test]
fn subgraph_direction_parsed() {
    let d = parse("flowchart TD\nsubgraph S\ndirection LR\nA --> B\nend\n").unwrap();
    let s = d.subgraphs.iter().find(|s| s.id == "S").unwrap();
    assert_eq!(s.direction, Some(FlowDirection::LeftRight));
    // Top-level `direction` (outside any subgraph) stays a no-op.
    let d2 = parse("flowchart TD\ndirection LR\nA --> B\n").unwrap();
    assert_eq!(d2.direction, FlowDirection::TopDown);
}

#[test]
fn edge_to_subgraph_id_no_phantom_node() {
    let d = parse("flowchart TD\nsubgraph SG\nA --> B\nend\nC --> SG\n").unwrap();
    // No node materialized for the subgraph id; the edge keeps its endpoint.
    assert!(!d.nodes.iter().any(|n| n.id == "SG"));
    assert!(d.edges.iter().any(|e| e.from == "C" && e.to == "SG"));
    for id in ["A", "B", "C"] {
        assert!(d.nodes.iter().any(|n| n.id == id), "missing node {id}");
    }
}

#[test]
fn edge_to_subgraph_id_forward_ref_no_phantom() {
    // The edge references the subgraph before its `subgraph` line appears.
    let d = parse("flowchart TD\nC --> SG\nsubgraph SG\nA --> B\nend\n").unwrap();
    assert!(!d.nodes.iter().any(|n| n.id == "SG"));
    assert!(d.edges.iter().any(|e| e.to == "SG"));
}

#[test]
fn style_on_subgraph_id_lands_on_cluster() {
    let d =
        parse("flowchart TD\nsubgraph S [Group]\nA --> B\nend\nstyle S fill:#f9f,stroke:#333\n")
            .unwrap();
    assert!(!d.nodes.iter().any(|n| n.id == "S"));
    let s = d.subgraphs.iter().find(|s| s.id == "S").unwrap();
    assert_eq!(
        s.style,
        vec![
            ("fill".to_string(), "#f9f".to_string()),
            ("stroke".to_string(), "#333".to_string()),
        ]
    );
}

#[test]
fn class_on_subgraph_id_lands_on_cluster() {
    let d = parse("flowchart TD\nsubgraph S\nA --> B\nend\nclassDef hot fill:#f00\nclass S hot\n")
        .unwrap();
    assert!(!d.nodes.iter().any(|n| n.id == "S"));
    let s = d.subgraphs.iter().find(|s| s.id == "S").unwrap();
    assert_eq!(s.classes, vec!["hot".to_string()]);
}

#[test]
fn multiword_subgraph_title_is_not_truncated() {
    // A bracket-less multi-word title keeps every word (upstream renders
    // the whole text), so the id/label is not cut at the first space.
    let d = parse("flowchart TD\nsubgraph one two three\nA --> B\nend\n").unwrap();
    assert_eq!(d.subgraphs.len(), 1);
    assert_eq!(d.subgraphs[0].id, "one two three");
    // With no bracket label the renderer shows the id verbatim.
    assert!(d.subgraphs[0].label.is_empty());
}

#[test]
fn semicolon_after_header() {
    let d = parse("graph TD;\nA-->B\n").unwrap();
    assert_eq!(d.direction, FlowDirection::TopDown);
    assert_eq!(d.edges.len(), 1);
}

#[test]
fn semicolon_terminated_statement() {
    let d = parse("graph TD\nA-->B;\nB-->C;\n").unwrap();
    assert_eq!(d.nodes.len(), 3);
    assert_eq!(d.edges.len(), 2);
}

#[test]
fn statements_on_header_line() {
    let d = parse("graph LR; A-->B; B-->C\n").unwrap();
    assert_eq!(d.direction, FlowDirection::LeftRight);
    assert_eq!(d.nodes.len(), 3);
    assert_eq!(d.edges.len(), 2);
}

#[test]
fn semicolon_inside_label_is_kept() {
    let d = parse("graph TD;\nA[\"a;b\"]-->B;\n").unwrap();
    assert_eq!(d.edges.len(), 1);
    assert_eq!(node(&d, "A").text, "a;b");
}

#[test]
fn semicolon_in_pipe_label_is_kept() {
    let d = parse("graph TD;\nA-->|a;b|B;\n").unwrap();
    assert_eq!(d.edges.len(), 1);
    assert_eq!(d.edges[0].label.as_deref(), Some("a;b"));
}

fn syntax_line(input: &str) -> usize {
    match parse(input) {
        Err(ParseError::Syntax { line, .. }) => line,
        other => panic!("expected ParseError::Syntax, got {other:?}"),
    }
}

#[test]
fn unparseable_statement_hard_errors() {
    // A misspelled keyword parses as a node followed by junk it can't read
    // as an arrow — that must error, not silently disappear.
    assert_eq!(syntax_line("flowchart TD\nsubgrapgh Foo bar\n"), 2);
}

#[test]
fn malformed_directives_hard_error() {
    // Recognized keyword, but an incomplete body → error on that line.
    assert_eq!(syntax_line("flowchart TD\nstyle A\n"), 2);
    assert_eq!(syntax_line("flowchart TD\nclassDef foo\n"), 2);
    assert_eq!(syntax_line("flowchart TD\nclass foo\n"), 2);
    assert_eq!(syntax_line("flowchart TD\nlinkStyle 0\n"), 2);
    assert_eq!(syntax_line("flowchart TD\nA-->B\nclick A\n"), 3);
}

#[test]
fn unknown_direction_hard_errors() {
    assert_eq!(
        syntax_line("flowchart TD\nsubgraph S\ndirection SIDEWAYS\nend\n"),
        3
    );
}

#[test]
fn top_level_direction_is_tolerated_no_op() {
    // A valid top-level `direction` stays a no-op (the header wins), but its
    // value is still validated — so this parses.
    let d = parse("flowchart TD\ndirection LR\nA-->B\n").unwrap();
    assert_eq!(d.direction, FlowDirection::TopDown);
}
