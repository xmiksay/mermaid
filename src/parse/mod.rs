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
mod preamble;
mod quadrant;
mod radar;
mod requirement;
mod sankey;
mod sequence;
mod state;
mod style;
mod timeline;
mod token;
mod treemap;
mod xychart;
mod zenuml;

pub use ast::*;

use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum ParseError {
    #[error("parse error at line {line}: {message}")]
    Syntax { message: String, line: usize },
    #[error("unknown diagram type: {0}")]
    UnknownDiagramType(String),
    #[error("empty input")]
    Empty,
}

pub fn parse(input: &str) -> Result<Diagram, ParseError> {
    parse_with_meta(input).map(|(d, _)| d)
}

/// Parse `input`, also returning the cross-cutting [`DiagramMeta`] (title,
/// `accTitle`/`accDescr`, theme) extracted from the source preamble. The
/// diagram body is parsed from the source with the preamble removed.
pub fn parse_with_meta(input: &str) -> Result<(Diagram, DiagramMeta), ParseError> {
    let (meta, cleaned) = preamble::strip(input);
    let mut diagram = dispatch(&cleaned)?;
    if let Some(title) = &meta.title {
        apply_title(&mut diagram, title);
    }
    if let (Diagram::Kanban(k), Some(url)) = (&mut diagram, &meta.ticket_base_url) {
        k.ticket_base_url = Some(url.clone());
    }
    if let (Diagram::Treemap(t), Some(fmt)) = (&mut diagram, &meta.value_format) {
        t.value_format = Some(fmt.clone());
    }
    if let Diagram::GitGraph(g) = &mut diagram {
        apply_git_graph_config(&mut g.config, &meta.git_graph);
    }
    Ok((diagram, meta))
}

/// Copy a frontmatter `title` onto the concrete diagram, but only for diagram
/// kinds that carry a title and only when the body did not set one itself.
fn apply_title(diagram: &mut Diagram, title: &str) {
    let slot: Option<&mut Option<String>> = match diagram {
        Diagram::Pie(d) => Some(&mut d.title),
        Diagram::Sequence(d) => Some(&mut d.title),
        Diagram::Flowchart(d) => Some(&mut d.title),
        Diagram::Gantt(d) => Some(&mut d.title),
        Diagram::Journey(d) => Some(&mut d.title),
        Diagram::Timeline(d) => Some(&mut d.title),
        Diagram::Quadrant(d) => Some(&mut d.title),
        Diagram::XyChart(d) => Some(&mut d.title),
        Diagram::Radar(d) => Some(&mut d.title),
        Diagram::Packet(d) => Some(&mut d.title),
        Diagram::GitGraph(d) => Some(&mut d.title),
        Diagram::C4(d) => Some(&mut d.title),
        Diagram::Treemap(d) => Some(&mut d.title),
        _ => None,
    };
    if let Some(slot) = slot {
        if slot.is_none() {
            *slot = Some(title.to_string());
        }
    }
}

/// Overlay the preamble's `config.gitGraph.*` keys onto the diagram's config,
/// leaving upstream defaults where the source set nothing.
fn apply_git_graph_config(cfg: &mut ast::GitGraphConfig, meta: &ast::GitGraphMeta) {
    if let Some(name) = &meta.main_branch_name {
        cfg.main_branch_name = name.clone();
    }
    if let Some(v) = meta.show_branches {
        cfg.show_branches = v;
    }
    if let Some(v) = meta.show_commit_label {
        cfg.show_commit_label = v;
    }
    if let Some(v) = meta.rotate_commit_label {
        cfg.rotate_commit_label = v;
    }
    if let Some(v) = meta.parallel_commits {
        cfg.parallel_commits = v;
    }
}

fn dispatch(input: &str) -> Result<Diagram, ParseError> {
    let header_line = input
        .lines()
        .map(strip_comment)
        .map(str::trim)
        .find(|l| !l.is_empty())
        .ok_or(ParseError::Empty)?;

    let head_token = header_line
        .split(|c: char| c.is_whitespace())
        .next()
        .unwrap_or("")
        // Upstream's grammar accepts a trailing colon on the header (`gitGraph:`).
        .trim_end_matches(':');
    match head_token {
        "pie" => pie::parse(input).map(Diagram::Pie),
        "sequenceDiagram" => sequence::parse(input).map(Diagram::Sequence),
        "flowchart" | "graph" => flowchart::parse(input).map(Diagram::Flowchart),
        "stateDiagram" | "stateDiagram-v2" => state::parse(input).map(Diagram::State),
        "classDiagram" | "classDiagram-v2" => class::parse(input).map(Diagram::Class),
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
