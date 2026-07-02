//! Shared rendering of `click`/`link`/`callback` interactions: wraps a shape in
//! an `<a href>` hyperlink or a `<g onclick>` callback group with an optional
//! `<title>` tooltip. Used by the flowchart and class renderers.

use crate::parse::ClickAction;

use super::builder::{escape, SvgBuilder};

/// Open the wrapper element for a clickable shape: an `<a>` for hyperlinks or a
/// `<g class="clickable" onclick=…>` for JS callbacks, plus a `<title>` tooltip.
pub(super) fn open_click(svg: &mut SvgBuilder, action: &ClickAction) {
    match action {
        ClickAction::Href {
            url,
            tooltip,
            target,
        } => {
            let target_attr = match target {
                Some(t) => format!(" target=\"{}\"", escape(t)),
                None => String::new(),
            };
            svg.raw(&format!(
                "<a href=\"{url}\"{target_attr}>",
                url = escape(url)
            ));
            emit_tooltip(svg, tooltip);
        }
        ClickAction::Callback { function, tooltip } => {
            let call = if function.contains('(') {
                function.clone()
            } else {
                format!("{function}()")
            };
            svg.raw(&format!(
                "<g class=\"clickable\" style=\"cursor:pointer\" onclick=\"{}\">",
                escape(&call)
            ));
            emit_tooltip(svg, tooltip);
        }
    }
}

pub(super) fn close_click(svg: &mut SvgBuilder, action: &ClickAction) {
    match action {
        ClickAction::Href { .. } => svg.raw("</a>"),
        ClickAction::Callback { .. } => svg.raw("</g>"),
    }
}

fn emit_tooltip(svg: &mut SvgBuilder, tooltip: &Option<String>) {
    if let Some(t) = tooltip {
        svg.raw(&format!("<title>{}</title>", escape(t)));
    }
}
