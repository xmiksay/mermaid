//! Render [Mermaid](https://mermaid.js.org/) diagrams to SVG in pure Rust.
//!
//! Supported diagram types: pie, sequence, flowchart, state, class, ER,
//! gantt, journey, timeline, sankey, quadrantChart, xychart, radar, packet,
//! mindmap, gitGraph, requirementDiagram, C4 (Context/Container/Component/
//! Dynamic/Deployment), block, architecture, kanban, treemap, zenuml.
//!
//! ```
//! let svg = mermaid_svg::render("pie\n\"A\" : 1\n\"B\" : 2\n").unwrap();
//! assert!(svg.starts_with("<svg"));
//! ```
//!
//! A rendered gallery of every supported diagram type follows below
//! (regenerate with `cargo run --example gen-doc-diagrams`).
#![doc = include_str!("../assets/gallery.md")]

mod parse;
mod sugiyama;
mod svg;

pub use parse::ast;
pub use parse::ast::{Diagram, DiagramMeta};
pub use parse::{parse, parse_with_meta, ParseError};
pub use svg::{render, render_diagram, render_diagram_with, render_with, RenderError, Theme};
