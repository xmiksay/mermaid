//! C4 diagram parser (Context, Container, Component, Dynamic, Deployment).
//!
//! Grammar (call-like):
//!
//! ```text
//! C4Context
//! title <text>
//! Person(alias, "Label", "Optional description")
//! System(alias, "Label", "Optional description")
//! System_Ext(alias, "Label", "Description")
//! Container(alias, "Label", "Tech", "Description")
//! Component(alias, "Label", "Tech", "Description")
//! Node(alias, "Label", "Description") { ... }
//! Rel(from, to, "Label", "Optional Tech")
//! Rel_U/D/L/R(...)
//! Rel_Back(from, to, "Label")  // arrow reversed
//! RelIndex(index, from, to, "Label")  // C4Dynamic step, index first
//! Enterprise_Boundary(alias, "Label") { ... }
//! System_Boundary(alias, "Label") { ... }
//! Container_Boundary(alias, "Label") { ... }
//! Boundary(alias, "Label", "type") { ... }
//! ```

use super::ast::{C4BoundaryKind, C4Diagram, C4Element, C4ElementKind, C4Kind, C4Relation};
use super::{strip_comment, ParseError};

mod calls;
#[cfg(test)]
mod tests;

use calls::{apply_directive, parse_directive, parse_element, parse_rel, split_args, C4Directive};

pub(crate) fn parse(input: &str) -> Result<C4Diagram, ParseError> {
    let mut d = C4Diagram::default();
    let mut header_seen = false;
    let lines: Vec<(usize, String)> = input
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, strip_comment(l).to_string()))
        .filter(|(_, l)| !l.trim().is_empty())
        .collect();

    let mut i = 0;
    while i < lines.len() {
        let (line_no, line) = (lines[i].0, lines[i].1.trim().to_string());
        i += 1;

        if !header_seen {
            d.kind = match line.as_str() {
                "C4Context" => C4Kind::Context,
                "C4Container" => C4Kind::Container,
                "C4Component" => C4Kind::Component,
                "C4Dynamic" => C4Kind::Dynamic,
                "C4Deployment" => C4Kind::Deployment,
                _ => {
                    return Err(ParseError::header(
                        line_no,
                        "expected 'C4Context' or similar header",
                    ))
                }
            };
            header_seen = true;
            continue;
        }

        if let Some(rest) = line.strip_prefix("title") {
            d.title = Some(rest.trim().to_string());
            continue;
        }

        if let Some(dir) = parse_directive(&line) {
            apply_directive(&mut d, dir);
            continue;
        }

        // boundaries open with `{` at end of line.
        if let Some((kind, alias, label, btype, has_open)) = parse_boundary_open(&line) {
            let body = collect_until_close(&lines, &mut i);
            let inner = parse_boundary_body(&body)?;
            let mut element = C4Element {
                kind: C4ElementKind::System, // unused for boundary
                alias,
                label,
                descr: None,
                technology: None,
                sprite: None,
                tags: None,
                link: None,
                external: false,
                boundary_alias: None,
                boundary_label: None,
                boundary_kind: Some(kind),
                boundary_type: btype,
                members: Vec::new(),
            };
            for el in inner.elements {
                element.members.push(el);
            }
            for r in inner.relations {
                d.relations.push(r);
            }
            for dir in inner.directives {
                apply_directive(&mut d, dir);
            }
            d.elements.push(element);
            let _ = has_open;
            continue;
        }

        if let Some(rel) = parse_rel(&line, line_no)? {
            d.relations.push(rel);
            continue;
        }
        if let Some(el) = parse_element(&line, line_no)? {
            d.elements.push(el);
            continue;
        }
        // Unknown line — be tolerant.
    }

    if !header_seen {
        return Err(ParseError::Empty);
    }
    Ok(d)
}

fn parse_boundary_open(
    line: &str,
) -> Option<(C4BoundaryKind, String, String, Option<String>, bool)> {
    let (kind, rest) = if let Some(r) = line.strip_prefix("Enterprise_Boundary") {
        (C4BoundaryKind::Enterprise, r)
    } else if let Some(r) = line.strip_prefix("System_Boundary") {
        (C4BoundaryKind::System, r)
    } else if let Some(r) = line.strip_prefix("Container_Boundary") {
        (C4BoundaryKind::Container, r)
    } else if let Some(r) = line.strip_prefix("Deployment_Node") {
        (C4BoundaryKind::Deployment, r)
    } else if let Some(r) = line.strip_prefix("Node_L") {
        (C4BoundaryKind::Deployment, r)
    } else if let Some(r) = line.strip_prefix("Node_R") {
        (C4BoundaryKind::Deployment, r)
    } else if let Some(r) = line.strip_prefix("Node") {
        (C4BoundaryKind::Deployment, r)
    } else if let Some(r) = line.strip_prefix("Boundary") {
        (C4BoundaryKind::Generic, r)
    } else {
        return None;
    };
    let rest = rest.trim_start();
    let (args_str, has_open) = strip_open(rest);
    let args = split_args(args_str);
    let alias = args.first().cloned().unwrap_or_default();
    let label = args.get(1).cloned().unwrap_or_default();
    let btype = args.get(2).cloned();
    Some((kind, alias, label, btype, has_open))
}

fn strip_open(s: &str) -> (&str, bool) {
    let s = s.trim();
    let s = s.strip_prefix('(').unwrap_or(s);
    if let Some(brace_pos) = s.rfind('{') {
        let before = s[..brace_pos].trim().trim_end_matches(')').trim();
        (before, true)
    } else {
        let trimmed = s.trim_end_matches(')').trim();
        (trimmed, false)
    }
}

fn collect_until_close(lines: &[(usize, String)], i: &mut usize) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut depth = 1i32;
    while *i < lines.len() {
        let (n, raw) = (lines[*i].0, lines[*i].1.clone());
        let t = raw.trim();
        *i += 1;
        if t == "}" {
            depth -= 1;
            if depth == 0 {
                return out;
            }
            out.push((n, raw));
            continue;
        }
        if t.ends_with('{') {
            depth += 1;
        }
        out.push((n, raw));
    }
    out
}

struct BoundaryInner {
    elements: Vec<C4Element>,
    relations: Vec<C4Relation>,
    directives: Vec<C4Directive>,
}

fn parse_boundary_body(body: &[(usize, String)]) -> Result<BoundaryInner, ParseError> {
    let mut out = BoundaryInner {
        elements: Vec::new(),
        relations: Vec::new(),
        directives: Vec::new(),
    };
    let mut i = 0;
    while i < body.len() {
        let (line_no, line) = (body[i].0, body[i].1.trim().to_string());
        i += 1;
        if let Some(dir) = parse_directive(&line) {
            out.directives.push(dir);
            continue;
        }
        if let Some((kind, alias, label, btype, _)) = parse_boundary_open(&line) {
            let mut tmp_i = 0;
            let rest: Vec<(usize, String)> = body[i..].to_vec();
            let nested_body = collect_until_close(&rest, &mut tmp_i);
            i += tmp_i;
            let inner = parse_boundary_body(&nested_body)?;
            let element = C4Element {
                kind: C4ElementKind::System,
                alias,
                label,
                descr: None,
                technology: None,
                sprite: None,
                tags: None,
                link: None,
                external: false,
                boundary_alias: None,
                boundary_label: None,
                boundary_kind: Some(kind),
                boundary_type: btype,
                members: inner.elements,
            };
            for r in inner.relations {
                out.relations.push(r);
            }
            out.directives.extend(inner.directives);
            out.elements.push(element);
            continue;
        }
        if let Some(r) = parse_rel(&line, line_no)? {
            out.relations.push(r);
            continue;
        }
        if let Some(e) = parse_element(&line, line_no)? {
            out.elements.push(e);
            continue;
        }
    }
    Ok(out)
}
