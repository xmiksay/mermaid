//! Flowchart styling directives: `style`, `classDef`, `class`, `linkStyle`.
//!
//! Each fills the diagram's inline node style, class definitions, per-node
//! class assignments, or per-edge/link styles. `node_index` is the shared
//! helper that materializes a bare rectangle placeholder for any id referenced
//! by a directive before it is declared.

use std::collections::HashMap;

use super::super::ast::{EdgeCurve, FlowNode, FlowchartDiagram, NodeShape, Style};
use super::super::style::parse_style_props;
use super::super::ParseError;

/// Index of the node with `id`, creating a bare rectangle placeholder if a
/// directive references it before it is declared.
pub(super) fn node_index(
    diag: &mut FlowchartDiagram,
    nodes_by_id: &mut HashMap<String, usize>,
    id: &str,
) -> usize {
    *nodes_by_id.entry(id.to_string()).or_insert_with(|| {
        diag.nodes.push(FlowNode {
            id: id.to_string(),
            text: id.to_string(),
            shape: NodeShape::Rect,
            classes: Vec::new(),
            style: Style::new(),
            click: None,
        });
        diag.nodes.len() - 1
    })
}

/// `style <id> <props>` — inline style on a single node.
pub(super) fn handle_style(
    rest: &str,
    diag: &mut FlowchartDiagram,
    nodes_by_id: &mut HashMap<String, usize>,
    line_no: usize,
) -> Result<(), ParseError> {
    let (id, props) = rest
        .trim()
        .split_once(char::is_whitespace)
        .ok_or_else(|| malformed("style", line_no))?;
    let style = parse_style_props(props);
    let idx = node_index(diag, nodes_by_id, id.trim());
    diag.nodes[idx].style = style;
    Ok(())
}

/// `classDef <name>[,<name2>] <props>` — define one or more style classes.
pub(super) fn handle_class_def(
    rest: &str,
    diag: &mut FlowchartDiagram,
    line_no: usize,
) -> Result<(), ParseError> {
    let (names, props) = rest
        .trim()
        .split_once(char::is_whitespace)
        .ok_or_else(|| malformed("classDef", line_no))?;
    let style = parse_style_props(props);
    for name in names.split(',') {
        let name = name.trim();
        if !name.is_empty() {
            diag.class_defs.insert(name.to_string(), style.clone());
        }
    }
    Ok(())
}

/// `class <id1>,<id2> <className>` — apply a class to nodes.
pub(super) fn handle_class_apply(
    rest: &str,
    diag: &mut FlowchartDiagram,
    nodes_by_id: &mut HashMap<String, usize>,
    line_no: usize,
) -> Result<(), ParseError> {
    let (ids, class_name) = rest
        .trim()
        .rsplit_once(char::is_whitespace)
        .ok_or_else(|| malformed("class", line_no))?;
    let class_name = class_name.trim();
    if class_name.is_empty() {
        return Err(malformed("class", line_no));
    }
    for id in ids.split(',') {
        let id = id.trim();
        if id.is_empty() {
            continue;
        }
        let idx = node_index(diag, nodes_by_id, id);
        let classes = &mut diag.nodes[idx].classes;
        if !classes.iter().any(|c| c == class_name) {
            classes.push(class_name.to_string());
        }
    }
    Ok(())
}

/// `linkStyle <default|idx-list> [interpolate <curve>] <props>` — style edges
/// by their definition index. The optional `interpolate <curve>` clause sets the
/// edge interpolation (`linear`/`step`/…); the remaining tokens are real props.
pub(super) fn handle_link_style(
    rest: &str,
    diag: &mut FlowchartDiagram,
    line_no: usize,
) -> Result<(), ParseError> {
    let (selector, props) = rest
        .trim()
        .split_once(char::is_whitespace)
        .ok_or_else(|| malformed("linkStyle", line_no))?;
    let mut props = props.trim();
    let mut curve = None;
    if let Some(after) = props.strip_prefix("interpolate ") {
        let (name, remaining) = after
            .trim()
            .split_once(char::is_whitespace)
            .unwrap_or((after.trim(), ""));
        curve = Some(EdgeCurve::from_name(name));
        props = remaining;
    }
    let style = parse_style_props(props);
    if selector == "default" {
        diag.link_style_default = style;
        if let Some(c) = curve {
            diag.default_interpolate = Some(c);
        }
        return Ok(());
    }
    for idx in selector.split(',') {
        if let Ok(i) = idx.trim().parse::<usize>() {
            diag.edge_styles.insert(i, style.clone());
            if let Some(c) = curve {
                diag.edge_interpolate.insert(i, c);
            }
        }
    }
    Ok(())
}

/// A `ParseError::Syntax` for a directive keyword that was recognized but whose
/// body could not be parsed (e.g. `style` / `classDef` with no properties).
fn malformed(keyword: &str, line_no: usize) -> ParseError {
    ParseError::malformed(line_no, format!("malformed '{keyword}' statement"))
}

#[cfg(test)]
mod tests {
    use super::super::super::ast::{FlowNode, FlowchartDiagram};
    use super::super::parse;

    fn node<'a>(d: &'a FlowchartDiagram, id: &str) -> &'a FlowNode {
        d.nodes.iter().find(|n| n.id == id).unwrap()
    }

    #[test]
    fn style_directive_sets_inline_style() {
        let d = parse("flowchart TD\nA-->B\nstyle A fill:#f9f,stroke:#333\n").unwrap();
        assert_eq!(
            node(&d, "A").style,
            vec![
                ("fill".to_string(), "#f9f".to_string()),
                ("stroke".to_string(), "#333".to_string()),
            ]
        );
    }

    #[test]
    fn classdef_and_class_apply() {
        let d = parse("flowchart TD\nA-->B\nclassDef foo fill:#0f0,stroke:#333\nclass A foo\n")
            .unwrap();
        assert_eq!(d.class_defs["foo"].len(), 2);
        assert_eq!(node(&d, "A").classes, vec!["foo".to_string()]);
    }

    #[test]
    fn classdef_multiple_names() {
        let d = parse("flowchart TD\nA-->B\nclassDef a,b fill:#111\n").unwrap();
        assert!(d.class_defs.contains_key("a"));
        assert!(d.class_defs.contains_key("b"));
    }

    #[test]
    fn classdef_default_present() {
        let d = parse("flowchart TD\nA-->B\nclassDef default fill:#eee\n").unwrap();
        assert!(d.class_defs.contains_key("default"));
    }

    #[test]
    fn link_style_by_index_and_default() {
        let d = parse(
            "flowchart TD\nA-->B\nB-->C\nlinkStyle 0 stroke:#ff3,stroke-width:4px\nlinkStyle default stroke:#000\n",
        )
        .unwrap();
        assert_eq!(d.edge_styles[&0].len(), 2);
        assert_eq!(
            d.link_style_default,
            vec![("stroke".to_string(), "#000".to_string())]
        );
    }

    #[test]
    fn link_style_multiple_indices() {
        let d = parse("flowchart TD\nA-->B\nB-->C\nlinkStyle 0,1 stroke:#abc\n").unwrap();
        assert!(d.edge_styles.contains_key(&0));
        assert!(d.edge_styles.contains_key(&1));
    }

    #[test]
    fn link_style_interpolate_sets_curve_and_keeps_props() {
        use super::super::super::ast::EdgeCurve;
        let d = parse("flowchart TD\nA-->B\nlinkStyle 0 interpolate linear stroke:#abc\n").unwrap();
        assert_eq!(
            d.edge_styles[&0],
            vec![("stroke".to_string(), "#abc".to_string())]
        );
        assert_eq!(d.edge_interpolate[&0], EdgeCurve::Linear);
    }

    #[test]
    fn link_style_interpolate_without_props() {
        use super::super::super::ast::EdgeCurve;
        let d = parse("flowchart TD\nA-->B\nlinkStyle 0 interpolate step\n").unwrap();
        assert_eq!(d.edge_interpolate[&0], EdgeCurve::Step);
    }

    #[test]
    fn link_style_default_interpolate() {
        use super::super::super::ast::EdgeCurve;
        let d = parse("flowchart TD\nA-->B\nlinkStyle default interpolate linear\n").unwrap();
        assert_eq!(d.default_interpolate, Some(EdgeCurve::Linear));
    }
}
