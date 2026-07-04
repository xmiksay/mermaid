//! Mindmap parser.
//!
//! Indentation defines parent/child. A line `::icon(fa fa-book)` attaches
//! an icon to the most-recent node at that indent level.

use super::ast::{MindmapDiagram, MindmapNode, MindmapShape};
use super::style::parse_style_props;
use super::token::unquote_any;
use super::{strip_comment, ParseError};

pub(crate) fn parse(input: &str) -> Result<MindmapDiagram, ParseError> {
    let mut header_seen = false;
    let mut stack: Vec<(usize, MindmapNode)> = Vec::new();
    let mut root: Option<MindmapNode> = None;
    let mut diag = MindmapDiagram::default();

    let lines: Vec<&str> = input.lines().collect();
    let mut idx = 0;
    while idx < lines.len() {
        let line_no = idx + 1;
        // Reassemble a multi-line quoted/markdown label. The grammar is
        // line-oriented, but a `"…"` label (e.g. a `` "`**bold**\nmore`" ``
        // markdown string) may span lines. If the line opens a quote that does
        // not close, gather following lines until the quotes balance — the
        // same idea as `collect_init` for multi-line `%%{init}%%`.
        let mut logical = String::from(lines[idx]);
        idx += 1;
        while logical.matches('"').count() % 2 == 1 && idx < lines.len() {
            logical.push('\n');
            logical.push_str(lines[idx]);
            idx += 1;
        }
        let content = strip_comment(&logical);
        if content.trim().is_empty() {
            continue;
        }
        if !header_seen {
            if content.trim() != "mindmap" {
                return Err(ParseError::header(line_no, "expected 'mindmap' header"));
            }
            header_seen = true;
            continue;
        }

        let indent = content
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .count();
        let body = content.trim();
        if body.is_empty() {
            continue;
        }

        // `classDef <name>[,<name2>] <props>` — style classes referenced by a
        // node's `:::class` attachment (shared with the flowchart path).
        if let Some(rest) = body.strip_prefix("classDef ") {
            if let Some((names, props)) = rest.trim().split_once(char::is_whitespace) {
                let style = parse_style_props(props);
                for name in names.split(',') {
                    let name = name.trim();
                    if !name.is_empty() {
                        diag.class_defs.insert(name.to_string(), style.clone());
                    }
                }
            }
            continue;
        }

        // Icon attachment.
        if let Some(rest) = body.strip_prefix("::icon(") {
            let icon = rest.trim_end_matches(')').trim().to_string();
            // Attach to last node at this or deeper indent.
            if let Some((_, n)) = stack.last_mut() {
                n.icon = Some(icon);
            } else if let Some(n) = root.as_mut() {
                n.icon = Some(icon);
            }
            continue;
        }

        // Class attachment: `:::class1 class2` attaches CSS classes to the
        // preceding node instead of creating a literal child node.
        if let Some(rest) = body.strip_prefix(":::") {
            let classes = rest.split_whitespace().map(str::to_string);
            if let Some((_, n)) = stack.last_mut() {
                n.classes.extend(classes);
            } else if let Some(n) = root.as_mut() {
                n.classes.extend(classes);
            }
            continue;
        }

        let node = parse_node(body);

        // Pop deeper levels.
        while let Some((d, _)) = stack.last() {
            if *d >= indent {
                let (_, child) = stack.pop().unwrap();
                if let Some((_, parent)) = stack.last_mut() {
                    parent.children.push(child);
                } else if let Some(r) = root.as_mut() {
                    r.children.push(child);
                } else {
                    root = Some(child);
                }
            } else {
                break;
            }
        }

        stack.push((indent, node));
    }

    // Drain stack.
    while let Some((_, child)) = stack.pop() {
        if let Some((_, parent)) = stack.last_mut() {
            parent.children.push(child);
        } else if let Some(r) = root.as_mut() {
            r.children.push(child);
        } else {
            root = Some(child);
        }
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    diag.root = root;
    Ok(diag)
}

fn parse_node(body: &str) -> MindmapNode {
    let body = body.trim();
    let shape_start = body.find(['(', '[', '{', ')']);
    let (id, shape_part) = match shape_start {
        Some(pos) => (&body[..pos], &body[pos..]),
        None => (body, ""),
    };
    let (shape, label) = if shape_part.is_empty() {
        (MindmapShape::Default, id)
    } else if shape_part.starts_with("(((") && shape_part.ends_with(")))") {
        (MindmapShape::Circle, &shape_part[3..shape_part.len() - 3])
    } else if shape_part.starts_with("((") && shape_part.ends_with("))") {
        (MindmapShape::Circle, &shape_part[2..shape_part.len() - 2])
    } else if shape_part.starts_with("))") && shape_part.ends_with("((") {
        (MindmapShape::Bang, &shape_part[2..shape_part.len() - 2])
    } else if shape_part.starts_with(')') && shape_part.ends_with('(') {
        (MindmapShape::Cloud, &shape_part[1..shape_part.len() - 1])
    } else if shape_part.starts_with("{{") && shape_part.ends_with("}}") {
        (MindmapShape::Hexagon, &shape_part[2..shape_part.len() - 2])
    } else if shape_part.starts_with('[') && shape_part.ends_with(']') {
        (MindmapShape::Square, &shape_part[1..shape_part.len() - 1])
    } else if shape_part.starts_with('(') && shape_part.ends_with(')') {
        (MindmapShape::Rounded, &shape_part[1..shape_part.len() - 1])
    } else {
        (MindmapShape::Default, body)
    };
    let text = label.trim();
    let text = if !text.is_empty() { text } else { id.trim() };
    // Strip the surrounding string delimiters (upstream's NSTR): `"quoted"` →
    // `quoted`, and a markdown string `"` `` `**bold**` `` `"` → `` `**bold**` ``
    // so the backtick-fence machinery in svg/markup.rs styles it.
    let text = unquote_any(text);
    MindmapNode {
        text: text.to_string(),
        shape,
        icon: None,
        classes: Vec::new(),
        children: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_indent() {
        let d = parse("mindmap\nroot((root))\n  A\n    A1\n  B\n").unwrap();
        let r = d.root.unwrap();
        assert_eq!(r.text, "root");
        assert_eq!(r.children.len(), 2);
        assert_eq!(r.children[0].text, "A");
        assert_eq!(r.children[0].children[0].text, "A1");
        assert_eq!(r.children[1].text, "B");
    }

    #[test]
    fn shapes() {
        let d = parse("mindmap\nroot[Sq]\n  (Round)\n  ((Circle))\n  {{Hex}}\n").unwrap();
        let r = d.root.unwrap();
        assert_eq!(r.shape, MindmapShape::Square);
        assert_eq!(r.children[0].shape, MindmapShape::Rounded);
        assert_eq!(r.children[1].shape, MindmapShape::Circle);
        assert_eq!(r.children[2].shape, MindmapShape::Hexagon);
    }

    #[test]
    fn icon() {
        let d = parse("mindmap\nroot\n  A\n  ::icon(fa fa-book)\n").unwrap();
        let r = d.root.unwrap();
        assert_eq!(r.children[0].icon.as_deref(), Some("fa fa-book"));
    }

    #[test]
    fn quoted_label_strips_delimiters() {
        let d = parse("mindmap\nroot\n  A[\"quoted label\"]\n").unwrap();
        let r = d.root.unwrap();
        assert_eq!(r.children[0].text, "quoted label");
        assert_eq!(r.children[0].shape, MindmapShape::Square);
    }

    #[test]
    fn markdown_string_keeps_fence_drops_quotes() {
        let d = parse("mindmap\nroot\n  id1[\"`**Bold** and *italic*`\"]\n").unwrap();
        let r = d.root.unwrap();
        // Quotes stripped, backtick fence preserved for the markup layer.
        assert_eq!(r.children[0].text, "`**Bold** and *italic*`");
    }

    #[test]
    fn multiline_markdown_string_reassembled() {
        // A `"`…`"` markdown string spanning two lines must join into one node,
        // not leak brackets/backticks and spawn a bogus `second line` sibling.
        let d = parse("mindmap\n  id1[\"`**Root**\n  second line`\"]\n").unwrap();
        let r = d.root.unwrap();
        assert_eq!(r.shape, MindmapShape::Square);
        assert_eq!(r.text, "`**Root**\n  second line`");
        assert!(r.children.is_empty());
    }

    #[test]
    fn multiline_plain_quoted_label_reassembled() {
        let d = parse("mindmap\n  id1[\"first\n  second\"]\n").unwrap();
        let r = d.root.unwrap();
        assert_eq!(r.text, "first\n  second");
        assert!(r.children.is_empty());
    }

    #[test]
    fn classdef_collected() {
        let d = parse(
            "mindmap\nroot(Root)\n  A[Node]\n  :::urgent\nclassDef urgent fill:#f00,color:#fff\n",
        )
        .unwrap();
        assert!(d.class_defs.contains_key("urgent"));
        assert_eq!(
            d.class_defs["urgent"],
            vec![
                ("fill".to_string(), "#f00".to_string()),
                ("color".to_string(), "#fff".to_string()),
            ]
        );
    }

    #[test]
    fn class_attaches_not_child() {
        let d = parse("mindmap\n  root(Root)\n    A[Node]\n    :::urgent large\n").unwrap();
        let r = d.root.unwrap();
        assert_eq!(r.text, "Root");
        // The `:::` line must not become a child node.
        assert_eq!(r.children.len(), 1);
        let a = &r.children[0];
        assert_eq!(a.text, "Node");
        assert_eq!(a.classes, vec!["urgent".to_string(), "large".to_string()]);
    }
}
