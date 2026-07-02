//! Flowchart parser.
//!
//! Supports:
//!   * `flowchart <DIR>` / `graph <DIR>` header.
//!   * Node shapes: rect `[]`, round `()`, stadium `([ ])`, subroutine `[[]]`,
//!     cylinder `[( )]`, circle `(())`, double circle `((()))`, rhombus `{}`,
//!     hexagon `{{}}`, parallelogram `[/ /]`, parallelogram-alt `[\ \]`,
//!     trapezoid `[/ \]`, trapezoid-alt `[\ /]`, asymmetric flag `> ]`.
//!   * Edge tokens combine a line style (`-`, `.`, `=`) and head (`>`, `o`,
//!     `x`, none): `-->`, `---`, `-.->`, `-.-`, `==>`, `===`, `--o`, `--x`.
//!   * Multi-source / multi-target via `&`: `A & B --> C & D` produces all
//!     four edges.
//!   * `subgraph <id> [label]` ... `end` blocks tracked in `subgraphs`,
//!     including nesting.
//!   * `click <id> …` binds a hyperlink or JS callback to a node.
//!   * Skipped quietly: `style`, `classDef`, `class`, `linkStyle`.

use std::collections::HashMap;

use super::ast::{
    ClickAction, EdgeHead, EdgeLine, FlowDirection, FlowEdge, FlowNode, FlowchartDiagram,
    NodeShape, Style, Subgraph,
};
use super::style::parse_style_props;
use super::{strip_comment, ParseError};

pub(crate) fn parse(input: &str) -> Result<FlowchartDiagram, ParseError> {
    let mut diag = FlowchartDiagram::default();
    let mut header_seen = false;
    let mut nodes_by_id: HashMap<String, usize> = HashMap::new();
    let mut subgraph_stack: Vec<usize> = Vec::new();
    let mut subgraph_auto_id = 0usize;

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw).trim();
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
            handle_style(rest, &mut diag, &mut nodes_by_id);
            continue;
        }
        if let Some(rest) = line.strip_prefix("classDef ") {
            handle_class_def(rest, &mut diag);
            continue;
        }
        if let Some(rest) = line.strip_prefix("class ") {
            handle_class_apply(rest, &mut diag, &mut nodes_by_id);
            continue;
        }
        if let Some(rest) = line.strip_prefix("linkStyle ") {
            handle_link_style(rest, &mut diag);
            continue;
        }
        if let Some(rest) = line.strip_prefix("click ") {
            if let Some((id, action)) = parse_click(rest) {
                let idx = node_index(&mut diag, &mut nodes_by_id, &id);
                diag.nodes[idx].click = Some(action);
            }
            continue;
        }
        if line.starts_with("direction ") {
            continue;
        }

        let added_node_ids = parse_statement(line, &mut diag, &mut nodes_by_id, line_no)?;
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
    Ok(diag)
}

fn parse_header(line: &str, diag: &mut FlowchartDiagram, line_no: usize) -> Result<(), ParseError> {
    let rest = if let Some(r) = line.strip_prefix("flowchart") {
        r
    } else if let Some(r) = line.strip_prefix("graph") {
        r
    } else {
        return Err(ParseError::Syntax {
            message: "expected 'flowchart' or 'graph' header".into(),
            line: line_no,
        });
    };
    if let Some(c) = rest.chars().next() {
        if !c.is_whitespace() {
            return Err(ParseError::Syntax {
                message: "expected 'flowchart' or 'graph' header".into(),
                line: line_no,
            });
        }
    }
    diag.direction = parse_direction(rest.trim()).ok_or_else(|| ParseError::Syntax {
        message: format!("unknown direction: '{}'", rest.trim()),
        line: line_no,
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
    });
    if let Some(&parent) = stack.last() {
        diag.subgraphs[parent].child_subgraph_ids.push(id);
    }
    stack.push(new_idx);
}

/// Index of the node with `id`, creating a bare rectangle placeholder if a
/// directive references it before it is declared.
fn node_index(
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
fn handle_style(rest: &str, diag: &mut FlowchartDiagram, nodes_by_id: &mut HashMap<String, usize>) {
    let Some((id, props)) = rest.trim().split_once(char::is_whitespace) else {
        return;
    };
    let style = parse_style_props(props);
    let idx = node_index(diag, nodes_by_id, id.trim());
    diag.nodes[idx].style = style;
}

/// `classDef <name>[,<name2>] <props>` — define one or more style classes.
fn handle_class_def(rest: &str, diag: &mut FlowchartDiagram) {
    let Some((names, props)) = rest.trim().split_once(char::is_whitespace) else {
        return;
    };
    let style = parse_style_props(props);
    for name in names.split(',') {
        let name = name.trim();
        if !name.is_empty() {
            diag.class_defs.insert(name.to_string(), style.clone());
        }
    }
}

/// `class <id1>,<id2> <className>` — apply a class to nodes.
fn handle_class_apply(
    rest: &str,
    diag: &mut FlowchartDiagram,
    nodes_by_id: &mut HashMap<String, usize>,
) {
    let Some((ids, class_name)) = rest.trim().rsplit_once(char::is_whitespace) else {
        return;
    };
    let class_name = class_name.trim();
    if class_name.is_empty() {
        return;
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
}

/// `linkStyle <default|idx-list> [interpolate <curve>] <props>` — style edges
/// by their definition index. The optional `interpolate <curve>` clause is
/// accepted but ignored (curve is fixed to basis).
fn handle_link_style(rest: &str, diag: &mut FlowchartDiagram) {
    let Some((selector, props)) = rest.trim().split_once(char::is_whitespace) else {
        return;
    };
    let mut props = props.trim();
    if let Some(after) = props.strip_prefix("interpolate ") {
        // Drop `interpolate <curve>`; the remaining (if any) are real props.
        props = after
            .trim()
            .split_once(char::is_whitespace)
            .map_or("", |(_, p)| p);
    }
    let style = parse_style_props(props);
    if selector == "default" {
        diag.link_style_default = style;
        return;
    }
    for idx in selector.split(',') {
        if let Ok(i) = idx.trim().parse::<usize>() {
            diag.edge_styles.insert(i, style.clone());
        }
    }
}

fn parse_statement(
    line: &str,
    diag: &mut FlowchartDiagram,
    nodes_by_id: &mut HashMap<String, usize>,
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
        let Some((line_style, head, label)) = parse_arrow(&mut sc, line_no)? else {
            return Err(ParseError::Syntax {
                message: format!("unexpected text: '{}'", sc.remaining()),
                line: line_no,
            });
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
                    head,
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
    let id = sc.read_ident().ok_or_else(|| ParseError::Syntax {
        message: format!("expected node identifier at: '{}'", sc.remaining()),
        line: line_no,
    })?;

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
        let (text, used_close) =
            read_until_either(sc, "/]", "\\]").ok_or_else(|| ParseError::Syntax {
                message: format!("missing closing for node '{id}'"),
                line: line_no,
            })?;
        let shape = match (opener_was_slash, used_close) {
            (true, "/]") => NodeShape::Parallelogram,
            (true, "\\]") => NodeShape::Trapezoid,
            (false, "\\]") => NodeShape::ParallelogramAlt,
            (false, "/]") => NodeShape::TrapezoidAlt,
            _ => NodeShape::Rect,
        };
        return Ok(finish_node(id, unquote(text.trim()), shape, sc));
    }
    for (open, close, shape) in SHAPES {
        if sc.try_consume(open) {
            let text = sc.read_until(close).ok_or_else(|| ParseError::Syntax {
                message: format!("missing closing '{close}' for node '{id}'"),
                line: line_no,
            })?;
            let _ = sc.try_consume(close);
            return Ok(finish_node(id, unquote(text.trim()), *shape, sc));
        }
    }
    let text = id.clone();
    Ok(finish_node(id, text, NodeShape::Rect, sc))
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

/// Parse a `click <id> …` directive body (text after `click `) into the node
/// id and its bound action. Returns `None` if the line is malformed.
///
/// Recognized forms (tooltips and `_target` are optional throughout):
///   `click A "url" "tooltip" _blank`   → hyperlink
///   `click A href "url" "tooltip"`      → hyperlink
///   `click A callback "tooltip"`        → JS callback
///   `click A call callback() "tooltip"` → JS callback
fn parse_click(rest: &str) -> Option<(String, ClickAction)> {
    let toks = click_tokens(rest);
    let (id_tok, args) = toks.split_first()?;
    let id = id_tok.value.clone();
    let head = args.first()?;

    if !head.quoted && head.value == "href" {
        let url = args.get(1)?.value.clone();
        let (tooltip, target) = tooltip_and_target(&args[2..]);
        return Some((
            id,
            ClickAction::Href {
                url,
                tooltip,
                target,
            },
        ));
    }
    if !head.quoted && head.value == "call" {
        let function = args.get(1)?.value.clone();
        let tooltip = args.get(2).map(|t| t.value.clone());
        return Some((id, ClickAction::Callback { function, tooltip }));
    }
    if head.quoted {
        let url = head.value.clone();
        let (tooltip, target) = tooltip_and_target(&args[1..]);
        return Some((
            id,
            ClickAction::Href {
                url,
                tooltip,
                target,
            },
        ));
    }
    // Bare token → callback function name.
    let function = head.value.clone();
    let tooltip = args.get(1).map(|t| t.value.clone());
    Some((id, ClickAction::Callback { function, tooltip }))
}

struct ClickToken {
    quoted: bool,
    value: String,
}

/// Split a click-directive body into whitespace-delimited tokens, treating a
/// `"…"` run as a single (quoted) token so URLs and tooltips keep their spaces.
fn click_tokens(s: &str) -> Vec<ClickToken> {
    let bytes = s.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        if bytes[i] == b'"' {
            i += 1;
            let start = i;
            while i < bytes.len() && bytes[i] != b'"' {
                i += 1;
            }
            tokens.push(ClickToken {
                quoted: true,
                value: s[start..i].to_string(),
            });
            if i < bytes.len() {
                i += 1; // closing quote
            }
        } else {
            let start = i;
            while i < bytes.len() && bytes[i] != b' ' && bytes[i] != b'\t' {
                i += 1;
            }
            tokens.push(ClickToken {
                quoted: false,
                value: s[start..i].to_string(),
            });
        }
    }
    tokens
}

/// From the trailing tokens of a hyperlink `click`, pick the first quoted token
/// as the tooltip and the first `_`-prefixed bare token (e.g. `_blank`) as the
/// link target.
fn tooltip_and_target(rest: &[ClickToken]) -> (Option<String>, Option<String>) {
    let mut tooltip = None;
    let mut target = None;
    for tok in rest {
        if tok.quoted {
            tooltip.get_or_insert_with(|| tok.value.clone());
        } else if tok.value.starts_with('_') {
            target.get_or_insert_with(|| tok.value.clone());
        }
    }
    (tooltip, target)
}

fn unquote(s: &str) -> String {
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn read_until_either<'a>(
    sc: &mut Scanner<'a>,
    a: &'static str,
    b: &'static str,
) -> Option<(String, &'static str)> {
    let rem = sc.remaining();
    let pa = rem.find(a);
    let pb = rem.find(b);
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

fn parse_arrow(
    sc: &mut Scanner<'_>,
    line_no: usize,
) -> Result<Option<(EdgeLine, EdgeHead, Option<String>)>, ParseError> {
    sc.skip_ws();
    // Edge tokens always start with one of `-`, `.`, `=`. Reject anything else.
    let first = match sc.remaining().chars().next() {
        Some(c) if c == '-' || c == '=' || c == '.' => c,
        _ => return Ok(None),
    };

    // Distinguish thick (`=`) vs solid (`-`) vs dotted (`-.` / `.`).
    // Patterns to recognize (all may have optional head suffix):
    //   `===` thick no-head; `==>` `==o` `==x` thick with head
    //   `---` solid no-head; `-->` `--o` `--x` solid with head
    //   `-.-` dotted no-head; `-.->` `-.-o` `-.-x` dotted with head
    //   `~~~` invisible — treat as solid no-head for v0.1
    let start = sc.i;
    let line_style = if first == '=' {
        // Consume `=` chars until we hit something else.
        while sc.try_consume("=") {}
        EdgeLine::Thick
    } else if sc.peek_str("-.") {
        sc.advance(2);
        // Optional more `.` and `-`
        while sc.try_consume(".") || sc.try_consume("-") {}
        EdgeLine::Dotted
    } else if first == '-' {
        while sc.try_consume("-") {}
        EdgeLine::Solid
    } else if first == '.' {
        while sc.try_consume(".") {}
        EdgeLine::Dotted
    } else {
        return Ok(None);
    };

    let head = if sc.try_consume(">") {
        EdgeHead::Arrow
    } else if sc.try_consume("o") {
        EdgeHead::Circle
    } else if sc.try_consume("x") {
        EdgeHead::Cross
    } else {
        EdgeHead::None
    };

    // Reject lone `-` or `=` that wasn't a real arrow (e.g., inside an id).
    if sc.i - start < 2 {
        sc.i = start;
        return Ok(None);
    }

    sc.skip_ws();
    let label = if sc.try_consume("|") {
        let txt = sc.read_until("|").ok_or_else(|| ParseError::Syntax {
            message: "unclosed edge label".into(),
            line: line_no,
        })?;
        sc.try_consume("|");
        Some(unquote(txt.trim()))
    } else {
        None
    };
    Ok(Some((line_style, head, label)))
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

// ---- tiny scanner ----------------------------------------------------------

struct Scanner<'a> {
    s: &'a str,
    i: usize,
}

impl<'a> Scanner<'a> {
    fn new(s: &'a str) -> Self {
        Self { s, i: 0 }
    }
    fn eof(&self) -> bool {
        self.i >= self.s.len()
    }
    fn remaining(&self) -> &'a str {
        &self.s[self.i..]
    }
    fn peek_str(&self, prefix: &str) -> bool {
        self.remaining().starts_with(prefix)
    }
    fn try_consume(&mut self, prefix: &str) -> bool {
        if self.peek_str(prefix) {
            self.i += prefix.len();
            true
        } else {
            false
        }
    }
    fn advance(&mut self, n: usize) {
        self.i += n;
    }
    fn skip_ws(&mut self) {
        while let Some(c) = self.remaining().chars().next() {
            if c == ' ' || c == '\t' {
                self.i += c.len_utf8();
            } else {
                break;
            }
        }
    }
    fn read_ident(&mut self) -> Option<String> {
        let mut end = 0;
        for c in self.remaining().chars() {
            if c.is_alphanumeric() || c == '_' || c == '.' {
                end += c.len_utf8();
            } else {
                break;
            }
        }
        if end == 0 {
            return None;
        }
        let s = self.remaining()[..end].to_string();
        self.i += end;
        Some(s)
    }
    fn read_until(&mut self, terminator: &str) -> Option<String> {
        let rem = self.remaining();
        let pos = rem.find(terminator)?;
        let s = rem[..pos].to_string();
        self.i += pos;
        Some(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_chain() {
        let d = parse("flowchart TD\nA --> B --> C\n").unwrap();
        assert_eq!(d.nodes.len(), 3);
        assert_eq!(d.edges.len(), 2);
        assert_eq!(d.edges[0].line, EdgeLine::Solid);
        assert_eq!(d.edges[0].head, EdgeHead::Arrow);
    }

    #[test]
    fn no_space_arrows() {
        let d = parse("flowchart TD\nA-->B-->C\n").unwrap();
        assert_eq!(d.nodes.len(), 3);
        assert_eq!(d.edges.len(), 2);
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
    fn all_edge_kinds() {
        let d = parse(
            "flowchart TD\nA --> B\nA --- B\nA -.-> B\nA ==> B\nA --o B\nA --x B\nA -.- B\nA === B\n",
        )
        .unwrap();
        let kinds: Vec<_> = d.edges.iter().map(|e| (e.line, e.head)).collect();
        assert!(kinds.contains(&(EdgeLine::Solid, EdgeHead::Arrow)));
        assert!(kinds.contains(&(EdgeLine::Solid, EdgeHead::None)));
        assert!(kinds.contains(&(EdgeLine::Dotted, EdgeHead::Arrow)));
        assert!(kinds.contains(&(EdgeLine::Thick, EdgeHead::Arrow)));
        assert!(kinds.contains(&(EdgeLine::Solid, EdgeHead::Circle)));
        assert!(kinds.contains(&(EdgeLine::Solid, EdgeHead::Cross)));
        assert!(kinds.contains(&(EdgeLine::Dotted, EdgeHead::None)));
        assert!(kinds.contains(&(EdgeLine::Thick, EdgeHead::None)));
    }

    #[test]
    fn multi_source_target_cross_product() {
        let d = parse("flowchart LR\nA & B --> C & D\n").unwrap();
        assert_eq!(d.nodes.len(), 4);
        assert_eq!(d.edges.len(), 4);
        let pairs: Vec<_> = d
            .edges
            .iter()
            .map(|e| (e.from.clone(), e.to.clone()))
            .collect();
        assert!(pairs.contains(&("A".into(), "C".into())));
        assert!(pairs.contains(&("A".into(), "D".into())));
        assert!(pairs.contains(&("B".into(), "C".into())));
        assert!(pairs.contains(&("B".into(), "D".into())));
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
    fn edge_label() {
        let d = parse("flowchart TD\nA -->|yes| B\n").unwrap();
        assert_eq!(d.edges[0].label.as_deref(), Some("yes"));
    }

    #[test]
    fn click_href_with_tooltip() {
        let d =
            parse("flowchart TD\nA-->B\nclick A \"https://example.com\" \"tooltip\"\n").unwrap();
        assert_eq!(d.edges.len(), 1);
        assert_eq!(
            node(&d, "A").click,
            Some(ClickAction::Href {
                url: "https://example.com".into(),
                tooltip: Some("tooltip".into()),
                target: None,
            })
        );
    }

    #[test]
    fn click_href_keyword_and_target() {
        let d = parse("flowchart TD\nA-->B\nclick A href \"http://x\" \"tip\" _blank\n").unwrap();
        assert_eq!(
            node(&d, "A").click,
            Some(ClickAction::Href {
                url: "http://x".into(),
                tooltip: Some("tip".into()),
                target: Some("_blank".into()),
            })
        );
    }

    #[test]
    fn click_callback_bare() {
        let d = parse("flowchart TD\nA-->B\nclick A callback \"a tip\"\n").unwrap();
        assert_eq!(
            node(&d, "A").click,
            Some(ClickAction::Callback {
                function: "callback".into(),
                tooltip: Some("a tip".into()),
            })
        );
    }

    #[test]
    fn click_callback_call_keyword() {
        let d = parse("flowchart TD\nA-->B\nclick A call handler()\n").unwrap();
        assert_eq!(
            node(&d, "A").click,
            Some(ClickAction::Callback {
                function: "handler()".into(),
                tooltip: None,
            })
        );
    }

    #[test]
    fn click_before_node_declared_creates_it() {
        let d = parse("flowchart TD\nclick Z \"http://z\"\nZ-->B\n").unwrap();
        assert!(node(&d, "Z").click.is_some());
        assert_eq!(d.edges.len(), 1);
    }

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
    fn link_style_interpolate_is_ignored() {
        let d = parse("flowchart TD\nA-->B\nlinkStyle 0 interpolate basis stroke:#abc\n").unwrap();
        assert_eq!(
            d.edge_styles[&0],
            vec![("stroke".to_string(), "#abc".to_string())]
        );
    }
}
