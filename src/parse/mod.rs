//! Mermaid syntax parser. Produces a [`Diagram`] AST from Mermaid source.
//!
//! Supports pie, sequence, flowchart (`flowchart`/`graph`), state
//! (`stateDiagram`/`stateDiagram-v2`), class, ER, and gantt diagrams.
//!
//! Implementation: hand-rolled line-oriented scanners (one per diagram type)
//! rather than a single PEG grammar — Mermaid's syntax is strongly
//! line-based and per-type scanners stay short and easy to extend.

mod architecture;
pub mod ast;
mod block;
mod c4;
mod class;
mod er;
mod flowchart;
mod gantt;
mod gitgraph;
mod journey;
mod kanban;
mod mindmap;
mod packet;
mod pie;
mod quadrant;
mod radar;
mod requirement;
mod sankey;
mod sequence;
mod state;
mod style;
mod timeline;
mod treemap;
mod xychart;
mod zenuml;

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
        "stateDiagram" | "stateDiagram-v2" => state::parse(input).map(Diagram::State),
        "classDiagram" => class::parse(input).map(Diagram::Class),
        "erDiagram" => er::parse(input).map(Diagram::Er),
        "gantt" => gantt::parse(input).map(Diagram::Gantt),
        "journey" => journey::parse(input).map(Diagram::Journey),
        "timeline" => timeline::parse(input).map(Diagram::Timeline),
        "sankey-beta" | "sankey" => sankey::parse(input).map(Diagram::Sankey),
        "quadrantChart" => quadrant::parse(input).map(Diagram::Quadrant),
        "xychart-beta" | "xychart" => xychart::parse(input).map(Diagram::XyChart),
        "radar-beta" | "radar" => radar::parse(input).map(Diagram::Radar),
        "packet-beta" | "packet" => packet::parse(input).map(Diagram::Packet),
        "mindmap" => mindmap::parse(input).map(Diagram::Mindmap),
        "gitGraph" => gitgraph::parse(input).map(Diagram::GitGraph),
        "requirementDiagram" => requirement::parse(input).map(Diagram::Requirement),
        "C4Context" | "C4Container" | "C4Component" | "C4Dynamic" | "C4Deployment" => {
            c4::parse(input).map(Diagram::C4)
        }
        "block-beta" | "block" => block::parse(input).map(Diagram::Block),
        "architecture-beta" | "architecture" => {
            architecture::parse(input).map(Diagram::Architecture)
        }
        "kanban" => kanban::parse(input).map(Diagram::Kanban),
        "treemap-beta" | "treemap" => treemap::parse(input).map(Diagram::Treemap),
        "zenuml" => zenuml::parse(input).map(Diagram::Sequence),
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
