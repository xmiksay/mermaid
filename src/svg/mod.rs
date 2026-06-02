//! SVG renderer for Mermaid diagrams.
//!
//! Entry points:
//! - [`render`] / [`render_with`] — accept Mermaid source, dispatch to parse + render
//! - [`render_diagram`] / [`render_diagram_with`] — for an already-parsed [`Diagram`]
//!
//! The non-`_with` variants use [`Theme::default`].

use thiserror::Error;

use crate::parse::{parse, Diagram, ParseError};

pub use self::theme::Theme;

mod architecture;
mod block;
mod builder;
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
mod theme;
mod timeline;
mod treemap;
mod xychart;

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("parse error: {0}")]
    Parse(#[from] ParseError),
    #[error("diagram type not yet supported by SVG renderer")]
    Unsupported,
}

pub fn render(input: &str) -> Result<String, RenderError> {
    render_with(input, &Theme::default())
}

pub fn render_with(input: &str, theme: &Theme) -> Result<String, RenderError> {
    let d = parse(input)?;
    render_diagram_with(&d, theme)
}

pub fn render_diagram(d: &Diagram) -> Result<String, RenderError> {
    render_diagram_with(d, &Theme::default())
}

pub fn render_diagram_with(d: &Diagram, theme: &Theme) -> Result<String, RenderError> {
    Ok(match d {
        Diagram::Pie(p) => pie::render(p, theme),
        Diagram::Sequence(s) => sequence::render(s, theme),
        Diagram::Flowchart(f) => flowchart::render(f, theme),
        Diagram::State(s) => state::render(s, theme),
        Diagram::Class(c) => class::render(c, theme),
        Diagram::Er(e) => er::render(e, theme),
        Diagram::Gantt(g) => gantt::render(g, theme),
        Diagram::Journey(j) => journey::render(j, theme),
        Diagram::Timeline(t) => timeline::render(t, theme),
        Diagram::Sankey(s) => sankey::render(s, theme),
        Diagram::Quadrant(q) => quadrant::render(q, theme),
        Diagram::XyChart(x) => xychart::render(x, theme),
        Diagram::Radar(r) => radar::render(r, theme),
        Diagram::Packet(p) => packet::render(p, theme),
        Diagram::Mindmap(m) => mindmap::render(m, theme),
        Diagram::GitGraph(g) => gitgraph::render(g, theme),
        Diagram::Requirement(r) => requirement::render(r, theme),
        Diagram::C4(c) => c4::render(c, theme),
        Diagram::Block(b) => block::render(b, theme),
        Diagram::Architecture(a) => architecture::render(a, theme),
        Diagram::Kanban(k) => kanban::render(k, theme),
        Diagram::Treemap(t) => treemap::render(t, theme),
    })
}
