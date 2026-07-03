//! Treemap parser.
//!
//! ```text
//! treemap-beta
//!     title "Title"
//!     "Section 1"
//!         "Leaf 1.1": 12
//!         "Section 1.2"
//!             "Leaf 1.2.1": 12
//!     "Section 2": 30
//! ```

use super::ast::{TreemapDiagram, TreemapNode};
use super::style::parse_style_props;
use super::{strip_comment, ParseError};

pub(crate) fn parse(input: &str) -> Result<TreemapDiagram, ParseError> {
    let mut d = TreemapDiagram::default();
    let mut header_seen = false;
    let mut stack: Vec<(usize, TreemapNode)> = Vec::new();
    let mut base_indent: Option<usize> = None;

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let content = strip_comment(raw);
        if content.trim().is_empty() {
            continue;
        }
        if !header_seen {
            let trimmed = content.trim();
            if trimmed != "treemap" && trimmed != "treemap-beta" {
                return Err(ParseError::header(line_no, "expected 'treemap' header"));
            }
            header_seen = true;
            continue;
        }

        let indent = content
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .count();
        let body = content.trim();
        if let Some(rest) = body.strip_prefix("title") {
            d.title = Some(rest.trim().trim_matches('"').to_string());
            continue;
        }
        if let Some(rest) = body.strip_prefix("classDef ") {
            if let Some((name, props)) = rest.trim().split_once(char::is_whitespace) {
                d.class_defs
                    .insert(name.trim().to_string(), parse_style_props(props));
            }
            continue;
        }

        if base_indent.is_none() {
            base_indent = Some(indent);
        }

        // A trailing `:::className` attaches a class; strip it before the
        // label/value split so the `:::` can't be mistaken for the value colon.
        let (body, class_name) = match body.split_once(":::") {
            Some((before, after)) => {
                let cls = after
                    .split([':', ' ', '\t'])
                    .find(|s| !s.is_empty())
                    .map(str::to_string);
                (before.trim(), cls)
            }
            None => (body, None),
        };

        let (label, value) = if let Some((l, v)) = body.rsplit_once(':') {
            // Make sure the colon is not inside the label quotes.
            let candidate: Option<f64> = v.trim().parse().ok();
            match candidate {
                Some(num) => (l.trim().trim_matches('"').to_string(), Some(num)),
                None => (body.trim_matches('"').to_string(), None),
            }
        } else {
            (body.trim_matches('"').to_string(), None)
        };

        let node = TreemapNode {
            label,
            value,
            children: Vec::new(),
            class_name,
        };

        // Pop deeper levels.
        while let Some((d_indent, _)) = stack.last() {
            if *d_indent >= indent {
                let (_, child) = stack.pop().unwrap();
                if let Some((_, parent)) = stack.last_mut() {
                    parent.children.push(child);
                } else {
                    d.root.push(child);
                }
            } else {
                break;
            }
        }
        stack.push((indent, node));
    }

    while let Some((_, child)) = stack.pop() {
        if let Some((_, parent)) = stack.last_mut() {
            parent.children.push(child);
        } else {
            d.root.push(child);
        }
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let d = parse("treemap-beta\n\"Section 1\"\n    \"Leaf 1.1\": 12\n    \"Leaf 1.2\": 7\n\"Section 2\": 30\n").unwrap();
        assert_eq!(d.root.len(), 2);
        assert_eq!(d.root[0].label, "Section 1");
        assert_eq!(d.root[0].children.len(), 2);
        assert_eq!(d.root[0].children[0].value, Some(12.0));
        assert_eq!(d.root[1].value, Some(30.0));
    }

    #[test]
    fn classdef_and_class_ref() {
        let d = parse(
            "treemap-beta\nclassDef hot fill:#f00,stroke:#333\n\"Section 1\":::hot\n    \"Leaf 1.1\": 12:::hot\n",
        )
        .unwrap();
        assert!(d.class_defs.contains_key("hot"));
        assert_eq!(d.root[0].label, "Section 1");
        assert_eq!(d.root[0].class_name.as_deref(), Some("hot"));
        let leaf = &d.root[0].children[0];
        assert_eq!(leaf.label, "Leaf 1.1");
        assert_eq!(leaf.value, Some(12.0));
        assert_eq!(leaf.class_name.as_deref(), Some("hot"));
    }
}
