//! gitGraph parser.
//!
//! Grammar (line-based, each event is one keyword line):
//!
//! ```text
//! gitGraph [TB|LR|BT]
//!     commit
//!     commit id: "x" tag: "v1" type: HIGHLIGHT
//!     branch develop
//!     checkout develop
//!     merge main id: "m1" tag: "rel"
//!     cherry-pick id: "abc"
//! ```

use super::ast::{CommitKind, GitDirection, GitEvent, GitGraphDiagram};
use super::{strip_comment, ParseError};

pub(crate) fn parse(input: &str) -> Result<GitGraphDiagram, ParseError> {
    let mut d = GitGraphDiagram::default();
    let mut header_seen = false;

    for (idx, raw) in input.lines().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if !header_seen {
            let rest = line
                .strip_prefix("gitGraph")
                .ok_or_else(|| ParseError::Syntax {
                    message: "expected 'gitGraph' header".into(),
                    line: line_no,
                })?;
            let rest = rest.trim().trim_start_matches(':').trim();
            d.direction = match rest {
                "" | "LR" => GitDirection::LeftRight,
                "TB" | "TD" => GitDirection::TopDown,
                "BT" => GitDirection::BottomTop,
                _ => GitDirection::LeftRight,
            };
            header_seen = true;
            continue;
        }

        if let Some(rest) = line.strip_prefix("title") {
            d.title = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("commit") {
            let (id, tag, kind) = parse_commit_attrs(rest);
            d.events.push(GitEvent::Commit { id, tag, kind });
        } else if let Some(rest) = line.strip_prefix("branch") {
            d.events.push(GitEvent::Branch {
                name: rest.trim().to_string(),
            });
        } else if let Some(rest) = line.strip_prefix("checkout") {
            d.events.push(GitEvent::Checkout {
                name: rest.trim().to_string(),
            });
        } else if let Some(rest) = line.strip_prefix("switch") {
            d.events.push(GitEvent::Checkout {
                name: rest.trim().to_string(),
            });
        } else if let Some(rest) = line.strip_prefix("merge") {
            let mut iter = rest.split_whitespace();
            let from = iter.next().unwrap_or("").to_string();
            let attrs = iter.collect::<Vec<_>>().join(" ");
            let (id, tag, _) = parse_commit_attrs(&attrs);
            d.events.push(GitEvent::Merge { from, id, tag });
        } else if let Some(rest) = line.strip_prefix("cherry-pick") {
            let (id, _, _) = parse_commit_attrs(rest);
            d.events.push(GitEvent::CherryPick {
                commit_id: id.unwrap_or_default(),
            });
        } else {
            return Err(ParseError::Syntax {
                message: format!("unknown gitGraph statement: '{line}'"),
                line: line_no,
            });
        }
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(d)
}

fn parse_commit_attrs(s: &str) -> (Option<String>, Option<String>, CommitKind) {
    let mut id = None;
    let mut tag = None;
    let mut kind = CommitKind::Normal;
    let mut s = s.trim();
    while !s.is_empty() {
        if let Some(rest) = s.strip_prefix("id:") {
            let (v, r) = take_value(rest);
            id = Some(v);
            s = r;
        } else if let Some(rest) = s.strip_prefix("tag:") {
            let (v, r) = take_value(rest);
            tag = Some(v);
            s = r;
        } else if let Some(rest) = s.strip_prefix("type:") {
            let (v, r) = take_value(rest);
            kind = match v.as_str() {
                "HIGHLIGHT" => CommitKind::Highlight,
                "REVERSE" => CommitKind::Reverse,
                _ => CommitKind::Normal,
            };
            s = r;
        } else {
            // Skip one token forward.
            match s.find(char::is_whitespace) {
                Some(pos) => s = s[pos..].trim_start(),
                None => break,
            }
        }
    }
    (id, tag, kind)
}

fn take_value(s: &str) -> (String, &str) {
    let s = s.trim_start();
    if let Some(rest) = s.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            return (rest[..end].to_string(), rest[end + 1..].trim_start());
        }
    }
    if let Some(pos) = s.find(char::is_whitespace) {
        (s[..pos].to_string(), s[pos..].trim_start())
    } else {
        (s.to_string(), "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let d = parse("gitGraph\ncommit\ncommit id: \"x\" tag: \"v1\"\nbranch develop\ncheckout develop\ncommit\nmerge develop\n").unwrap();
        assert_eq!(d.events.len(), 6);
        match &d.events[1] {
            GitEvent::Commit { id, tag, .. } => {
                assert_eq!(id.as_deref(), Some("x"));
                assert_eq!(tag.as_deref(), Some("v1"));
            }
            _ => panic!(),
        }
        assert!(matches!(d.events[5], GitEvent::Merge { .. }));
    }
}
