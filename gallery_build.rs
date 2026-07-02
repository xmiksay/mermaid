// Shared by `examples/gen-doc-diagrams.rs` and `tests/integration.rs` via
// `include!`. Not a crate module — it is pasted into each target. Paths are made
// absolute with CARGO_MANIFEST_DIR so they resolve regardless of the includer.

/// (display title, file stem, mermaid source) for every supported diagram type,
/// loaded from the shared `samples/` directory.
const SAMPLES: &[(&str, &str, &str)] = &[
    ("Pie", "pie", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/samples/pie.mmd"))),
    ("Sequence", "sequence", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/samples/sequence.mmd"))),
    ("Flowchart", "flowchart", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/samples/flowchart.mmd"))),
    ("State", "state", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/samples/state.mmd"))),
    ("Class", "class", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/samples/class.mmd"))),
    ("Entity relationship", "er", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/samples/er.mmd"))),
    ("Gantt", "gantt", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/samples/gantt.mmd"))),
    ("User journey", "journey", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/samples/journey.mmd"))),
    ("Timeline", "timeline", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/samples/timeline.mmd"))),
    ("Sankey", "sankey", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/samples/sankey.mmd"))),
    ("Quadrant", "quadrant", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/samples/quadrant.mmd"))),
    ("XY chart", "xychart", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/samples/xychart.mmd"))),
    ("Radar", "radar", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/samples/radar.mmd"))),
    ("Packet", "packet", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/samples/packet.mmd"))),
    ("Mindmap", "mindmap", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/samples/mindmap.mmd"))),
    ("Git graph", "gitgraph", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/samples/gitgraph.mmd"))),
    ("Requirement", "requirement", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/samples/requirement.mmd"))),
    ("C4", "c4", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/samples/c4.mmd"))),
    ("Block", "block", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/samples/block.mmd"))),
    ("Architecture", "architecture", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/samples/architecture.mmd"))),
    ("Kanban", "kanban", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/samples/kanban.mmd"))),
    ("Treemap", "treemap", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/samples/treemap.mmd"))),
    ("ZenUML", "zenuml", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/samples/zenuml.mmd"))),
];

/// The exact Markdown of one `assets/gallery/<stem>.md` section.
/// rustdoc strips inline `<svg>`, so each diagram is a base64 data-URI `<img>`.
fn gallery_section(title: &str, src: &str) -> String {
    let svg = mermaid_svg::render(src).unwrap_or_else(|e| panic!("render {title}: {e}"));
    let data = gallery_base64(svg.as_bytes());
    format!(
        "## {title}\n\n\
         <img alt=\"{title} diagram\" style=\"max-width:100%;height:auto\" \
         src=\"data:image/svg+xml;base64,{data}\" />\n"
    )
}

fn gallery_base64(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[(n >> 18 & 63) as usize] as char);
        out.push(T[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}
