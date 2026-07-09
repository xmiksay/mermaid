//! Flowchart header, direction, semicolon-split, and subgraph-open helpers.
//!
//! Split out of the parser driver: turning a raw line into `;`-separated
//! statements, reading the `flowchart`/`graph` header and its direction, and
//! opening a `subgraph … [label]` block.

use super::super::ast::{FlowDirection, FlowchartDiagram, Style, Subgraph};
use super::super::ParseError;

/// Split a comment-stripped line into statements at top-level `;`. A semicolon
/// only separates when it is not inside a quoted string, a shape bracket, or an
/// edge-label `|…|` run, so `#59;` entity codes and labels like `["a;b"]` stay
/// intact.
pub(super) fn split_semicolons(line: &str) -> Vec<&str> {
    if !line.contains(';') {
        return vec![line];
    }
    let mut out = Vec::new();
    let mut depth: i32 = 0;
    let mut in_quote = false;
    let mut in_pipe = false;
    let mut start = 0;
    for (i, c) in line.char_indices() {
        match c {
            '"' if !in_pipe => in_quote = !in_quote,
            '|' if !in_quote => in_pipe = !in_pipe,
            '[' | '(' | '{' if !in_quote && !in_pipe => depth += 1,
            ']' | ')' | '}' if !in_quote && !in_pipe => depth -= 1,
            ';' if !in_quote && !in_pipe && depth <= 0 => {
                out.push(&line[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&line[start..]);
    out
}

pub(super) fn parse_header(
    line: &str,
    diag: &mut FlowchartDiagram,
    line_no: usize,
) -> Result<(), ParseError> {
    let rest = if let Some(r) = line.strip_prefix("flowchart-elk") {
        r
    } else if let Some(r) = line.strip_prefix("flowchart") {
        r
    } else if let Some(r) = line.strip_prefix("graph") {
        r
    } else {
        return Err(ParseError::header(
            line_no,
            "expected 'flowchart' or 'graph' header",
        ));
    };
    if let Some(c) = rest.chars().next() {
        if !c.is_whitespace() {
            return Err(ParseError::header(
                line_no,
                "expected 'flowchart' or 'graph' header",
            ));
        }
    }
    diag.direction = parse_direction(rest.trim()).ok_or_else(|| {
        ParseError::unknown(line_no, format!("unknown direction: '{}'", rest.trim()))
    })?;
    Ok(())
}

pub(super) fn parse_direction(s: &str) -> Option<FlowDirection> {
    // Upstream's <dir> lexer also accepts the symbol aliases `>`/`<`/`^`/`v`.
    match s {
        "" | "TD" | "TB" | "v" => Some(FlowDirection::TopDown),
        "BT" | "^" => Some(FlowDirection::BottomTop),
        "LR" | ">" => Some(FlowDirection::LeftRight),
        "RL" | "<" => Some(FlowDirection::RightLeft),
        _ => None,
    }
}

pub(super) fn handle_subgraph_open(
    rest: &str,
    diag: &mut FlowchartDiagram,
    stack: &mut Vec<usize>,
    auto: &mut usize,
) {
    // Forms:
    //   subgraph X
    //   subgraph X [Label]
    //   subgraph "Just a label"   (auto id)
    //   subgraph one two three    (whole text is the id, per upstream
    //                              `subgraph SPACE textNoTags`)
    let rest = rest.trim();
    let (id, label) = if rest.is_empty() {
        *auto += 1;
        (format!("sg{auto}"), String::new())
    } else if rest.starts_with('"') {
        *auto += 1;
        let label = rest.trim_matches('"').to_string();
        (format!("sg{auto}"), label)
    } else if let Some(open) = rest.find('[') {
        // `id [Label]`: id is the text before the bracket, label inside it.
        let id = rest[..open].trim().to_string();
        let label = rest[open + 1..]
            .trim()
            .trim_end_matches(']')
            .trim()
            .trim_matches('"')
            .to_string();
        (id, label)
    } else {
        // A bracket-less title keeps all its words as the id; the renderer
        // shows the id when no label was given, so a multi-word title is not
        // truncated at the first space.
        (rest.to_string(), String::new())
    };

    let new_idx = diag.subgraphs.len();
    diag.subgraphs.push(Subgraph {
        id: id.clone(),
        label,
        direction: None,
        node_ids: Vec::new(),
        child_subgraph_ids: Vec::new(),
        classes: Vec::new(),
        style: Style::new(),
    });
    if let Some(&parent) = stack.last() {
        diag.subgraphs[parent].child_subgraph_ids.push(id);
    }
    stack.push(new_idx);
}
