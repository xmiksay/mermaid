//! SVG renderer for Mermaid diagrams.
//!
//! Public entry point: [`render`] (parses the source and dispatches), or
//! [`render_diagram`] if you already have a parsed [`Diagram`].

use thiserror::Error;

use crate::parse::{parse, Diagram, ParseError};

mod builder;
mod class;
mod er;
mod flowchart;
mod gantt;
mod pie;
mod sequence;
mod state;
mod theme;

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("parse error: {0}")]
    Parse(#[from] ParseError),
    #[error("diagram type not yet supported by SVG renderer")]
    Unsupported,
}

pub fn render(input: &str) -> Result<String, RenderError> {
    let d = parse(input)?;
    render_diagram(&d)
}

pub fn render_diagram(d: &Diagram) -> Result<String, RenderError> {
    Ok(match d {
        Diagram::Pie(p) => pie::render(p),
        Diagram::Sequence(s) => sequence::render(s),
        Diagram::Flowchart(f) => flowchart::render(f),
        Diagram::State(s) => state::render(s),
        Diagram::Class(c) => class::render(c),
        Diagram::Er(e) => er::render(e),
        Diagram::Gantt(g) => gantt::render(g),
    })
}
