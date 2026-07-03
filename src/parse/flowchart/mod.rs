//! Flowchart parser.
//!
//! Supports:
//!   * `flowchart <DIR>` / `graph <DIR>` header.
//!   * Node shapes: rect `[]`, round `()`, stadium `([ ])`, subroutine `[[]]`,
//!     cylinder `[( )]`, circle `(())`, double circle `((()))`, rhombus `{}`,
//!     hexagon `{{}}`, parallelogram `[/ /]`, parallelogram-alt `[\ \]`,
//!     trapezoid `[/ \]`, trapezoid-alt `[\ /]`, asymmetric flag `> ]`.
//!   * Mermaid v11 attribute syntax `id@{ shape: …, label: … }`: named shapes
//!     map onto the variants above (unknown names fall back to rect);
//!     `icon`/`img` forms are dropped but any `label` is preserved.
//!   * Edge tokens combine a line style (`-`, `.`, `=`) and head (`>`, `o`,
//!     `x`, none): `-->`, `---`, `-.->`, `-.-`, `==>`, `===`, `--o`, `--x`.
//!   * Edge labels in either the pipe form `A -->|text| B` or the inline form
//!     `A -- text --> B` (also `-. text .->` and `== text ==>`).
//!   * Multi-source / multi-target via `&`: `A & B --> C & D` produces all
//!     four edges.
//!   * `subgraph <id> [label]` ... `end` blocks tracked in `subgraphs`,
//!     including nesting.
//!   * `click <id> …` binds a hyperlink or JS callback to a node.
//!   * Mermaid v11 edge ids: the `e1@` prefix in `A e1@--> B` names the edge,
//!     and a standalone `e1@{ animate: …, curve: … }` statement applies those
//!     attributes to it.
//!   * `style`/`class` on a subgraph id styles the cluster frame; other
//!     `style`/`classDef`/`class`/`linkStyle` populate the node/edge styles.
//!
//! The parser is split into submodules driven from `parse`:
//!   * `scanner` — the shared line cursor,
//!   * `node` — node/shape/statement scanning,
//!   * `edge` — the arrow scanner and v11 edge ids,
//!   * `click` — the `click` directive,
//!   * `directive` — `style`/`classDef`/`class`/`linkStyle`.
//!
//! # Unknown-line policy
//!
//! Unparseable statements **hard-error** with `ParseError::Syntax { line }`,
//! matching every other diagram parser (upstream renders its error diagram for
//! the same input). A recognized keyword whose body is malformed — a bare
//! `style` / `classDef` / `class` / `linkStyle` / `click` with no arguments, or
//! a `direction` naming an unknown token — errors on the offending line rather
//! than being silently dropped, so a typo can't vanish.
//!
//! Two tolerances are deliberate, following upstream:
//!   * a top-level `direction` (outside any `subgraph`) is a no-op — the header
//!     already fixed the diagram direction — though its value is still checked;
//!   * unknown keys inside a v11 `id@{ … }` attribute block, and unknown
//!     `shape:` names, are ignored/fall back to `Rect` (see `node`) so forward-
//!     compatible metadata doesn't break older renderers.

use std::collections::{HashMap, HashSet};

use super::ast::{EdgeCurve, FlowDirection, FlowchartDiagram, Style, Subgraph};
use super::{strip_comment, ParseError};

mod click;
mod directive;
mod edge;
mod node;
mod scanner;

use click::parse_click;
use directive::{
    handle_class_apply, handle_class_def, handle_link_style, handle_style, node_index,
};
use edge::edge_attr_stmt;
use node::parse_statement;

pub(crate) fn parse(input: &str) -> Result<FlowchartDiagram, ParseError> {
    let mut diag = FlowchartDiagram::default();
    let mut header_seen = false;
    let mut nodes_by_id: HashMap<String, usize> = HashMap::new();
    let mut subgraph_stack: Vec<usize> = Vec::new();
    let mut subgraph_auto_id = 0usize;
    // Edge ids from the v11 `A e1@--> B` syntax. Recorded so a later standalone
    // `e1@{ … }` edge-attribute statement is recognized and dropped rather than
    // materialized as a phantom node.
    let mut edge_ids: HashSet<String> = HashSet::new();

    // A `;` terminates a statement anywhere a newline would (upstream grammar),
    // so flatten each source line into its `;`-separated statements. This lets
    // `graph TD;` and `graph LR; A-->B` parse (header + statements on one line).
    let statements = input.lines().enumerate().flat_map(|(idx, raw)| {
        split_semicolons(strip_comment(raw))
            .into_iter()
            .map(move |s| (idx + 1, s.trim()))
    });

    for (line_no, line) in statements {
        if line.is_empty() {
            continue;
        }

        if !header_seen {
            parse_header(line, &mut diag, line_no)?;
            header_seen = true;
            continue;
        }

        if line == "end" {
            subgraph_stack.pop();
            continue;
        }

        if let Some(rest) = line.strip_prefix("subgraph") {
            handle_subgraph_open(
                rest.trim(),
                &mut diag,
                &mut subgraph_stack,
                &mut subgraph_auto_id,
            );
            continue;
        }

        if let Some(rest) = line.strip_prefix("style ") {
            handle_style(rest, &mut diag, &mut nodes_by_id, line_no)?;
            continue;
        }
        if let Some(rest) = line.strip_prefix("classDef ") {
            handle_class_def(rest, &mut diag, line_no)?;
            continue;
        }
        if let Some(rest) = line.strip_prefix("class ") {
            handle_class_apply(rest, &mut diag, &mut nodes_by_id, line_no)?;
            continue;
        }
        if let Some(rest) = line.strip_prefix("linkStyle ") {
            handle_link_style(rest, &mut diag, line_no)?;
            continue;
        }
        if let Some(rest) = line.strip_prefix("click ") {
            let (id, action) = parse_click(rest)
                .ok_or_else(|| ParseError::malformed(line_no, "malformed 'click' statement"))?;
            let idx = node_index(&mut diag, &mut nodes_by_id, &id);
            diag.nodes[idx].click = Some(action);
            continue;
        }
        if let Some(rest) = line.strip_prefix("direction ") {
            // The direction value is validated (an unknown token is a typo we
            // report), but a `direction X` only takes effect inside a subgraph
            // body — at top level the header already set the diagram direction,
            // so upstream treats it as a no-op.
            let dir = parse_direction(rest.trim()).ok_or_else(|| {
                ParseError::unknown(line_no, format!("unknown direction: '{}'", rest.trim()))
            })?;
            if let Some(&parent) = subgraph_stack.last() {
                diag.subgraphs[parent].direction = Some(dir);
            }
            continue;
        }

        // A standalone `e1@{ … }` edge-attribute statement (v11) referencing a
        // known edge id carries no node — apply its attributes to the edge and
        // skip it so it doesn't spawn a phantom node.
        if let Some((eid, attrs)) = edge_attr_stmt(line) {
            if edge_ids.contains(&eid) {
                apply_edge_attrs(&mut diag, &eid, &attrs);
                continue;
            }
        }

        let added_node_ids =
            parse_statement(line, &mut diag, &mut nodes_by_id, &mut edge_ids, line_no)?;
        if let Some(&parent) = subgraph_stack.last() {
            for id in added_node_ids {
                if !diag.subgraphs[parent].node_ids.contains(&id) {
                    diag.subgraphs[parent].node_ids.push(id);
                }
            }
        }
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }

    // An edge endpoint naming a subgraph refers to the cluster, not a node.
    // Drop any node materialized for such an id (whether from a forward
    // reference before the `subgraph` line or an edge to it); the edge keeps
    // its string endpoint and the renderer routes it to the cluster box.
    let sub_ids: HashSet<String> = diag.subgraphs.iter().map(|s| s.id.clone()).collect();
    if !sub_ids.is_empty() {
        // A `style`/`class` directive on a subgraph id lands on the phantom node
        // about to be dropped — move it onto the cluster so the frame is styled.
        for s in &mut diag.subgraphs {
            if let Some(n) = diag.nodes.iter().find(|n| n.id == s.id) {
                if !n.style.is_empty() {
                    s.style = n.style.clone();
                }
                for c in &n.classes {
                    if !s.classes.contains(c) {
                        s.classes.push(c.clone());
                    }
                }
            }
        }
        diag.nodes.retain(|n| !sub_ids.contains(&n.id));
        for s in &mut diag.subgraphs {
            s.node_ids.retain(|id| !sub_ids.contains(id));
        }
    }
    Ok(diag)
}

/// Apply a v11 `id@{ … }` edge-attribute statement to every edge carrying that
/// id. `animate: true` turns on the dash-flow animation; `curve: <name>` sets
/// the per-edge interpolation. Unknown keys are ignored.
fn apply_edge_attrs(diag: &mut FlowchartDiagram, id: &str, attrs: &[(String, String)]) {
    for edge in diag.edges.iter_mut() {
        if edge.id.as_deref() != Some(id) {
            continue;
        }
        for (key, value) in attrs {
            match key.as_str() {
                "animate" => edge.animate = value.eq_ignore_ascii_case("true"),
                "curve" => edge.curve = Some(EdgeCurve::from_name(value)),
                _ => {}
            }
        }
    }
}

/// Split a comment-stripped line into statements at top-level `;`. A semicolon
/// only separates when it is not inside a quoted string, a shape bracket, or an
/// edge-label `|…|` run, so `#59;` entity codes and labels like `["a;b"]` stay
/// intact.
fn split_semicolons(line: &str) -> Vec<&str> {
    if !line.contains(';') {
        return vec![line];
    }
    let mut out = Vec::new();
    let mut depth: i32 = 0;
    let mut in_quote = false;
    let mut in_pipe = false;
    let mut start = 0;
    for (i, c) in line.char_indices() {
        match c {
            '"' if !in_pipe => in_quote = !in_quote,
            '|' if !in_quote => in_pipe = !in_pipe,
            '[' | '(' | '{' if !in_quote && !in_pipe => depth += 1,
            ']' | ')' | '}' if !in_quote && !in_pipe => depth -= 1,
            ';' if !in_quote && !in_pipe && depth <= 0 => {
                out.push(&line[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&line[start..]);
    out
}

fn parse_header(line: &str, diag: &mut FlowchartDiagram, line_no: usize) -> Result<(), ParseError> {
    let rest = if let Some(r) = line.strip_prefix("flowchart") {
        r
    } else if let Some(r) = line.strip_prefix("graph") {
        r
    } else {
        return Err(ParseError::header(
            line_no,
            "expected 'flowchart' or 'graph' header",
        ));
    };
    if let Some(c) = rest.chars().next() {
        if !c.is_whitespace() {
            return Err(ParseError::header(
                line_no,
                "expected 'flowchart' or 'graph' header",
            ));
        }
    }
    diag.direction = parse_direction(rest.trim()).ok_or_else(|| {
        ParseError::unknown(line_no, format!("unknown direction: '{}'", rest.trim()))
    })?;
    Ok(())
}

fn parse_direction(s: &str) -> Option<FlowDirection> {
    match s {
        "" | "TD" | "TB" => Some(FlowDirection::TopDown),
        "BT" => Some(FlowDirection::BottomTop),
        "LR" => Some(FlowDirection::LeftRight),
        "RL" => Some(FlowDirection::RightLeft),
        _ => None,
    }
}

fn handle_subgraph_open(
    rest: &str,
    diag: &mut FlowchartDiagram,
    stack: &mut Vec<usize>,
    auto: &mut usize,
) {
    // Forms:
    //   subgraph X
    //   subgraph X [Label]
    //   subgraph "Just a label"   (auto id)
    let rest = rest.trim();
    let (id, label) = if rest.is_empty() {
        *auto += 1;
        (format!("sg{auto}"), String::new())
    } else if rest.starts_with('"') {
        *auto += 1;
        let label = rest.trim_matches('"').to_string();
        (format!("sg{auto}"), label)
    } else if let Some((id, label)) = rest.split_once(' ') {
        let label_clean = label
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']')
            .trim_matches('"')
            .to_string();
        (id.trim().to_string(), label_clean)
    } else {
        (rest.to_string(), String::new())
    };

    let new_idx = diag.subgraphs.len();
    diag.subgraphs.push(Subgraph {
        id: id.clone(),
        label,
        direction: None,
        node_ids: Vec::new(),
        child_subgraph_ids: Vec::new(),
        classes: Vec::new(),
        style: Style::new(),
    });
    if let Some(&parent) = stack.last() {
        diag.subgraphs[parent].child_subgraph_ids.push(id);
    }
    stack.push(new_idx);
}

#[cfg(test)]
mod tests {
    use super::super::ast::FlowNode;
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
        let d = parse(
            "flowchart TD\nsubgraph S [Group]\nA --> B\nend\nstyle S fill:#f9f,stroke:#333\n",
        )
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
        let d =
            parse("flowchart TD\nsubgraph S\nA --> B\nend\nclassDef hot fill:#f00\nclass S hot\n")
                .unwrap();
        assert!(!d.nodes.iter().any(|n| n.id == "S"));
        let s = d.subgraphs.iter().find(|s| s.id == "S").unwrap();
        assert_eq!(s.classes, vec!["hot".to_string()]);
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
}
