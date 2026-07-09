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

use super::ast::{EdgeCurve, FlowchartDiagram};
use super::{strip_comment, ParseError};

pub(crate) mod click;
mod directive;
mod edge;
mod header;
mod node;
mod scanner;

use click::parse_click;
use directive::{
    handle_class_apply, handle_class_def, handle_link_style, handle_style, node_index,
};
use edge::edge_attr_stmt;
use header::{handle_subgraph_open, parse_direction, parse_header, split_semicolons};
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

#[cfg(test)]
mod tests;
