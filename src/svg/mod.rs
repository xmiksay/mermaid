//! SVG renderer for Mermaid diagrams.
//!
//! Entry points:
//! - [`render`] / [`render_with`] — accept Mermaid source, dispatch to parse + render
//! - [`render_diagram`] / [`render_diagram_with`] — for an already-parsed [`Diagram`]
//!
//! The non-`_with` variants use [`Theme::default`].

use thiserror::Error;

use crate::parse::{parse_with_meta, Diagram, ParseError};

pub use self::theme::Theme;

mod architecture;
mod block;
mod builder;
mod c4;
mod class;
mod decorate;
mod er;
mod flowchart;
mod gantt;
mod gantt_date;
mod geometry;
mod gitgraph;
mod interact;
mod journey;
mod kanban;
mod label;
mod markup;
mod metrics;
mod mindmap;
mod packet;
mod pie;
mod quadrant;
mod radar;
mod requirement;
mod sankey;
mod sankey_layout;
mod sequence;
mod state;
mod style;
mod theme;
mod timeline;
mod treemap;
mod xychart;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RenderError {
    #[error("parse error: {0}")]
    Parse(#[from] ParseError),
}

pub fn render(input: &str) -> Result<String, RenderError> {
    render_with(input, &Theme::default())
}

pub fn render_with(input: &str, theme: &Theme) -> Result<String, RenderError> {
    let (d, meta) = parse_with_meta(input)?;
    let effective = theme_from_meta(theme, &meta);
    let body = render_body(&d, &effective);
    Ok(decorate::apply(body, &d, Some(&meta)))
}

/// Build the effective theme for a render from the caller's theme and the
/// source-preamble config. A `theme` named in the preamble (frontmatter
/// `config.theme` or an `%%{init}%%` directive) takes precedence over the
/// caller's; `themeVariables`, `fontFamily`/`fontSize` and `useMaxWidth` then
/// layer on top of whichever base was chosen.
fn theme_from_meta(caller: &Theme, meta: &crate::parse::DiagramMeta) -> Theme {
    let mut effective = meta
        .theme
        .as_deref()
        .and_then(Theme::by_name)
        .unwrap_or_else(|| caller.clone());
    if !meta.theme_variables.is_empty() {
        effective.apply_theme_variables(&meta.theme_variables);
    }
    if let Some(family) = &meta.font_family {
        effective.font_family = family.clone().into();
    }
    if let Some(size) = meta.font_size {
        effective.font_size = size;
    }
    if let Some(use_max_width) = meta.use_max_width {
        effective.responsive = use_max_width;
    }
    effective
}

pub fn render_diagram(d: &Diagram) -> Result<String, RenderError> {
    render_diagram_with(d, &Theme::default())
}

pub fn render_diagram_with(d: &Diagram, theme: &Theme) -> Result<String, RenderError> {
    let body = render_body(d, theme);
    Ok(decorate::apply(body, d, None))
}

fn render_body(d: &Diagram, theme: &Theme) -> String {
    match d {
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
    }
}
