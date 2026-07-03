//! Parsers for the call-like C4 statements: `Person(...)`/`System(...)` etc.
//! elements, `Rel(...)`/`BiRel(...)` relations, and the `Update*` directives,
//! plus the shared quote-aware argument splitter.

use super::super::ast::{
    C4Diagram, C4Element, C4ElementKind, C4ElementStyle, C4LayoutConfig, C4RelDirection,
    C4RelStyle, C4Relation,
};
use super::super::ParseError;

pub(super) enum C4Directive {
    Element(C4ElementStyle),
    Rel(C4RelStyle),
    Layout(C4LayoutConfig),
}

pub(super) fn apply_directive(d: &mut C4Diagram, dir: C4Directive) {
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

pub(super) fn parse_directive(line: &str) -> Option<C4Directive> {
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

pub(super) fn parse_element(line: &str, _line_no: usize) -> Result<Option<C4Element>, ParseError> {
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

pub(super) fn parse_rel(line: &str, _line_no: usize) -> Result<Option<C4Relation>, ParseError> {
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

pub(super) fn split_args(s: &str) -> Vec<String> {
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
