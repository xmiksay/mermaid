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
//! # Gallery
//!
//! Reference output for every supported diagram type, rendered by this crate
//! from the sources in
//! [`samples/`](https://github.com/xmiksay/mermaid/tree/master/samples)
//! (regenerate with `cargo run --example gen-doc-diagrams`).
#![doc = include_str!("../assets/gallery/pie.md")]
#![doc = include_str!("../assets/gallery/sequence.md")]
#![doc = include_str!("../assets/gallery/flowchart.md")]
#![doc = include_str!("../assets/gallery/state.md")]
#![doc = include_str!("../assets/gallery/class.md")]
#![doc = include_str!("../assets/gallery/er.md")]
#![doc = include_str!("../assets/gallery/gantt.md")]
#![doc = include_str!("../assets/gallery/journey.md")]
#![doc = include_str!("../assets/gallery/timeline.md")]
#![doc = include_str!("../assets/gallery/sankey.md")]
#![doc = include_str!("../assets/gallery/quadrant.md")]
#![doc = include_str!("../assets/gallery/xychart.md")]
#![doc = include_str!("../assets/gallery/radar.md")]
#![doc = include_str!("../assets/gallery/packet.md")]
#![doc = include_str!("../assets/gallery/mindmap.md")]
#![doc = include_str!("../assets/gallery/gitgraph.md")]
#![doc = include_str!("../assets/gallery/requirement.md")]
#![doc = include_str!("../assets/gallery/c4.md")]
#![doc = include_str!("../assets/gallery/block.md")]
#![doc = include_str!("../assets/gallery/architecture.md")]
#![doc = include_str!("../assets/gallery/kanban.md")]
#![doc = include_str!("../assets/gallery/treemap.md")]
#![doc = include_str!("../assets/gallery/zenuml.md")]

mod parse;
mod sugiyama;
mod svg;

pub use parse::ast;
pub use parse::ast::{Diagram, DiagramMeta};
pub use parse::{parse, parse_with_meta, ParseError};
pub use svg::{render, render_diagram, render_diagram_with, render_with, RenderError, Theme};
