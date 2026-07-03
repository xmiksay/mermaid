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
    Syntax {
        /// The class of syntax failure, so callers can branch without
        /// string-matching `message`.
        kind: SyntaxKind,
        /// A human-readable description of the specific failure.
        message: String,
        /// 1-based source line the error was detected on.
        line: usize,
    },
    #[error("unknown diagram type: {0}")]
    UnknownDiagramType(String),
    #[error("empty input")]
    Empty,
}

/// The class of a [`ParseError::Syntax`] failure. Lets callers distinguish the
/// recurring kinds of parse error (unknown statement vs bad number vs unclosed
/// block) without inspecting the free-form `message`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SyntaxKind {
    /// The diagram's opening header keyword was missing or misspelled. A
    /// defensive per-parser check: [`parse`] pre-validates the header and
    /// reports an unrecognized one as [`ParseError::UnknownDiagramType`], so
    /// this surfaces only when a diagram parser is driven directly.
    MissingHeader,
    /// A line did not match any statement in the diagram's grammar.
    UnknownStatement,
    /// A numeric field could not be parsed (bad integer/float, NaN, or out of
    /// range).
    InvalidNumber,
    /// A bracket, brace, quote, or block delimiter was left unclosed.
    Unclosed,
    /// A recognized statement was otherwise malformed — a missing or empty
    /// required field, or a value that did not fit the expected shape.
    Malformed,
}

impl ParseError {
    /// [`SyntaxKind::MissingHeader`] error at `line`.
    pub(crate) fn header(line: usize, message: impl Into<String>) -> Self {
        Self::syntax(SyntaxKind::MissingHeader, line, message)
    }
    /// [`SyntaxKind::UnknownStatement`] error at `line`.
    pub(crate) fn unknown(line: usize, message: impl Into<String>) -> Self {
        Self::syntax(SyntaxKind::UnknownStatement, line, message)
    }
    /// [`SyntaxKind::InvalidNumber`] error at `line`.
    pub(crate) fn number(line: usize, message: impl Into<String>) -> Self {
        Self::syntax(SyntaxKind::InvalidNumber, line, message)
    }
    /// [`SyntaxKind::Unclosed`] error at `line`.
    pub(crate) fn unclosed(line: usize, message: impl Into<String>) -> Self {
        Self::syntax(SyntaxKind::Unclosed, line, message)
    }
    /// [`SyntaxKind::Malformed`] error at `line`.
    pub(crate) fn malformed(line: usize, message: impl Into<String>) -> Self {
        Self::syntax(SyntaxKind::Malformed, line, message)
    }

    fn syntax(kind: SyntaxKind, line: usize, message: impl Into<String>) -> Self {
        Self::Syntax {
            kind,
            message: message.into(),
            line,
        }
    }
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
    if let Diagram::Treemap(t) = &mut diagram {
        if let Some(fmt) = &meta.value_format {
            t.value_format = Some(fmt.clone());
        }
        if meta.show_values.is_some() {
            t.show_values = meta.show_values;
        }
    }
    if let Diagram::Sankey(s) = &mut diagram {
        if meta.sankey_link_color.is_some() {
            s.link_color = meta.sankey_link_color.clone();
        }
        if meta.sankey_node_alignment.is_some() {
            s.node_alignment = meta.sankey_node_alignment.clone();
        }
        if meta.sankey_show_values.is_some() {
            s.show_values = meta.sankey_show_values;
        }
        if meta.sankey_prefix.is_some() {
            s.prefix = meta.sankey_prefix.clone();
        }
        if meta.sankey_suffix.is_some() {
            s.suffix = meta.sankey_suffix.clone();
        }
        if meta.sankey_width.is_some() {
            s.width = meta.sankey_width;
        }
        if meta.sankey_height.is_some() {
            s.height = meta.sankey_height;
        }
        if meta.sankey_node_width.is_some() {
            s.node_width = meta.sankey_node_width;
        }
        if meta.sankey_node_padding.is_some() {
            s.node_padding = meta.sankey_node_padding;
        }
    }
    if let (Diagram::Timeline(t), Some(true)) = (&mut diagram, meta.timeline_disable_multicolor) {
        t.disable_multicolor = true;
    }
    if let Diagram::GitGraph(g) = &mut diagram {
        apply_git_graph_config(&mut g.config, &meta.git_graph);
    }
    if let Diagram::Packet(p) = &mut diagram {
        apply_packet_config(&mut p.config, &meta.config);
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
    if let Some(v) = meta.main_branch_order {
        cfg.main_branch_order = Some(v);
    }
}

/// Overlay the preamble's `config.packet.*` keys onto the packet layout config,
/// leaving the renderer's defaults where the source set nothing (or set an
/// unparseable / non-positive value).
fn apply_packet_config(
    cfg: &mut ast::PacketConfig,
    map: &std::collections::BTreeMap<String, String>,
) {
    if let Some(v) = map.get("packet.bitsPerRow").and_then(|v| v.parse().ok()) {
        if v >= 1 {
            cfg.bits_per_row = v;
        }
    }
    if let Some(v) = map.get("packet.bitWidth").and_then(|v| v.parse().ok()) {
        if v > 0.0 {
            cfg.bit_width = v;
        }
    }
    if let Some(v) = map.get("packet.rowHeight").and_then(|v| v.parse().ok()) {
        if v > 0.0 {
            cfg.row_height = v;
        }
    }
    if let Some(v) = map.get("packet.showBits") {
        match v.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => cfg.show_bits = true,
            "false" | "0" | "no" => cfg.show_bits = false,
            _ => {}
        }
    }
    if let Some(v) = map.get("packet.paddingX").and_then(|v| v.parse().ok()) {
        if v >= 0.0 {
            cfg.padding_x = v;
        }
    }
    if let Some(v) = map.get("packet.paddingY").and_then(|v| v.parse().ok()) {
        if v >= 0.0 {
            cfg.padding_y = v;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The class of a syntax failure is exposed on the error so callers can
    /// branch without string-matching the human-readable `message`.
    fn kind_of(input: &str) -> SyntaxKind {
        match parse(input).unwrap_err() {
            ParseError::Syntax { kind, .. } => kind,
            e => panic!("expected Syntax error, got {e:?}"),
        }
    }

    #[test]
    fn classifies_missing_header() {
        // The top-level dispatcher pre-validates the header (yielding
        // `UnknownDiagramType`), so a per-parser header re-check only fires
        // when that parser is called directly with the wrong opener.
        let err = pie::parse("notpie\n").unwrap_err();
        assert!(matches!(
            err,
            ParseError::Syntax {
                kind: SyntaxKind::MissingHeader,
                ..
            }
        ));
    }

    #[test]
    fn classifies_unknown_statement() {
        assert_eq!(
            kind_of("stateDiagram-v2\n??? garbage\n"),
            SyntaxKind::UnknownStatement
        );
    }

    #[test]
    fn classifies_invalid_number() {
        assert_eq!(
            kind_of("pie\n\"A\" : not-a-number\n"),
            SyntaxKind::InvalidNumber
        );
    }

    #[test]
    fn classifies_unclosed_delimiter() {
        assert_eq!(
            kind_of("quadrantChart\nPoint: [0.1, 0.2\n"),
            SyntaxKind::Unclosed
        );
    }

    #[test]
    fn classifies_malformed_statement() {
        assert_eq!(kind_of("pie\n : 3\n"), SyntaxKind::Malformed);
    }

    #[test]
    fn packet_config_overlays_from_frontmatter() {
        let src = "---\nconfig:\n  packet:\n    bitsPerRow: 16\n    rowHeight: 24\n    showBits: false\n---\npacket-beta\n0-15: \"Src\"\n";
        let (d, _) = parse_with_meta(src).unwrap();
        let Diagram::Packet(p) = d else {
            panic!("expected packet diagram")
        };
        assert_eq!(p.config.bits_per_row, 16);
        assert_eq!(p.config.row_height, 24.0);
        assert!(!p.config.show_bits);
        // Unset knobs keep their defaults.
        assert_eq!(p.config.bit_width, 16.0);
    }

    #[test]
    fn packet_config_defaults_when_absent() {
        let (d, _) = parse_with_meta("packet-beta\n0-15: \"Src\"\n").unwrap();
        let Diagram::Packet(p) = d else {
            panic!("expected packet diagram")
        };
        assert_eq!(p.config, ast::PacketConfig::default());
    }
}
