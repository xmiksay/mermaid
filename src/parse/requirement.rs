//! requirementDiagram parser.
//!
//! Grammar:
//!
//! ```text
//! requirementDiagram
//! requirement <name> {
//!     id: 1
//!     text: <text>
//!     risk: high
//!     verifymethod: test
//! }
//! element <name> {
//!     type: simulation
//!     docref: doc.md
//! }
//! <src> - <kind> -> <dst>
//! <dst> <- <kind> - <src>
//! direction LR
//! classDef <name> <props>
//! class <name> <class>
//! style <name> <props>
//! ```
//!
//! Kinds: `contains`, `copies`, `derives`, `satisfies`, `verifies`,
//! `refines`, `traces` (matched case-insensitively, like upstream).

use super::ast::{
    FlowDirection, ReqElement, ReqRelation, ReqRelationKind, Requirement, RequirementDiagram,
    RequirementKind,
};
use super::style::parse_style_props;
use super::{strip_comment, ParseError};

pub(crate) fn parse(input: &str) -> Result<RequirementDiagram, ParseError> {
    let mut d = RequirementDiagram::default();
    let mut header_seen = false;
    let mut lines = input.lines().enumerate().peekable();

    while let Some((idx, raw)) = lines.next() {
        let line_no = idx + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }

        if !header_seen {
            if line != "requirementDiagram" {
                return Err(ParseError::header(
                    line_no,
                    "expected 'requirementDiagram' header",
                ));
            }
            header_seen = true;
            continue;
        }

        // v11 statements: consume them instead of falling through to the
        // relation parser (which would hard-error the whole diagram).
        if let Some(rest) = line.strip_prefix("direction ") {
            d.direction = parse_direction(rest.trim()).ok_or_else(|| {
                ParseError::unknown(line_no, format!("unknown direction: '{}'", rest.trim()))
            })?;
            continue;
        }
        if let Some(rest) = line.strip_prefix("classDef ") {
            handle_class_def(rest, &mut d);
            continue;
        }
        if let Some(rest) = line.strip_prefix("class ") {
            handle_class_apply(rest, &mut d);
            continue;
        }
        if let Some(rest) = line.strip_prefix("style ") {
            handle_style(rest, &mut d);
            continue;
        }

        if let Some(kind) = parse_req_kind(line) {
            let after_kind = &line[kind_token_len(line)..].trim_start();
            let rest = after_kind;
            // <name> {
            let open = rest.find('{').ok_or_else(|| {
                ParseError::unclosed(line_no, format!("expected '{{' in '{line}'"))
            })?;
            let name = rest[..open].trim().to_string();
            let mut req = Requirement {
                kind,
                name,
                id: None,
                text: None,
                risk: None,
                verifymethod: None,
            };
            consume_req_body(&mut lines, &mut req)?;
            d.requirements.push(req);
        } else if let Some(rest) = line.strip_prefix("element") {
            let rest = rest.trim_start();
            let open = rest.find('{').ok_or_else(|| {
                ParseError::unclosed(line_no, format!("expected '{{' in '{line}'"))
            })?;
            let name = rest[..open].trim().to_string();
            let mut el = ReqElement {
                name,
                type_: None,
                docref: None,
            };
            consume_element_body(&mut lines, &mut el)?;
            d.elements.push(el);
        } else {
            // relation line: a - kind -> b
            d.relations.push(parse_relation(line, line_no)?);
        }
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(d)
}

fn parse_req_kind(line: &str) -> Option<RequirementKind> {
    // Upstream matches these keywords case-insensitively.
    let token = line.split_whitespace().next()?.to_ascii_lowercase();
    Some(match token.as_str() {
        "requirement" => RequirementKind::Requirement,
        "functionalrequirement" => RequirementKind::Functional,
        "interfacerequirement" => RequirementKind::Interface,
        "performancerequirement" => RequirementKind::Performance,
        "physicalrequirement" => RequirementKind::Physical,
        "designconstraint" => RequirementKind::DesignConstraint,
        _ => return None,
    })
}

fn parse_direction(s: &str) -> Option<FlowDirection> {
    match s {
        "TD" | "TB" => Some(FlowDirection::TopDown),
        "BT" => Some(FlowDirection::BottomTop),
        "LR" => Some(FlowDirection::LeftRight),
        "RL" => Some(FlowDirection::RightLeft),
        _ => None,
    }
}

/// `classDef <name>[,<name2>] <props>` — define one or more style classes.
fn handle_class_def(rest: &str, d: &mut RequirementDiagram) {
    let Some((names, props)) = rest.trim().split_once(char::is_whitespace) else {
        return;
    };
    let style = parse_style_props(props);
    for name in names.split(',') {
        let name = name.trim();
        if !name.is_empty() {
            d.class_defs.insert(name.to_string(), style.clone());
        }
    }
}

/// `class <id1>,<id2> <className>` — apply a class to requirements/elements.
fn handle_class_apply(rest: &str, d: &mut RequirementDiagram) {
    let Some((ids, class_name)) = rest.trim().rsplit_once(char::is_whitespace) else {
        return;
    };
    let class_name = class_name.trim();
    if class_name.is_empty() {
        return;
    }
    for id in ids.split(',') {
        let id = id.trim();
        if !id.is_empty() {
            d.node_classes
                .entry(id.to_string())
                .or_default()
                .push(class_name.to_string());
        }
    }
}

/// `style <id> <props>` — inline style on a single requirement/element.
fn handle_style(rest: &str, d: &mut RequirementDiagram) {
    let Some((id, props)) = rest.trim().split_once(char::is_whitespace) else {
        return;
    };
    let id = id.trim();
    if !id.is_empty() {
        d.node_styles
            .insert(id.to_string(), parse_style_props(props));
    }
}

fn kind_token_len(line: &str) -> usize {
    line.split_whitespace().next().map(|t| t.len()).unwrap_or(0)
}

fn consume_req_body<'a, I: Iterator<Item = (usize, &'a str)>>(
    lines: &mut std::iter::Peekable<I>,
    req: &mut Requirement,
) -> Result<(), ParseError> {
    for (_, raw) in lines.by_ref() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if line == "}" {
            return Ok(());
        }
        let (k, v) = match line.split_once(':') {
            Some((k, v)) => (
                k.trim(),
                v.trim()
                    .trim_end_matches('"')
                    .trim_start_matches('"')
                    .to_string(),
            ),
            None => continue,
        };
        match k {
            "id" => req.id = Some(v),
            "text" => req.text = Some(v),
            "risk" => req.risk = Some(v),
            "verifymethod" => req.verifymethod = Some(v),
            _ => {}
        }
    }
    Ok(())
}

fn consume_element_body<'a, I: Iterator<Item = (usize, &'a str)>>(
    lines: &mut std::iter::Peekable<I>,
    el: &mut ReqElement,
) -> Result<(), ParseError> {
    for (_, raw) in lines.by_ref() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if line == "}" {
            return Ok(());
        }
        let (k, v) = match line.split_once(':') {
            Some((k, v)) => (
                k.trim(),
                v.trim()
                    .trim_end_matches('"')
                    .trim_start_matches('"')
                    .to_string(),
            ),
            None => continue,
        };
        match k {
            "type" => el.type_ = Some(v),
            "docref" | "docRef" => el.docref = Some(v),
            _ => {}
        }
    }
    Ok(())
}

fn parse_relation(line: &str, line_no: usize) -> Result<ReqRelation, ParseError> {
    // Two documented forms, `from`→`to` order preserved for both:
    //   forward: {src} - {kind} -> {dst}
    //   reverse: {dst} <- {kind} - {src}
    if let Some((dst, rest)) = line.split_once("<-") {
        let (kind_str, src) = rest.rsplit_once('-').ok_or_else(|| {
            ParseError::malformed(line_no, format!("expected 'b <- kind - a': '{line}'"))
        })?;
        let kind = parse_relation_kind(kind_str, line_no)?;
        return Ok(ReqRelation {
            from: src.trim().to_string(),
            to: dst.trim().to_string(),
            kind,
        });
    }

    let (left, to) = line.split_once("->").ok_or_else(|| {
        ParseError::malformed(line_no, format!("expected 'a - kind -> b': '{line}'"))
    })?;
    let (from, kind_str) = left.rsplit_once('-').ok_or_else(|| {
        ParseError::malformed(line_no, format!("expected 'a - kind -> b': '{line}'"))
    })?;
    let from = from.trim().trim_end_matches('-').trim().to_string();
    let kind = parse_relation_kind(kind_str, line_no)?;
    Ok(ReqRelation {
        from,
        to: to.trim().to_string(),
        kind,
    })
}

fn parse_relation_kind(kind_str: &str, line_no: usize) -> Result<ReqRelationKind, ParseError> {
    Ok(match kind_str.trim().to_ascii_lowercase().as_str() {
        "contains" => ReqRelationKind::Contains,
        "copies" => ReqRelationKind::Copies,
        "derives" => ReqRelationKind::Derives,
        "satisfies" => ReqRelationKind::Satisfies,
        "verifies" => ReqRelationKind::Verifies,
        "refines" => ReqRelationKind::Refines,
        "traces" => ReqRelationKind::Traces,
        k => {
            return Err(ParseError::unknown(
                line_no,
                format!("unknown relation kind '{k}'"),
            ))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let src = "requirementDiagram\nrequirement test_req {\n    id: 1\n    text: the test\n    risk: high\n    verifymethod: test\n}\nelement test_entity {\n    type: simulation\n}\ntest_entity - satisfies -> test_req\n";
        let d = parse(src).unwrap();
        assert_eq!(d.requirements.len(), 1);
        assert_eq!(d.requirements[0].name, "test_req");
        assert_eq!(d.requirements[0].risk.as_deref(), Some("high"));
        assert_eq!(d.elements.len(), 1);
        assert_eq!(d.relations.len(), 1);
        assert_eq!(d.relations[0].kind, ReqRelationKind::Satisfies);
    }

    #[test]
    fn reverse_relation_swaps_endpoints() {
        // `dst <- kind - src` yields the same edge as `src - kind -> dst`.
        let src = "requirementDiagram\ntest_entity2 <- copies - test_entity\n";
        let d = parse(src).unwrap();
        assert_eq!(d.relations.len(), 1);
        let rel = &d.relations[0];
        assert_eq!(rel.from, "test_entity");
        assert_eq!(rel.to, "test_entity2");
        assert_eq!(rel.kind, ReqRelationKind::Copies);
    }

    #[test]
    fn case_insensitive_kinds() {
        let src =
            "requirementDiagram\nfunctionalrequirement fr {\n    id: 1\n}\nfr - CONTAINS -> fr\n";
        let d = parse(src).unwrap();
        assert_eq!(d.requirements[0].kind, RequirementKind::Functional);
        assert_eq!(d.relations[0].kind, ReqRelationKind::Contains);
    }

    #[test]
    fn v11_statements_are_consumed() {
        let src = "requirementDiagram\ndirection LR\nrequirement r {\n    id: 1\n}\nelement e {\n    type: sim\n}\nclassDef hot fill:#f00,stroke:#900\nclass r hot\nstyle e fill:#0f0\ne - satisfies -> r\n";
        let d = parse(src).unwrap();
        assert_eq!(d.direction, FlowDirection::LeftRight);
        assert_eq!(d.class_defs.get("hot").unwrap()[0].0, "fill");
        assert_eq!(d.node_classes.get("r").unwrap(), &vec!["hot".to_string()]);
        assert_eq!(
            d.node_styles.get("e").unwrap()[0],
            ("fill".into(), "#0f0".into())
        );
        assert_eq!(d.relations.len(), 1);
    }
}
