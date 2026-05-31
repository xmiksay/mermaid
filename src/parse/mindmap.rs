//! Mindmap parser.
//!
//! Indentation defines parent/child. A line `::icon(fa fa-book)` attaches
//! an icon to the most-recent node at that indent level.

use super::ast::{MindmapDiagram, MindmapNode, MindmapShape};
use super::{strip_comment, ParseError};

pub(crate) fn parse(input: &str) -> Result<MindmapDiagram, ParseError> {
    let mut header_seen = false;
    let mut stack: Vec<(usize, MindmapNode)> = Vec::new();
    let mut root: Option<MindmapNode> = None;

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let content = strip_comment(raw);
        if content.trim().is_empty() {
            continue;
        }
        if !header_seen {
            if content.trim() != "mindmap" {
                return Err(ParseError::Syntax {
                    message: "expected 'mindmap' header".into(),
                    line: line_no,
                });
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

        // Icon attachment.
        if let Some(rest) = body.strip_prefix("::icon(") {
            let icon = rest.trim_end_matches(')').trim().to_string();
            // Attach to last node at this or deeper indent.
            if let Some((_, n)) = stack.last_mut() {
                n.icon = Some(icon);
                continue;
            } else if let Some(n) = root.as_mut() {
                n.icon = Some(icon);
                continue;
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
    Ok(MindmapDiagram { root })
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
    MindmapNode {
        text: text.to_string(),
        shape,
        icon: None,
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
}
