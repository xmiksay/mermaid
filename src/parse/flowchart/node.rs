//! Flowchart node/shape scanner and statement parser.
//!
//! Parses a statement line into its node groups and the edges between them:
//!   * node shapes (`[]`, `()`, `([ ])`, `[[]]`, `[( )]`, `(())`, `((()))`,
//!     `{}`, `{{}}`, the parallelogram/trapezoid family, and the `> ]` flag),
//!   * the Mermaid v11 `id@{ shape: …, label: … }` attribute block,
//!   * multi-source/target groups joined by `&` (cross product),
//!   * the `:::class` shorthand.

use std::collections::{HashMap, HashSet};

use super::super::ast::{FlowEdge, FlowNode, FlowchartDiagram, NodeShape, Style};
use super::super::token::{find_unquoted, split_unquoted, unquote};
use super::super::ParseError;
use super::edge::{consume_edge_id, parse_arrow};
use super::scanner::Scanner;

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

fn parse_node_group(sc: &mut Scanner<'_>, line_no: usize) -> Result<Vec<FlowNode>, ParseError> {
    let mut out = Vec::new();
    loop {
        sc.skip_ws();
        let node = parse_node_spec(sc, line_no)?;
        out.push(node);
        sc.skip_ws();
        if !sc.try_consume("&") {
            break;
        }
    }
    Ok(out)
}

fn parse_node_spec(sc: &mut Scanner<'_>, line_no: usize) -> Result<FlowNode, ParseError> {
    sc.skip_ws();
    // The asymmetric `>` flag opens BEFORE an id appears, but Mermaid actually
    // requires an id first. The shape opener is detected after the id.
    let id = sc.read_ident().ok_or_else(|| {
        ParseError::malformed(
            line_no,
            format!("expected node identifier at: '{}'", sc.remaining()),
        )
    })?;

    // Mermaid v11 attribute syntax: `id@{ shape: …, label: … }`.
    if sc.peek_str("@{") {
        return parse_at_node(id, sc, line_no);
    }

    // The shape table: longer openers first so they win over their prefixes.
    const SHAPES: &[(&str, &str, NodeShape)] = &[
        ("(((", ")))", NodeShape::DoubleCircle),
        ("([", "])", NodeShape::Stadium),
        ("[[", "]]", NodeShape::Subroutine),
        ("[(", ")]", NodeShape::Cylinder),
        ("((", "))", NodeShape::Circle),
        ("{{", "}}", NodeShape::Hexagon),
        ("[/", "/]", NodeShape::Parallelogram),
        ("[\\", "\\]", NodeShape::ParallelogramAlt),
        // trapezoids — must be tried before plain `[`/`/]`
        ("[/", "\\]", NodeShape::Trapezoid),
        ("[\\", "/]", NodeShape::TrapezoidAlt),
        (">", "]", NodeShape::Asymmetric),
        ("[", "]", NodeShape::Rect),
        ("(", ")", NodeShape::Round),
        ("{", "}", NodeShape::Rhombus),
    ];
    // For each opener, try matching with its specific closer. Multi-closer shapes
    // (parallelogram vs trapezoid) share the `[/` opener — so when we see `[/`
    // we scan until the first matching closer of either `/]` or `\]`.
    if sc.peek_str("[/") || sc.peek_str("[\\") {
        sc.advance(2);
        let opener_was_slash = sc.s.as_bytes()[sc.i - 1] == b'/';
        // Scan text until we hit `/]` or `\]`.
        let (text, used_close) = read_until_either(sc, "/]", "\\]").ok_or_else(|| {
            ParseError::unclosed(line_no, format!("missing closing for node '{id}'"))
        })?;
        let shape = match (opener_was_slash, used_close) {
            (true, "/]") => NodeShape::Parallelogram,
            (true, "\\]") => NodeShape::Trapezoid,
            (false, "\\]") => NodeShape::ParallelogramAlt,
            (false, "/]") => NodeShape::TrapezoidAlt,
            _ => NodeShape::Rect,
        };
        return Ok(finish_node(id, unquote(text.trim()).to_string(), shape, sc));
    }
    for (open, close, shape) in SHAPES {
        if sc.try_consume(open) {
            let text = sc.read_until_unquoted(close).ok_or_else(|| {
                ParseError::unclosed(
                    line_no,
                    format!("missing closing '{close}' for node '{id}'"),
                )
            })?;
            let _ = sc.try_consume(close);
            return Ok(finish_node(
                id,
                unquote(text.trim()).to_string(),
                *shape,
                sc,
            ));
        }
    }
    let text = id.clone();
    Ok(finish_node(id, text, NodeShape::Rect, sc))
}

/// Parse the v11 `id@{ key: value, … }` attribute block. `shape` maps a named
/// shape onto a `NodeShape` (unknown names fall back to `Rect`, matching
/// upstream); `label`/`title` set the node text. `icon`/`img` forms are out of
/// scope — dropped, but any `label` is still honored so content is never lost.
fn parse_at_node(id: String, sc: &mut Scanner<'_>, line_no: usize) -> Result<FlowNode, ParseError> {
    sc.advance(2); // consume `@{`
    let body = sc.read_until_unquoted("}").ok_or_else(|| {
        ParseError::unclosed(line_no, format!("missing closing '}}' for node '{id}'"))
    })?;
    sc.try_consume("}");

    let mut text = id.clone();
    let mut shape = NodeShape::Rect;
    for (key, value) in split_attrs(&body) {
        match key.as_str() {
            "shape" => shape = shape_from_name(&value),
            "label" | "title" => text = value,
            _ => {} // icon/img and any other keys are dropped
        }
    }
    Ok(finish_node(id, text, shape, sc))
}

/// Split an attribute block body into `(key, value)` pairs. Commas separate
/// pairs and `:` separates a key from its value; both are honored only outside
/// quotes so a quoted value may embed either character. Values are unquoted.
fn split_attrs(body: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    for part in split_unquoted(body, ',') {
        if let Some((k, v)) = part.split_once(':') {
            pairs.push((k.trim().to_string(), unquote(v.trim()).to_string()));
        }
    }
    pairs
}

/// Map a v11 named shape onto an existing `NodeShape`. Aliases follow upstream
/// Mermaid; visual-only shapes still without a variant (e.g. `sm-circ`, `fork`,
/// `text`) fall back to `Rect` so their content is still rendered. Unknown names
/// likewise fall back to `Rect`.
fn shape_from_name(name: &str) -> NodeShape {
    match name.trim() {
        "rounded" | "event" => NodeShape::Round,
        "stadium" | "pill" | "term" | "terminal" => NodeShape::Stadium,
        "subproc" | "subprocess" | "subroutine" | "fr-rect" | "framed-rectangle" => {
            NodeShape::Subroutine
        }
        "cyl" | "cylinder" | "database" | "db" => NodeShape::Cylinder,
        "circle" | "circ" => NodeShape::Circle,
        "dbl-circ" | "double-circle" => NodeShape::DoubleCircle,
        "diam" | "diamond" | "decision" | "question" => NodeShape::Rhombus,
        "hex" | "hexagon" | "prepare" => NodeShape::Hexagon,
        "lean-r" | "lean-right" | "in-out" => NodeShape::Parallelogram,
        "lean-l" | "lean-left" | "out-in" => NodeShape::ParallelogramAlt,
        "trap-b" | "trapezoid-bottom" | "trapezoid" | "priority" => NodeShape::Trapezoid,
        "trap-t" | "trapezoid-top" | "inv-trapezoid" | "manual" => NodeShape::TrapezoidAlt,
        "odd" => NodeShape::Asymmetric,
        "notch-rect" | "card" | "notched-rectangle" => NodeShape::NotchedRect,
        "doc" | "document" => NodeShape::Document,
        "docs" | "documents" | "st-doc" | "stacked-document" => NodeShape::MultiDocument,
        "tag-doc" | "tagged-document" => NodeShape::TaggedDocument,
        "bolt" | "com-link" | "lightning-bolt" => NodeShape::LightningBolt,
        "hourglass" | "collate" => NodeShape::Hourglass,
        "brace" | "brace-l" | "brace-r" | "braces" | "comment" => NodeShape::Comment,
        "delay" | "half-rounded-rectangle" => NodeShape::Delay,
        "das" | "h-cyl" | "horizontal-cylinder" => NodeShape::DirectAccessStorage,
        "lin-cyl" | "disk" | "lined-cylinder" => NodeShape::LinedCylinder,
        "lin-rect" | "lin-proc" | "lined-process" | "lined-rectangle" | "shaded-process" => {
            NodeShape::LinedProcess
        }
        "div-rect" | "div-proc" | "divided-rectangle" | "divided-process" => {
            NodeShape::DividedProcess
        }
        "win-pane" | "window-pane" | "internal-storage" => NodeShape::WindowPane,
        "tri" | "triangle" | "extract" => NodeShape::Triangle,
        "flip-tri" | "flipped-triangle" | "manual-file" => NodeShape::FlippedTriangle,
        "f-circ" | "filled-circle" | "junction" => NodeShape::FilledCircle,
        "cross-circ" | "crossed-circle" | "summary" => NodeShape::CrossedCircle,
        "flag" | "paper-tape" => NodeShape::PaperTape,
        "bow-rect" | "bow-tie-rectangle" | "stored-data" => NodeShape::StoredData,
        _ => NodeShape::Rect,
    }
}

/// Build a node, consuming an optional `:::class` shorthand that follows the
/// id/shape (no whitespace allowed before `:::`, per Mermaid).
fn finish_node(id: String, text: String, shape: NodeShape, sc: &mut Scanner<'_>) -> FlowNode {
    let mut classes = Vec::new();
    if sc.try_consume(":::") {
        if let Some(name) = sc.read_ident() {
            classes.push(name);
        }
    }
    FlowNode {
        id,
        text,
        shape,
        classes,
        style: Style::new(),
        click: None,
    }
}

fn read_until_either<'a>(
    sc: &mut Scanner<'a>,
    a: &'static str,
    b: &'static str,
) -> Option<(String, &'static str)> {
    let rem = sc.remaining();
    let pa = find_unquoted(rem, a);
    let pb = find_unquoted(rem, b);
    let (pos, tok) = match (pa, pb) {
        (Some(x), Some(y)) => {
            if x <= y {
                (x, a)
            } else {
                (y, b)
            }
        }
        (Some(x), None) => (x, a),
        (None, Some(y)) => (y, b),
        (None, None) => return None,
    };
    let text = rem[..pos].to_string();
    sc.i += pos + tok.len();
    Some((text, tok))
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

#[cfg(test)]
mod tests {
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
}
