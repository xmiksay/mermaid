//! Post-render decoration shared by every diagram: accessibility metadata that
//! upstream Mermaid attaches to the root `<svg>`.
//!
//! - `role="graphics-document document"` and `aria-roledescription="<kind>"` are
//!   always added (the kind names the diagram type).
//! - When the source carried `accTitle`/`accDescr`, a `<title>`/`<desc>` element
//!   is injected and linked via `aria-labelledby`/`aria-describedby`.
//!
//! Implemented as string surgery on the finished document so no per-diagram
//! renderer has to thread the metadata through.

use std::fmt::Write as _;

use crate::parse::ast::{Diagram, DiagramMeta};

use super::builder::escape;

/// Stable element ids for the injected accessibility nodes.
const TITLE_ID: &str = "chart-title-mermaid";
const DESC_ID: &str = "chart-desc-mermaid";

/// Add role/aria attributes and, when present, the `<title>`/`<desc>` nodes.
pub fn apply(svg: String, diagram: &Diagram, meta: Option<&DiagramMeta>) -> String {
    let kind = aria_roledescription(diagram);
    let acc_title = meta
        .and_then(|m| m.acc_title.as_deref())
        .filter(|s| !s.is_empty());
    let acc_descr = meta
        .and_then(|m| m.acc_descr.as_deref())
        .filter(|s| !s.is_empty());

    let mut attrs = format!(" role=\"graphics-document document\" aria-roledescription=\"{kind}\"");
    if acc_title.is_some() {
        let _ = write!(attrs, " aria-labelledby=\"{TITLE_ID}\"");
    }
    if acc_descr.is_some() {
        let _ = write!(attrs, " aria-describedby=\"{DESC_ID}\"");
    }

    // Insert attributes right after the `<svg` name token.
    let Some(rest) = svg.strip_prefix("<svg") else {
        return svg;
    };
    let mut out = String::with_capacity(svg.len() + attrs.len() + 128);
    out.push_str("<svg");
    out.push_str(&attrs);

    // Inject `<title>`/`<desc>` immediately after the opening tag's `>`.
    if let Some(gt) = rest.find('>') {
        out.push_str(&rest[..=gt]);
        if let Some(t) = acc_title {
            let _ = write!(out, "<title id=\"{TITLE_ID}\">{}</title>", escape(t));
        }
        if let Some(d) = acc_descr {
            let _ = write!(out, "<desc id=\"{DESC_ID}\">{}</desc>", escape(d));
        }
        out.push_str(&rest[gt + 1..]);
    } else {
        out.push_str(rest);
    }
    out
}

/// The `aria-roledescription` value upstream uses per diagram type.
fn aria_roledescription(d: &Diagram) -> &'static str {
    match d {
        Diagram::Pie(_) => "pie",
        Diagram::Sequence(_) => "sequence",
        Diagram::Flowchart(_) => "flowchart-v2",
        Diagram::State(_) => "stateDiagram",
        Diagram::Class(_) => "classDiagram",
        Diagram::Er(_) => "er",
        Diagram::Gantt(_) => "gantt",
        Diagram::Journey(_) => "journey",
        Diagram::Timeline(_) => "timeline",
        Diagram::Sankey(_) => "sankey",
        Diagram::Quadrant(_) => "quadrantChart",
        Diagram::XyChart(_) => "xychart",
        Diagram::Radar(_) => "radar",
        Diagram::Packet(_) => "packet",
        Diagram::Mindmap(_) => "mindmap",
        Diagram::GitGraph(_) => "gitGraph",
        Diagram::Requirement(_) => "requirement",
        Diagram::C4(_) => "c4",
        Diagram::Block(_) => "block",
        Diagram::Architecture(_) => "architecture",
        Diagram::Kanban(_) => "kanban",
        Diagram::Treemap(_) => "treemap",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::ast::{FlowchartDiagram, PieDiagram};

    fn flow() -> Diagram {
        Diagram::Flowchart(FlowchartDiagram::default())
    }

    #[test]
    fn adds_role_and_roledescription() {
        let out = apply("<svg xmlns=\"x\"></svg>".into(), &flow(), None);
        assert!(out.contains("role=\"graphics-document document\""));
        assert!(out.contains("aria-roledescription=\"flowchart-v2\""));
        assert!(out.starts_with("<svg role="));
    }

    #[test]
    fn roledescription_tracks_kind() {
        let out = apply(
            "<svg></svg>".into(),
            &Diagram::Pie(PieDiagram::default()),
            None,
        );
        assert!(out.contains("aria-roledescription=\"pie\""));
    }

    #[test]
    fn injects_title_and_desc_with_aria_links() {
        let meta = DiagramMeta {
            acc_title: Some("Chart title".into()),
            acc_descr: Some("Chart <desc>".into()),
            ..Default::default()
        };
        let out = apply("<svg xmlns=\"x\"><g/></svg>".into(), &flow(), Some(&meta));
        assert!(out.contains("aria-labelledby=\"chart-title-mermaid\""));
        assert!(out.contains("aria-describedby=\"chart-desc-mermaid\""));
        assert!(out.contains("<title id=\"chart-title-mermaid\">Chart title</title>"));
        // Content is XML-escaped.
        assert!(out.contains("<desc id=\"chart-desc-mermaid\">Chart &lt;desc&gt;</desc>"));
        // Injected before the existing body.
        assert!(out.find("<title").unwrap() < out.find("<g/>").unwrap());
    }

    #[test]
    fn no_meta_means_no_title_desc() {
        let out = apply("<svg></svg>".into(), &flow(), None);
        assert!(!out.contains("<title"));
        assert!(!out.contains("<desc"));
        assert!(!out.contains("aria-labelledby"));
    }
}
