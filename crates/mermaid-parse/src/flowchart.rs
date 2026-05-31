//! Flowchart parser (subset for v0.1).
//!
//! Supports:
//!   * `flowchart <DIR>` / `graph <DIR>` header (DIR ∈ TD/TB/BT/LR/RL).
//!   * Node specs with shapes: `id`, `id[text]`, `id(text)`, `id((text))`,
//!     `id{text}`, `id{{text}}`, `id[[text]]`, `id[(text)]`, `id([text])`.
//!   * Edge chains in a single line:
//!     `A --> B --> C`
//!     `A -.-> B --x C` (cross not supported — only solid/dotted/thick/no-arrow)
//!     `A -->|label| B`
//!   * Edge kinds: `-->`, `---`, `-.->`, `==>`.
//!
//! Skipped statements (consumed silently): `subgraph`, `end`, `style`,
//! `classDef`, `class`, `click`, `linkStyle`. Anything else is a syntax error.

use std::collections::HashMap;

use crate::ast::{EdgeKind, FlowDirection, FlowEdge, FlowNode, FlowchartDiagram, NodeShape};
use crate::{strip_comment, ParseError};

pub(crate) fn parse(input: &str) -> Result<FlowchartDiagram, ParseError> {
    let mut diag = FlowchartDiagram::default();
    let mut header_seen = false;
    let mut nodes_by_id: HashMap<String, usize> = HashMap::new();

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

        if is_unsupported_statement(line) {
            continue;
        }

        parse_statement(line, &mut diag, &mut nodes_by_id, line_no)?;
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(diag)
}

fn parse_header(
    line: &str,
    diag: &mut FlowchartDiagram,
    line_no: usize,
) -> Result<(), ParseError> {
    let (kw_len, rest) = if let Some(r) = line.strip_prefix("flowchart") {
        (9, r)
    } else if let Some(r) = line.strip_prefix("graph") {
        (5, r)
    } else {
        return Err(ParseError::Syntax {
            message: "expected 'flowchart' or 'graph' header".into(),
            line: line_no,
        });
    };
    // Reject prefix-of-an-identifier matches: keyword must be followed by EOL or whitespace.
    if let Some(first) = rest.chars().next() {
        if !first.is_whitespace() {
            return Err(ParseError::Syntax {
                message: "expected 'flowchart' or 'graph' header".into(),
                line: line_no,
            });
        }
    }
    let _ = kw_len;
    let dir = rest.trim();
    diag.direction = match dir {
        "" | "TD" | "TB" => FlowDirection::TopDown,
        "BT" => FlowDirection::BottomTop,
        "LR" => FlowDirection::LeftRight,
        "RL" => FlowDirection::RightLeft,
        other => {
            return Err(ParseError::Syntax {
                message: format!("unknown direction: '{other}'"),
                line: line_no,
            })
        }
    };
    Ok(())
}

fn is_unsupported_statement(line: &str) -> bool {
    const UNSUPPORTED: &[&str] = &[
        "subgraph", "end", "style", "classDef", "class ", "click ", "linkStyle",
    ];
    UNSUPPORTED.iter().any(|k| line.starts_with(k)) || line == "end"
}

fn parse_statement(
    line: &str,
    diag: &mut FlowchartDiagram,
    nodes_by_id: &mut HashMap<String, usize>,
    line_no: usize,
) -> Result<(), ParseError> {
    let mut sc = Scanner::new(line);
    let first = parse_node_spec(&mut sc, line_no)?;
    register_node(diag, nodes_by_id, first.clone());
    let mut last_id = first.id;

    loop {
        sc.skip_ws();
        if sc.eof() {
            break;
        }
        let Some((kind, label)) = parse_arrow(&mut sc, line_no)? else {
            return Err(ParseError::Syntax {
                message: format!("unexpected text: '{}'", sc.remaining()),
                line: line_no,
            });
        };
        sc.skip_ws();
        let next_node = parse_node_spec(&mut sc, line_no)?;
        register_node(diag, nodes_by_id, next_node.clone());
        diag.edges.push(FlowEdge {
            from: last_id.clone(),
            to: next_node.id.clone(),
            label,
            kind,
        });
        last_id = next_node.id;
    }
    Ok(())
}

fn register_node(
    diag: &mut FlowchartDiagram,
    by_id: &mut HashMap<String, usize>,
    node: FlowNode,
) {
    if let Some(&idx) = by_id.get(&node.id) {
        // Replace placeholder text/shape if a more specific spec arrives later.
        let existing = &mut diag.nodes[idx];
        if existing.shape == NodeShape::Rect && existing.text == existing.id && node.text != node.id
        {
            existing.text = node.text;
            existing.shape = node.shape;
        }
        return;
    }
    by_id.insert(node.id.clone(), diag.nodes.len());
    diag.nodes.push(node);
}

// ---- node spec parsing -----------------------------------------------------

fn parse_node_spec(sc: &mut Scanner<'_>, line_no: usize) -> Result<FlowNode, ParseError> {
    sc.skip_ws();
    let id = sc.read_ident().ok_or_else(|| ParseError::Syntax {
        message: format!("expected node identifier at: '{}'", sc.remaining()),
        line: line_no,
    })?;
    // Try shapes in length order so multi-char openers win over their prefixes.
    const SHAPES: &[(&str, &str, NodeShape)] = &[
        ("([", "])", NodeShape::Stadium),
        ("[[", "]]", NodeShape::Subroutine),
        ("[(", ")]", NodeShape::Cylinder),
        ("((", "))", NodeShape::Circle),
        ("{{", "}}", NodeShape::Hexagon),
        ("[", "]", NodeShape::Rect),
        ("(", ")", NodeShape::Round),
        ("{", "}", NodeShape::Rhombus),
    ];
    for (open, close, shape) in SHAPES {
        if sc.try_consume(open) {
            let text = sc.read_until(close).ok_or_else(|| ParseError::Syntax {
                message: format!("missing closing '{close}' for node '{id}'"),
                line: line_no,
            })?;
            let _ = sc.try_consume(close);
            return Ok(FlowNode {
                id,
                text: unquote(text.trim()),
                shape: *shape,
            });
        }
    }
    Ok(FlowNode {
        text: id.clone(),
        id,
        shape: NodeShape::Rect,
    })
}

fn unquote(s: &str) -> String {
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

// ---- arrow parsing ---------------------------------------------------------

fn parse_arrow(
    sc: &mut Scanner<'_>,
    line_no: usize,
) -> Result<Option<(EdgeKind, Option<String>)>, ParseError> {
    // Longest-match across known arrow tokens.
    const ARROWS: &[(&str, EdgeKind)] = &[
        ("-.->", EdgeKind::Dotted),
        ("==>", EdgeKind::Thick),
        ("-->", EdgeKind::Solid),
        ("---", EdgeKind::SolidNoArrow),
    ];
    let mut chosen: Option<(usize, &str, EdgeKind)> = None;
    for (tok, kind) in ARROWS {
        if sc.peek_str(tok) {
            match chosen {
                Some((l, _, _)) if l >= tok.len() => {}
                _ => chosen = Some((tok.len(), tok, *kind)),
            }
        }
    }
    let (len, _tok, kind) = match chosen {
        Some(c) => c,
        None => return Ok(None),
    };
    sc.advance(len);

    // Optional `|label|`
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
    Ok(Some((kind, label)))
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
            if c.is_alphanumeric() || c == '_' || c == '-' || c == '.' {
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
    /// Read until `terminator` (not including it). Returns None if not found.
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
        let s = "flowchart TD\nA --> B --> C\n";
        let d = parse(s).unwrap();
        assert_eq!(d.direction, FlowDirection::TopDown);
        assert_eq!(d.nodes.len(), 3);
        assert_eq!(d.edges.len(), 2);
        assert_eq!(d.edges[0].from, "A");
        assert_eq!(d.edges[0].to, "B");
        assert_eq!(d.edges[1].from, "B");
        assert_eq!(d.edges[1].to, "C");
    }

    #[test]
    fn all_shapes() {
        let s = "flowchart TD\n\
             A[Rect] --> B(Round)\n\
             B --> C((Circle))\n\
             C --> D{Decision}\n\
             D --> E{{Hex}}\n\
             E --> F[[Sub]]\n\
             F --> G[(Db)]\n\
             G --> H([Stadium])\n";
        let d = parse(s).unwrap();
        let shapes: Vec<_> = d.nodes.iter().map(|n| (n.id.clone(), n.shape)).collect();
        assert!(shapes.contains(&("A".into(), NodeShape::Rect)));
        assert!(shapes.contains(&("B".into(), NodeShape::Round)));
        assert!(shapes.contains(&("C".into(), NodeShape::Circle)));
        assert!(shapes.contains(&("D".into(), NodeShape::Rhombus)));
        assert!(shapes.contains(&("E".into(), NodeShape::Hexagon)));
        assert!(shapes.contains(&("F".into(), NodeShape::Subroutine)));
        assert!(shapes.contains(&("G".into(), NodeShape::Cylinder)));
        assert!(shapes.contains(&("H".into(), NodeShape::Stadium)));
    }

    #[test]
    fn edge_kinds() {
        let s = "flowchart TD\nA --> B\nA --- B\nA -.-> B\nA ==> B\n";
        let d = parse(s).unwrap();
        let kinds: Vec<_> = d.edges.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            vec![
                EdgeKind::Solid,
                EdgeKind::SolidNoArrow,
                EdgeKind::Dotted,
                EdgeKind::Thick
            ]
        );
    }

    #[test]
    fn edge_label() {
        let s = "flowchart TD\nA -->|yes| B\nA -->|\"with spaces\"| C\n";
        let d = parse(s).unwrap();
        assert_eq!(d.edges[0].label.as_deref(), Some("yes"));
        assert_eq!(d.edges[1].label.as_deref(), Some("with spaces"));
    }

    #[test]
    fn directions() {
        for (input, want) in [
            ("flowchart TD\nA --> B", FlowDirection::TopDown),
            ("flowchart TB\nA --> B", FlowDirection::TopDown),
            ("flowchart BT\nA --> B", FlowDirection::BottomTop),
            ("flowchart LR\nA --> B", FlowDirection::LeftRight),
            ("flowchart RL\nA --> B", FlowDirection::RightLeft),
            ("graph LR\nA --> B", FlowDirection::LeftRight),
        ] {
            assert_eq!(parse(input).unwrap().direction, want, "input: {input}");
        }
    }

    #[test]
    fn inline_text_overrides_implicit_label() {
        let s = "flowchart TD\nA --> B\nA[Apple]\nB(Banana)\n";
        let d = parse(s).unwrap();
        let a = d.nodes.iter().find(|n| n.id == "A").unwrap();
        let b = d.nodes.iter().find(|n| n.id == "B").unwrap();
        assert_eq!(a.text, "Apple");
        assert_eq!(a.shape, NodeShape::Rect);
        assert_eq!(b.text, "Banana");
        assert_eq!(b.shape, NodeShape::Round);
    }

    #[test]
    fn skips_unsupported_keywords() {
        let s = "flowchart TD\nA --> B\nsubgraph S\nA --> C\nend\nclassDef foo fill:#fff\n";
        let d = parse(s).unwrap();
        assert!(d.edges.iter().any(|e| e.from == "A" && e.to == "B"));
        assert!(d.edges.iter().any(|e| e.from == "A" && e.to == "C"));
    }

    #[test]
    fn rejects_bad_arrow() {
        let err = parse("flowchart TD\nA wat B\n").unwrap_err();
        match err {
            ParseError::Syntax { line, .. } => assert_eq!(line, 2),
            e => panic!("unexpected: {e:?}"),
        }
    }
}
