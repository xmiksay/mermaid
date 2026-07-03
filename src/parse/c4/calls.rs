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
    Boundary(C4ElementStyle),
    Rel(C4RelStyle),
    Layout(C4LayoutConfig),
    ShowLegend,
}

pub(super) fn apply_directive(d: &mut C4Diagram, dir: C4Directive) {
    match dir {
        C4Directive::Element(s) => d.element_styles.push(s),
        C4Directive::Boundary(s) => d.boundary_styles.push(s),
        C4Directive::Rel(s) => d.rel_styles.push(s),
        C4Directive::Layout(c) => {
            if c.shape_in_row.is_some() {
                d.layout.shape_in_row = c.shape_in_row;
            }
            if c.boundary_in_row.is_some() {
                d.layout.boundary_in_row = c.boundary_in_row;
            }
        }
        C4Directive::ShowLegend => d.show_legend = true,
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
        "SHOW_LEGEND" => Some(C4Directive::ShowLegend),
        "UpdateElementStyle" => Some(C4Directive::Element(parse_style_directive(&args))),
        "UpdateBoundaryStyle" => Some(C4Directive::Boundary(parse_style_directive(&args))),
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

/// `Update{Element,Boundary}Style(alias, $bgColor=…, $fontColor=…, $borderColor=…)`.
/// The first positional arg is the alias; the rest are `$key=value` overrides.
fn parse_style_directive(args: &[String]) -> C4ElementStyle {
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
    s
}

/// Keyword args (`$descr`/`$techn`/`$sprite`/`$tags`/`$link`) shared by every
/// element and relation macro. Upstream accepts them in any position, so
/// `split_macro_args` separates them from the positional args before slotting.
#[derive(Default)]
struct MacroKeywords {
    descr: Option<String>,
    techn: Option<String>,
    sprite: Option<String>,
    tags: Option<String>,
    link: Option<String>,
}

/// Split a macro's args into its positional args (in order) and the recognized
/// `$key=value` keyword args. A `$key=value` token is pulled out of the
/// positional stream so it can't shift the remaining positional fields.
fn split_macro_args(args: &[String]) -> (Vec<String>, MacroKeywords) {
    let mut positional = Vec::new();
    let mut kw = MacroKeywords::default();
    for a in args {
        match kv(a) {
            Some(("$descr", v)) => kw.descr = Some(v),
            Some(("$techn", v)) => kw.techn = Some(v),
            Some(("$sprite", v)) => kw.sprite = Some(v),
            Some(("$tags", v)) => kw.tags = Some(v),
            Some(("$link", v)) => kw.link = Some(v),
            Some(_) => {} // unknown $keyword — drop, don't corrupt positions
            None => positional.push(a.clone()),
        }
    }
    (positional, kw)
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
    let (args, kw) = split_macro_args(&split_args(args_str));
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
    // Keyword args override the positional slot when both are present.
    let technology = kw.techn.or(technology);
    let descr = kw.descr.or(descr);
    Ok(Some(C4Element {
        kind,
        alias,
        label,
        descr,
        technology,
        sprite: kw.sprite,
        tags: kw.tags,
        link: kw.link,
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
    let (args, kw) = split_macro_args(&split_args(args_str));
    let from = args.first().cloned().unwrap_or_default();
    let to = args.get(1).cloned().unwrap_or_default();
    let label = args.get(2).cloned().unwrap_or_default();
    let technology = kw.techn.or_else(|| args.get(3).cloned());
    Ok(Some(C4Relation {
        from,
        to,
        label,
        technology,
        sprite: kw.sprite,
        tags: kw.tags,
        link: kw.link,
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
