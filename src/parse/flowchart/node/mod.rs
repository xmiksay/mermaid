//! Flowchart node/shape scanner and statement parser.
//!
//! Parses a statement line into its node groups and the edges between them:
//!   * node shapes (`[]`, `()`, `([ ])`, `[[]]`, `[( )]`, `(())`, `((()))`,
//!     `{}`, `{{}}`, the parallelogram/trapezoid family, and the `> ]` flag),
//!   * the Mermaid v11 `id@{ shape: …, label: … }` attribute block,
//!   * multi-source/target groups joined by `&` (cross product),
//!   * the `:::class` shorthand.

use std::collections::{HashMap, HashSet};

use super::super::ast::{FlowEdge, FlowNode, FlowchartDiagram, NodeShape};
use super::super::ParseError;
use super::edge::{consume_edge_id, parse_arrow};
use super::scanner::Scanner;

mod shapes;
mod spec;
#[cfg(test)]
mod tests;

use spec::parse_node_group;

pub(super) fn parse_statement(
    line: &str,
    diag: &mut FlowchartDiagram,
    nodes_by_id: &mut HashMap<String, usize>,
    edge_ids: &mut HashSet<String>,
    line_no: usize,
) -> Result<Vec<String>, ParseError> {
    let mut sc = Scanner::new(line);
    let mut referenced: Vec<String> = Vec::new();

    let first = parse_node_group(&mut sc, line_no)?;
    for n in &first {
        register_node(diag, nodes_by_id, n.clone());
        if !referenced.contains(&n.id) {
            referenced.push(n.id.clone());
        }
    }
    let mut prev_group = first;

    loop {
        sc.skip_ws();
        if sc.eof() {
            break;
        }
        // Optional v11 edge id prefix `e1@` before the connector — recorded so a
        // later `e1@{ … }` statement is recognized, and carried onto the edge(s)
        // this arrow creates so those attributes can be applied.
        let edge_id = consume_edge_id(&mut sc);
        if let Some(eid) = &edge_id {
            edge_ids.insert(eid.clone());
        }
        let Some((line_style, tail, head, label)) = parse_arrow(&mut sc, line_no)? else {
            return Err(ParseError::unknown(
                line_no,
                format!("unexpected text: '{}'", sc.remaining()),
            ));
        };
        sc.skip_ws();
        let next = parse_node_group(&mut sc, line_no)?;
        for n in &next {
            register_node(diag, nodes_by_id, n.clone());
            if !referenced.contains(&n.id) {
                referenced.push(n.id.clone());
            }
        }
        for src in &prev_group {
            for dst in &next {
                diag.edges.push(FlowEdge {
                    from: src.id.clone(),
                    to: dst.id.clone(),
                    label: label.clone(),
                    line: line_style,
                    tail,
                    head,
                    id: edge_id.clone(),
                    animate: false,
                    curve: None,
                });
            }
        }
        prev_group = next;
    }
    Ok(referenced)
}

fn register_node(
    diag: &mut FlowchartDiagram,
    by_id: &mut HashMap<String, usize>,
    node: FlowNode,
) -> bool {
    if let Some(&idx) = by_id.get(&node.id) {
        let existing = &mut diag.nodes[idx];
        // A later explicit declaration (one that supplied text or a non-default
        // shape) wins over earlier implicit references.
        let new_has_explicit = node.text != node.id || node.shape != NodeShape::Rect;
        if new_has_explicit {
            existing.text = node.text;
            existing.shape = node.shape;
        }
        // Merge any classes from a `:::` shorthand on this re-reference.
        for c in node.classes {
            if !existing.classes.contains(&c) {
                existing.classes.push(c);
            }
        }
        return false;
    }
    by_id.insert(node.id.clone(), diag.nodes.len());
    diag.nodes.push(node);
    true
}
