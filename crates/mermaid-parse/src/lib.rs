//! Mermaid syntax parser.
//!
//! Supports a subset of Mermaid syntax: pie charts and sequence diagrams.
//! More diagram types (flowchart, class, ER, state, gantt) will be added in
//! later phases.
//!
//! Implementation note: the project plan calls for a pest PEG grammar.
//! Mermaid's syntax is strongly line-oriented, so the current implementation
//! uses hand-rolled line-by-line scanners. The AST and public API would be
//! unchanged by a pest-based rewrite if we later want stricter error
//! locations or grammar reuse.

pub mod ast;
mod flowchart;
mod pie;
mod sequence;

pub use ast::*;

use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum ParseError {
    #[error("parse error at line {line}: {message}")]
    Syntax { message: String, line: usize },
    #[error("unknown diagram type: {0}")]
    UnknownDiagramType(String),
    #[error("empty input")]
    Empty,
}

pub fn parse(input: &str) -> Result<Diagram, ParseError> {
    let header_line = input
        .lines()
        .map(strip_comment)
        .map(str::trim)
        .find(|l| !l.is_empty())
        .ok_or(ParseError::Empty)?;

    let head_token = header_line
        .split(|c: char| c.is_whitespace())
        .next()
        .unwrap_or("");
    match head_token {
        "pie" => pie::parse(input).map(Diagram::Pie),
        "sequenceDiagram" => sequence::parse(input).map(Diagram::Sequence),
        "flowchart" | "graph" => flowchart::parse(input).map(Diagram::Flowchart),
        other => Err(ParseError::UnknownDiagramType(other.to_string())),
    }
}

pub(crate) fn strip_comment(line: &str) -> &str {
    if let Some(pos) = line.find("%%") {
        &line[..pos]
    } else {
        line
    }
}
