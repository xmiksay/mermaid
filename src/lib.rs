//! Render [Mermaid](https://mermaid.js.org/) diagrams to SVG in pure Rust.
//!
//! Supported diagram types: pie, sequence, flowchart, state, class, ER, gantt.
//!
//! ```
//! let svg = mermaid_svg::render("pie\n\"A\" : 1\n\"B\" : 2\n").unwrap();
//! assert!(svg.starts_with("<svg"));
//! ```

mod parse;
mod sugiyama;
mod svg;

pub use parse::ast;
pub use parse::ast::Diagram;
pub use parse::{parse, ParseError};
pub use svg::{render, render_diagram, RenderError};
