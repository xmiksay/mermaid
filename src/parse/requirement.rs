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
//! <name> - <kind> -> <name>
//! ```
//!
//! Kinds: `contains`, `copies`, `derives`, `satisfies`, `verifies`,
//! `refines`, `traces`.

use super::ast::{
    ReqElement, ReqRelation, ReqRelationKind, Requirement, RequirementDiagram, RequirementKind,
};
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
                return Err(ParseError::Syntax {
                    message: "expected 'requirementDiagram' header".into(),
                    line: line_no,
                });
            }
            header_seen = true;
            continue;
        }

        if let Some(kind) = parse_req_kind(line) {
            let after_kind = &line[kind_token_len(line)..].trim_start();
            let rest = after_kind;
            // <name> {
            let open = rest.find('{').ok_or_else(|| ParseError::Syntax {
                message: format!("expected '{{' in '{line}'"),
                line: line_no,
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
        } else if line.starts_with("element") {
            let rest = line[7..].trim_start();
            let open = rest.find('{').ok_or_else(|| ParseError::Syntax {
                message: format!("expected '{{' in '{line}'"),
                line: line_no,
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
    let token = line.split_whitespace().next()?;
    Some(match token {
        "requirement" => RequirementKind::Requirement,
        "functionalRequirement" => RequirementKind::Functional,
        "interfaceRequirement" => RequirementKind::Interface,
        "performanceRequirement" => RequirementKind::Performance,
        "physicalRequirement" => RequirementKind::Physical,
        "designConstraint" => RequirementKind::DesignConstraint,
        _ => return None,
    })
}

fn kind_token_len(line: &str) -> usize {
    line.split_whitespace().next().map(|t| t.len()).unwrap_or(0)
}

fn consume_req_body<'a, I: Iterator<Item = (usize, &'a str)>>(
    lines: &mut std::iter::Peekable<I>,
    req: &mut Requirement,
) -> Result<(), ParseError> {
    while let Some((_, raw)) = lines.next() {
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
    while let Some((_, raw)) = lines.next() {
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
    // a - kind -> b
    let parts: Vec<&str> = line.split("->").collect();
    if parts.len() != 2 {
        return Err(ParseError::Syntax {
            message: format!("expected 'a - kind -> b': '{line}'"),
            line: line_no,
        });
    }
    let left = parts[0];
    let to = parts[1].trim().to_string();
    let (from, kind_str) = left.rsplit_once('-').ok_or_else(|| ParseError::Syntax {
        message: format!("expected 'a - kind -> b': '{line}'"),
        line: line_no,
    })?;
    let from = from.trim().trim_end_matches('-').trim().to_string();
    let kind = match kind_str.trim() {
        "contains" => ReqRelationKind::Contains,
        "copies" => ReqRelationKind::Copies,
        "derives" => ReqRelationKind::Derives,
        "satisfies" => ReqRelationKind::Satisfies,
        "verifies" => ReqRelationKind::Verifies,
        "refines" => ReqRelationKind::Refines,
        "traces" => ReqRelationKind::Traces,
        k => {
            return Err(ParseError::Syntax {
                message: format!("unknown relation kind '{k}'"),
                line: line_no,
            })
        }
    };
    Ok(ReqRelation { from, to, kind })
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
}
