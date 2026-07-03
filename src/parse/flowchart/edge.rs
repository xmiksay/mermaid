//! Flowchart edge/arrow scanner.
//!
//! Recognizes the connector tokens (`-->`, `---`, `-.->`, `==>`, `--o`, `--x`,
//! the bidirectional `<-->`/`o--o`/`x--x`, and the invisible `~~~`), their pipe
//! and inline edge labels, plus the Mermaid v11 edge-id forms — the `A e1@--> B`
//! prefix and the standalone `e1@{ … }` edge-attribute statement.

use super::super::ast::{EdgeHead, EdgeLine};
use super::super::token::unquote;
use super::super::ParseError;
use super::scanner::Scanner;

/// Consume an optional v11 edge-id prefix `id@` sitting between a node and its
/// connector (`A e1@--> B`). Returns the id when the `@` is immediately followed
/// by a connector opener; otherwise leaves the scanner untouched. The id is not
/// stored on the edge — only recorded so the paired `id@{ … }` statement is
/// recognized.
pub(super) fn consume_edge_id(sc: &mut Scanner<'_>) -> Option<String> {
    let save = sc.i;
    let id = sc.read_ident()?;
    if !sc.try_consume("@") {
        sc.i = save;
        return None;
    }
    match sc.remaining().chars().next() {
        Some('-' | '=' | '.' | '<' | '~' | 'o' | 'x') => Some(id),
        _ => {
            sc.i = save;
            None
        }
    }
}

/// If `line` is exactly a v11 edge-attribute statement `id@{ … }` (an id, `@{`,
/// a body, a closing `}`, and nothing more), return the id. Used to drop such a
/// statement when `id` names a known edge instead of spawning a phantom node.
pub(super) fn edge_attr_stmt_id(line: &str) -> Option<String> {
    let mut sc = Scanner::new(line);
    let id = sc.read_ident()?;
    if !sc.peek_str("@{") {
        return None;
    }
    sc.advance(2);
    sc.read_until("}")?;
    sc.try_consume("}");
    sc.skip_ws();
    sc.eof().then_some(id)
}

/// The shape of an edge connector: `(line, tail head, arrow head, label)`.
type ArrowSpec = (EdgeLine, EdgeHead, EdgeHead, Option<String>);

pub(super) fn parse_arrow(
    sc: &mut Scanner<'_>,
    line_no: usize,
) -> Result<Option<ArrowSpec>, ParseError> {
    sc.skip_ws();
    let tail_start = sc.i;
    // Optional start-side head for bidirectional edges: `<-->`, `o--o`, `x--x`.
    // `<` is unambiguous; `o`/`x` are only a tail marker when a line char
    // (`-`, `=`, `.`) immediately follows, so a node id like `o` stays a node.
    let mut chars = sc.remaining().chars();
    let tail = match chars.next() {
        Some('<') => {
            sc.advance(1);
            EdgeHead::Arrow
        }
        Some('o') if matches!(chars.next(), Some('-') | Some('=') | Some('.')) => {
            sc.advance(1);
            EdgeHead::Circle
        }
        Some('x') if matches!(chars.next(), Some('-') | Some('=') | Some('.')) => {
            sc.advance(1);
            EdgeHead::Cross
        }
        _ => EdgeHead::None,
    };
    // Edge tokens always start with one of `-`, `.`, `=`, `~`. Reject anything
    // else. `~~~` is the invisible link: it lays out like an edge but is not
    // drawn, and never carries a head or a tail marker.
    let first = match sc.remaining().chars().next() {
        Some(c) if c == '-' || c == '=' || c == '.' || c == '~' => c,
        _ => {
            sc.i = tail_start;
            return Ok(None);
        }
    };
    if first == '~' {
        if tail != EdgeHead::None {
            sc.i = tail_start;
            return Ok(None);
        }
        let start = sc.i;
        while sc.try_consume("~") {}
        if sc.i - start < 3 {
            sc.i = tail_start;
            return Ok(None);
        }
        return Ok(Some((
            EdgeLine::Invisible,
            EdgeHead::None,
            EdgeHead::None,
            None,
        )));
    }

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

    let mut head = if sc.try_consume(">") {
        EdgeHead::Arrow
    } else if sc.try_consume("o") {
        EdgeHead::Circle
    } else if sc.try_consume("x") {
        EdgeHead::Cross
    } else {
        EdgeHead::None
    };

    // Reject lone `-` or `=` that wasn't a real arrow (e.g., inside an id).
    let opener_len = sc.i - start;
    if opener_len < 2 {
        sc.i = tail_start;
        return Ok(None);
    }

    sc.skip_ws();
    let label = if sc.try_consume("|") {
        let txt = sc.read_until("|").ok_or_else(|| ParseError::Syntax {
            message: "unclosed edge label".into(),
            line: line_no,
        })?;
        sc.try_consume("|");
        Some(unquote(txt.trim()).to_string())
    } else if head == EdgeHead::None && opener_len == 2 {
        // Inline edge-label form: `A -- text --> B` (also `-. text .->`,
        // `== text ==>`). The two-char opener with no head is Mermaid's
        // START_LINK; if a matching closer follows, the run between them is
        // the edge label rather than a chain node.
        match read_inline_label(sc, line_style) {
            Some((txt, closer_head)) => {
                head = closer_head;
                Some(unquote(txt.trim()).to_string())
            }
            None => None,
        }
    } else {
        None
    };
    Ok(Some((line_style, tail, head, label)))
}

/// Try to read the inline edge-label form. The scanner is positioned at the
/// label text, just past the two-char opener. On success, consume through the
/// closing arrow and return `(label, closer_head)`; otherwise leave the scanner
/// untouched and return `None` (so the text stays a chain node, unchanged).
fn read_inline_label(sc: &mut Scanner<'_>, style: EdgeLine) -> Option<(String, EdgeHead)> {
    let rem = sc.remaining();
    let bytes = rem.as_bytes();
    let mut p = 0;
    while p < bytes.len() {
        if let Some((end, head)) = match_closer(bytes, p, style) {
            let label = rem[..p].to_string();
            sc.i += end;
            return Some((label, head));
        }
        p += 1;
    }
    None
}

/// If a valid closing arrow of `style` starts at `bytes[p]`, return the byte
/// index just past it and its head. The connector run must be substantial
/// enough that a stray `-`/`=`/`.` inside the label text is not mistaken for
/// the closer: a head-bearing closer needs `>= 2` dashes/equals, and a
/// head-less solid/thick closer needs `>= 3` so a plain `A -- B -- C` chain is
/// left alone.
fn match_closer(bytes: &[u8], p: usize, style: EdgeLine) -> Option<(usize, EdgeHead)> {
    let n = bytes.len();
    let mut j = p;
    let run = match style {
        EdgeLine::Solid => {
            while j < n && bytes[j] == b'-' {
                j += 1;
            }
            j - p
        }
        EdgeLine::Thick => {
            while j < n && bytes[j] == b'=' {
                j += 1;
            }
            j - p
        }
        EdgeLine::Dotted => {
            if bytes[j] != b'.' {
                return None;
            }
            j += 1;
            let dash_start = j;
            while j < n && bytes[j] == b'-' {
                j += 1;
            }
            if j == dash_start {
                return None;
            }
            j - p
        }
        // Invisible links (`~~~`) never carry an inline label.
        EdgeLine::Invisible => return None,
    };
    let solid_or_thick = matches!(style, EdgeLine::Solid | EdgeLine::Thick);
    if solid_or_thick && run < 2 {
        return None;
    }
    let head = match bytes.get(j) {
        Some(b'>') => {
            j += 1;
            EdgeHead::Arrow
        }
        Some(b'o') => {
            j += 1;
            EdgeHead::Circle
        }
        Some(b'x') => {
            j += 1;
            EdgeHead::Cross
        }
        _ => EdgeHead::None,
    };
    if head == EdgeHead::None && solid_or_thick && run < 3 {
        return None;
    }
    Some((j, head))
}

#[cfg(test)]
mod tests {
    use super::super::parse;
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
    fn bidirectional_edges() {
        let d = parse("flowchart LR\nA <--> B\nC o--o D\nE x--x F\n").unwrap();
        assert_eq!(d.edges.len(), 3);
        assert_eq!(
            (d.edges[0].tail, d.edges[0].head, d.edges[0].line),
            (EdgeHead::Arrow, EdgeHead::Arrow, EdgeLine::Solid)
        );
        assert_eq!(
            (d.edges[1].tail, d.edges[1].head),
            (EdgeHead::Circle, EdgeHead::Circle)
        );
        assert_eq!(
            (d.edges[2].tail, d.edges[2].head),
            (EdgeHead::Cross, EdgeHead::Cross)
        );
        // A leading `o`/`x` node is not swallowed as a tail marker.
        assert!(d.nodes.iter().any(|n| n.id == "C"));
        assert!(d.nodes.iter().any(|n| n.id == "E"));
    }

    #[test]
    fn bidirectional_edges_thick_and_dotted() {
        let d = parse("flowchart LR\nA <==> B\nC <-.-> D\n").unwrap();
        assert_eq!(d.edges.len(), 2);
        assert_eq!(
            (d.edges[0].tail, d.edges[0].head, d.edges[0].line),
            (EdgeHead::Arrow, EdgeHead::Arrow, EdgeLine::Thick)
        );
        assert_eq!(
            (d.edges[1].tail, d.edges[1].head, d.edges[1].line),
            (EdgeHead::Arrow, EdgeHead::Arrow, EdgeLine::Dotted)
        );
    }

    #[test]
    fn leading_o_node_is_not_a_tail_marker() {
        // `o` and `x` as bare node ids in the chain position must stay nodes.
        let d = parse("flowchart LR\nA --- o\no --> B\n").unwrap();
        assert!(d.nodes.iter().any(|n| n.id == "o"));
        assert!(d.edges.iter().all(|e| e.tail == EdgeHead::None));
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
    fn edge_label() {
        let d = parse("flowchart TD\nA -->|yes| B\n").unwrap();
        assert_eq!(d.edges[0].label.as_deref(), Some("yes"));
    }

    #[test]
    fn inline_edge_label() {
        let d = parse("flowchart LR\nA -- yes --> B\n").unwrap();
        // No phantom `yes` node — just A and B with one labeled edge.
        assert_eq!(d.nodes.len(), 2);
        assert_eq!(d.edges.len(), 1);
        assert_eq!(d.edges[0].label.as_deref(), Some("yes"));
        assert_eq!(d.edges[0].line, EdgeLine::Solid);
        assert_eq!(d.edges[0].head, EdgeHead::Arrow);
    }

    #[test]
    fn inline_edge_label_dotted_and_thick() {
        let d = parse("flowchart LR\nA -. maybe .-> B\nB == no ==> C\n").unwrap();
        assert_eq!(d.nodes.len(), 3);
        assert_eq!(d.edges.len(), 2);
        assert_eq!(d.edges[0].label.as_deref(), Some("maybe"));
        assert_eq!(d.edges[0].line, EdgeLine::Dotted);
        assert_eq!(d.edges[0].head, EdgeHead::Arrow);
        assert_eq!(d.edges[1].label.as_deref(), Some("no"));
        assert_eq!(d.edges[1].line, EdgeLine::Thick);
        assert_eq!(d.edges[1].head, EdgeHead::Arrow);
    }

    #[test]
    fn inline_edge_label_multiword_and_nohead() {
        let d = parse("flowchart LR\nA -- two words --- B\n").unwrap();
        assert_eq!(d.nodes.len(), 2);
        assert_eq!(d.edges.len(), 1);
        assert_eq!(d.edges[0].label.as_deref(), Some("two words"));
        assert_eq!(d.edges[0].head, EdgeHead::None);
    }

    #[test]
    fn invisible_link_parses_as_edge() {
        // `~~~` is an invisible link: a real edge (shapes layout) with no head
        // and no tail. It must not error nor leave `~~~ B` as stray text.
        let d = parse("flowchart TD\nA ~~~ B\nA --> C\n").unwrap();
        assert_eq!(d.edges.len(), 2);
        let inv = &d.edges[0];
        assert_eq!(inv.from, "A");
        assert_eq!(inv.to, "B");
        assert_eq!(inv.line, EdgeLine::Invisible);
        assert_eq!(inv.head, EdgeHead::None);
        assert_eq!(inv.tail, EdgeHead::None);
    }

    #[test]
    fn lone_tilde_is_not_an_edge() {
        // A single/double `~` is not a valid invisible link, so `~` stays text.
        assert!(parse("flowchart TD\nA ~~ B\n").is_err());
    }

    #[test]
    fn plain_nohead_chain_not_labeled() {
        // `A -- B -- C` has no closing arrow, so it stays a plain chain and
        // `B` is a real node (not an edge label).
        let d = parse("flowchart LR\nA -- B -- C\n").unwrap();
        assert!(d.nodes.iter().any(|n| n.id == "B"));
        assert!(d.edges.iter().all(|e| e.label.is_none()));
    }

    #[test]
    fn v11_edge_id_prefix_parsed_and_ignored() {
        // `A e1@--> B` is a normal solid arrow; the `e1@` edge id is dropped.
        let d = parse("flowchart TD\nA e1@--> B\n").unwrap();
        assert_eq!(d.nodes.len(), 2);
        assert_eq!(d.edges.len(), 1);
        assert_eq!(d.edges[0].from, "A");
        assert_eq!(d.edges[0].to, "B");
        assert_eq!(d.edges[0].line, EdgeLine::Solid);
        assert_eq!(d.edges[0].head, EdgeHead::Arrow);
        // No phantom `e1` node.
        assert!(!d.nodes.iter().any(|n| n.id == "e1"));
    }

    #[test]
    fn v11_edge_attr_statement_is_dropped() {
        // `e1@{ animate: true }` referencing a known edge id spawns no node.
        let d = parse("flowchart TD\nA e1@--> B\ne1@{ animate: true }\n").unwrap();
        assert_eq!(d.edges.len(), 1);
        assert_eq!(d.nodes.len(), 2);
        assert!(!d.nodes.iter().any(|n| n.id == "e1"));
    }
}
