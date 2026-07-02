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
//! Enterprise_Boundary(alias, "Label") { ... }
//! System_Boundary(alias, "Label") { ... }
//! Container_Boundary(alias, "Label") { ... }
//! Boundary(alias, "Label", "type") { ... }
//! ```

use super::ast::{
    C4BoundaryKind, C4Diagram, C4Element, C4ElementKind, C4ElementStyle, C4Kind, C4LayoutConfig,
    C4RelDirection, C4RelStyle, C4Relation,
};
use super::{strip_comment, ParseError};

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
                    return Err(ParseError::Syntax {
                        message: "expected 'C4Context' or similar header".into(),
                        line: line_no,
                    })
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
                external: false,
                boundary_alias: None,
                boundary_label: None,
                boundary_kind: Some(kind),
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
            let _ = btype;
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
        if let Some((kind, alias, label, _, _)) = parse_boundary_open(&line) {
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
                external: false,
                boundary_alias: None,
                boundary_label: None,
                boundary_kind: Some(kind),
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

enum C4Directive {
    Element(C4ElementStyle),
    Rel(C4RelStyle),
    Layout(C4LayoutConfig),
}

fn apply_directive(d: &mut C4Diagram, dir: C4Directive) {
    match dir {
        C4Directive::Element(s) => d.element_styles.push(s),
        C4Directive::Rel(s) => d.rel_styles.push(s),
        C4Directive::Layout(c) => {
            if c.shape_in_row.is_some() {
                d.layout.shape_in_row = c.shape_in_row;
            }
            if c.boundary_in_row.is_some() {
                d.layout.boundary_in_row = c.boundary_in_row;
            }
        }
    }
}

fn parse_directive(line: &str) -> Option<C4Directive> {
    let (token, rest) = match line.find('(') {
        Some(p) => (&line[..p], &line[p..]),
        None => return None,
    };
    let args_str = rest.trim_start_matches('(').trim_end_matches(')');
    let args = split_args(args_str);
    match token {
        "UpdateElementStyle" => {
            let mut s = C4ElementStyle {
                alias: args.first().cloned().unwrap_or_default(),
                ..Default::default()
            };
            for a in args.iter().skip(1) {
                if let Some((k, v)) = kv(a) {
                    match k {
                        "$bgColor" => s.bg_color = Some(v),
                        "$fontColor" => s.font_color = Some(v),
                        "$borderColor" => s.border_color = Some(v),
                        _ => {}
                    }
                }
            }
            Some(C4Directive::Element(s))
        }
        "UpdateRelStyle" => {
            let mut s = C4RelStyle {
                from: args.first().cloned().unwrap_or_default(),
                to: args.get(1).cloned().unwrap_or_default(),
                ..Default::default()
            };
            for a in args.iter().skip(2) {
                if let Some((k, v)) = kv(a) {
                    match k {
                        "$textColor" => s.text_color = Some(v),
                        "$lineColor" => s.line_color = Some(v),
                        "$offsetX" => s.offset_x = v.parse().ok(),
                        "$offsetY" => s.offset_y = v.parse().ok(),
                        _ => {}
                    }
                }
            }
            Some(C4Directive::Rel(s))
        }
        "UpdateLayoutConfig" => {
            let mut c = C4LayoutConfig::default();
            for a in &args {
                if let Some((k, v)) = kv(a) {
                    match k {
                        "$c4ShapeInRow" => c.shape_in_row = v.parse().ok(),
                        "$c4BoundaryInRow" => c.boundary_in_row = v.parse().ok(),
                        _ => {}
                    }
                }
            }
            Some(C4Directive::Layout(c))
        }
        _ => None,
    }
}

/// Split a `$key="value"` directive argument. Quotes are already removed by
/// `split_args`, so the value arrives bare.
fn kv(arg: &str) -> Option<(&str, String)> {
    let arg = arg.trim();
    if !arg.starts_with('$') {
        return None;
    }
    let (k, v) = arg.split_once('=')?;
    Some((k.trim(), v.trim().trim_matches('"').to_string()))
}

fn parse_element(line: &str, _line_no: usize) -> Result<Option<C4Element>, ParseError> {
    let (token, rest) = match line.find('(') {
        Some(p) => (&line[..p], &line[p..]),
        None => return Ok(None),
    };
    let (external, kind) = match token {
        "Person" => (false, C4ElementKind::Person),
        "Person_Ext" => (true, C4ElementKind::Person),
        "System" => (false, C4ElementKind::System),
        "System_Ext" => (true, C4ElementKind::System),
        "SystemDb" => (false, C4ElementKind::SystemDb),
        "SystemDb_Ext" => (true, C4ElementKind::SystemDb),
        "SystemQueue" => (false, C4ElementKind::SystemQueue),
        "SystemQueue_Ext" => (true, C4ElementKind::SystemQueue),
        "Container" => (false, C4ElementKind::Container),
        "ContainerDb" => (false, C4ElementKind::ContainerDb),
        "ContainerDb_Ext" => (true, C4ElementKind::ContainerDb),
        "ContainerQueue" => (false, C4ElementKind::ContainerQueue),
        "ContainerQueue_Ext" => (true, C4ElementKind::ContainerQueue),
        "Container_Ext" => (true, C4ElementKind::Container),
        "Component" => (false, C4ElementKind::Component),
        "ComponentDb" => (false, C4ElementKind::ComponentDb),
        "ComponentDb_Ext" => (true, C4ElementKind::ComponentDb),
        "ComponentQueue" => (false, C4ElementKind::ComponentQueue),
        "ComponentQueue_Ext" => (true, C4ElementKind::ComponentQueue),
        "Component_Ext" => (true, C4ElementKind::Component),
        "Node" => (false, C4ElementKind::Node),
        _ => return Ok(None),
    };
    let args_str = rest.trim_start_matches('(').trim_end_matches(')');
    let args = split_args(args_str);
    let alias = args.first().cloned().unwrap_or_default();
    let label = args.get(1).cloned().unwrap_or_default();
    let (technology, descr) = match kind {
        C4ElementKind::Container
        | C4ElementKind::ContainerDb
        | C4ElementKind::ContainerQueue
        | C4ElementKind::Component
        | C4ElementKind::ComponentDb
        | C4ElementKind::ComponentQueue => (args.get(2).cloned(), args.get(3).cloned()),
        _ => (None, args.get(2).cloned()),
    };
    Ok(Some(C4Element {
        kind,
        alias,
        label,
        descr,
        technology,
        external,
        boundary_alias: None,
        boundary_label: None,
        boundary_kind: None,
        members: Vec::new(),
    }))
}

fn parse_rel(line: &str, _line_no: usize) -> Result<Option<C4Relation>, ParseError> {
    let (token, rest) = match line.find('(') {
        Some(p) => (&line[..p], &line[p..]),
        None => return Ok(None),
    };
    let (direction, bidirectional) = match token {
        "Rel" => (C4RelDirection::Default, false),
        "BiRel" => (C4RelDirection::Default, true),
        "Rel_U" | "Rel_Up" => (C4RelDirection::Up, false),
        "Rel_D" | "Rel_Down" => (C4RelDirection::Down, false),
        "Rel_L" | "Rel_Left" => (C4RelDirection::Left, false),
        "Rel_R" | "Rel_Right" => (C4RelDirection::Right, false),
        _ => return Ok(None),
    };
    let args_str = rest.trim_start_matches('(').trim_end_matches(')');
    let args = split_args(args_str);
    let from = args.first().cloned().unwrap_or_default();
    let to = args.get(1).cloned().unwrap_or_default();
    let label = args.get(2).cloned().unwrap_or_default();
    let technology = args.get(3).cloned();
    Ok(Some(C4Relation {
        from,
        to,
        label,
        technology,
        direction,
        bidirectional,
    }))
}

fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_q = false;
    let mut depth = 0i32;
    for c in s.chars() {
        match c {
            '"' => in_q = !in_q,
            '(' if !in_q => {
                depth += 1;
                cur.push(c);
            }
            ')' if !in_q => {
                depth -= 1;
                cur.push(c);
            }
            ',' if !in_q && depth == 0 => {
                out.push(cur.trim().trim_matches('"').to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    let last = cur.trim().trim_matches('"').to_string();
    if !last.is_empty() || !out.is_empty() {
        out.push(last);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_context() {
        let src = "C4Context\ntitle My system\nPerson(c, \"Customer\", \"A customer\")\nSystem(s, \"Banking\", \"App\")\nRel(c, s, \"Uses\")\n";
        let d = parse(src).unwrap();
        assert_eq!(d.kind, C4Kind::Context);
        assert_eq!(d.elements.len(), 2);
        assert_eq!(d.relations.len(), 1);
        assert_eq!(d.elements[0].label, "Customer");
        assert_eq!(d.relations[0].label, "Uses");
    }

    #[test]
    fn parses_ext_db_and_queue_variants() {
        let src = "C4Container\n\
            SystemQueue_Ext(sq, \"Bus\")\n\
            ContainerDb_Ext(cd, \"DB\", \"Postgres\")\n\
            ContainerQueue_Ext(cq, \"Q\", \"Kafka\")\n\
            ComponentDb_Ext(pd, \"Store\", \"Redis\")\n\
            ComponentQueue_Ext(pq, \"MQ\", \"NATS\")\n";
        let d = parse(src).unwrap();
        assert_eq!(d.elements.len(), 5);
        assert!(d.elements.iter().all(|e| e.external));
        let kinds: Vec<_> = d.elements.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            vec![
                C4ElementKind::SystemQueue,
                C4ElementKind::ContainerDb,
                C4ElementKind::ContainerQueue,
                C4ElementKind::ComponentDb,
                C4ElementKind::ComponentQueue,
            ]
        );
    }

    #[test]
    fn ext_element_keeps_its_relations() {
        let src = "C4Context\n\
            System(s, \"S\")\n\
            SystemQueue_Ext(q, \"Bus\")\n\
            Rel(s, q, \"publishes\")\n";
        let d = parse(src).unwrap();
        assert_eq!(d.elements.len(), 2);
        assert_eq!(d.relations.len(), 1);
    }

    #[test]
    fn parses_update_directives() {
        let src = "C4Context\n\
            Person(c, \"Customer\")\n\
            System(s, \"Banking\")\n\
            Rel(c, s, \"Uses\")\n\
            UpdateElementStyle(c, $bgColor=\"red\", $fontColor=\"white\", $borderColor=\"black\")\n\
            UpdateRelStyle(c, s, $textColor=\"blue\", $lineColor=\"green\", $offsetX=\"10\", $offsetY=\"-5\")\n\
            UpdateLayoutConfig($c4ShapeInRow=\"3\", $c4BoundaryInRow=\"2\")\n";
        let d = parse(src).unwrap();
        assert_eq!(d.element_styles.len(), 1);
        let es = &d.element_styles[0];
        assert_eq!(es.alias, "c");
        assert_eq!(es.bg_color.as_deref(), Some("red"));
        assert_eq!(es.font_color.as_deref(), Some("white"));
        assert_eq!(es.border_color.as_deref(), Some("black"));

        assert_eq!(d.rel_styles.len(), 1);
        let rs = &d.rel_styles[0];
        assert_eq!((rs.from.as_str(), rs.to.as_str()), ("c", "s"));
        assert_eq!(rs.text_color.as_deref(), Some("blue"));
        assert_eq!(rs.line_color.as_deref(), Some("green"));
        assert_eq!(rs.offset_x, Some(10.0));
        assert_eq!(rs.offset_y, Some(-5.0));

        assert_eq!(d.layout.shape_in_row, Some(3));
        assert_eq!(d.layout.boundary_in_row, Some(2));
    }

    #[test]
    fn parses_boundary() {
        let src = "C4Context\nSystem_Boundary(b, \"Boundary\") {\n  System(s, \"S\", \"d\")\n}\n";
        let d = parse(src).unwrap();
        assert_eq!(d.elements.len(), 1);
        assert!(matches!(
            d.elements[0].boundary_kind,
            Some(C4BoundaryKind::System)
        ));
        assert_eq!(d.elements[0].members.len(), 1);
    }

    #[test]
    fn parses_deployment_node_aliases() {
        let src = "C4Deployment\n\
            Node(dc, \"Datacenter\", \"region\") {\n\
              Node_L(l, \"Left\") {\n\
                Container(a, \"App\")\n\
              }\n\
              Node_R(r, \"Right\") {\n\
                Container(b, \"Db\")\n\
              }\n\
            }\n";
        let d = parse(src).unwrap();
        assert_eq!(d.elements.len(), 1);
        let dc = &d.elements[0];
        assert!(matches!(dc.boundary_kind, Some(C4BoundaryKind::Deployment)));
        assert_eq!(dc.alias, "dc");
        assert_eq!(dc.label, "Datacenter");
        assert_eq!(dc.members.len(), 2);
        assert!(dc
            .members
            .iter()
            .all(|m| matches!(m.boundary_kind, Some(C4BoundaryKind::Deployment))));
        assert_eq!(dc.members[0].alias, "l");
        assert_eq!(dc.members[0].members.len(), 1);
        assert_eq!(dc.members[1].alias, "r");
        assert_eq!(dc.members[1].members.len(), 1);
    }
}
