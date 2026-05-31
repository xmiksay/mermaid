//! SVG renderer for Mermaid diagrams.
//!
//! Currently supports pie charts and sequence diagrams. The public entry
//! point is [`render`], which parses the Mermaid source via `mermaid-parse`
//! and dispatches to the per-diagram renderer.

use mermaid_parse::{parse, Diagram, ParseError};
use thiserror::Error;

mod flowchart;
mod pie;
mod sequence;
mod svg;
mod theme;

pub use mermaid_parse;

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
    })
}
