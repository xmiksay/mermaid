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
                .ok_or_else(|| ParseError::header(line_no, "expected 'gitGraph' header"))?;
            let rest = rest.trim().trim_matches(':').trim();
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
            let (name, order) = parse_branch(rest);
            d.events.push(GitEvent::Branch { name, order });
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
            let (id, tag, parent) = parse_cherry_pick_attrs(rest);
            d.events.push(GitEvent::CherryPick {
                commit_id: id.unwrap_or_default(),
                parent,
                tag,
            });
        } else {
            return Err(ParseError::unknown(
                line_no,
                format!("unknown gitGraph statement: '{line}'"),
            ));
        }
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(d)
}

/// `branch <name> [order: <n>] [tag: <t>]` — the name is the first token; the
/// trailing `order:`/`tag:` attributes are consumed so they can't leak into it.
fn parse_branch(s: &str) -> (String, Option<usize>) {
    let (name, mut rest) = take_value(s);
    let mut order = None;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("order:") {
            let (v, r2) = take_value(r);
            order = v.parse().ok();
            rest = r2;
        } else {
            match rest.find(char::is_whitespace) {
                Some(pos) => rest = rest[pos..].trim_start(),
                None => break,
            }
        }
    }
    (name, order)
}

/// `cherry-pick id:"x" [parent:"y"] [tag:"t"]` — returns `(id, tag, parent)`.
/// Upstream requires `parent` when cherry-picking a merge commit and shows the
/// tag, so neither may be dropped.
fn parse_cherry_pick_attrs(s: &str) -> (Option<String>, Option<String>, Option<String>) {
    let mut id = None;
    let mut tag = None;
    let mut parent = None;
    let mut s = s.trim();
    while !s.is_empty() {
        if let Some(rest) = s.strip_prefix("id:") {
            let (v, r) = take_value(rest);
            id = Some(v);
            s = r;
        } else if let Some(rest) = s.strip_prefix("parent:") {
            let (v, r) = take_value(rest);
            parent = Some(v);
            s = r;
        } else if let Some(rest) = s.strip_prefix("tag:") {
            let (v, r) = take_value(rest);
            tag = Some(v);
            s = r;
        } else {
            match s.find(char::is_whitespace) {
                Some(pos) => s = s[pos..].trim_start(),
                None => break,
            }
        }
    }
    (id, tag, parent)
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

    #[test]
    fn direction_with_trailing_colon() {
        // The documented `gitGraph TB:` / `gitGraph BT:` forms.
        assert_eq!(
            parse("gitGraph TB:\ncommit\n").unwrap().direction,
            GitDirection::TopDown
        );
        assert_eq!(
            parse("gitGraph BT:\ncommit\n").unwrap().direction,
            GitDirection::BottomTop
        );
        // Bare header with a trailing colon still parses as the default LR.
        assert_eq!(
            parse("gitGraph:\ncommit\n").unwrap().direction,
            GitDirection::LeftRight
        );
    }

    #[test]
    fn dispatcher_accepts_trailing_colon_header() {
        // `gitGraph:` must route to the gitGraph parser, not be rejected.
        let d = crate::parse::parse("gitGraph:\ncommit\n").unwrap();
        assert!(matches!(d, crate::parse::Diagram::GitGraph(_)));
    }

    #[test]
    fn cherry_pick_keeps_parent_and_tag() {
        let d = parse("gitGraph\ncommit\ncherry-pick id: \"abc\" parent: \"xyz\" tag: \"v9\"\n")
            .unwrap();
        match &d.events[1] {
            GitEvent::CherryPick {
                commit_id,
                parent,
                tag,
            } => {
                assert_eq!(commit_id, "abc");
                assert_eq!(parent.as_deref(), Some("xyz"));
                assert_eq!(tag.as_deref(), Some("v9"));
            }
            _ => panic!("expected cherry-pick"),
        }
    }

    #[test]
    fn init_config_reaches_diagram() {
        // `%%{init}%%` gitGraph keys must flow through `parse_with_meta` onto
        // the diagram's config (default `mainBranchName` is otherwise `main`).
        let src = "%%{init: {'gitGraph': {'mainBranchName': 'trunk', 'showBranches': false}}}%%\ngitGraph\ncommit\n";
        let d = crate::parse::parse(src).unwrap();
        let crate::parse::Diagram::GitGraph(g) = d else {
            panic!("expected gitGraph");
        };
        assert_eq!(g.config.main_branch_name, "trunk");
        assert!(!g.config.show_branches);
        // Untouched keys keep their upstream defaults.
        assert!(g.config.show_commit_label);
    }

    #[test]
    fn branch_order_attribute() {
        let d = parse("gitGraph\nbranch develop order: 3\n").unwrap();
        match &d.events[0] {
            GitEvent::Branch { name, order } => {
                assert_eq!(name, "develop");
                assert_eq!(*order, Some(3));
            }
            _ => panic!("expected branch"),
        }
    }
}
