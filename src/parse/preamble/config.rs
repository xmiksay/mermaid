//! Derivation of the typed [`DiagramMeta`] fields the renderer honors from the
//! flattened dotted `config` map, plus the scalar value parsers it uses.

use super::super::ast::DiagramMeta;

/// Populate the typed [`DiagramMeta`] fields the renderer honors from the
/// flattened `config` map.
pub(super) fn derive_typed_fields(meta: &mut DiagramMeta) {
    let get = |k: &str| meta.config.get(k).cloned();

    meta.theme = get("theme");
    meta.font_family = get("fontFamily");
    meta.font_size = get("fontSize").as_deref().and_then(parse_font_size);
    meta.use_max_width = get("useMaxWidth")
        .or_else(|| per_diagram_use_max_width(&meta.config))
        .as_deref()
        .and_then(parse_flag);
    meta.look = get("look");
    meta.layout = get("layout");
    meta.security_level = get("securityLevel");
    meta.ticket_base_url = get("kanban.ticketBaseUrl");
    meta.value_format = get("treemap.valueFormat");
    meta.show_values = get("treemap.showValues").as_deref().and_then(parse_flag);
    meta.sankey_link_color = get("sankey.linkColor");
    meta.sankey_node_alignment = get("sankey.nodeAlignment");
    meta.timeline_disable_multicolor = get("timeline.disableMulticolor")
        .as_deref()
        .and_then(parse_flag);
    meta.sankey_show_values = get("sankey.showValues").as_deref().and_then(parse_flag);
    meta.sankey_prefix = get("sankey.prefix");
    meta.sankey_suffix = get("sankey.suffix");
    meta.sankey_width = get("sankey.width").as_deref().and_then(parse_dim);
    meta.sankey_height = get("sankey.height").as_deref().and_then(parse_dim);
    meta.sankey_node_width = get("sankey.nodeWidth").as_deref().and_then(parse_dim);
    meta.sankey_node_padding = get("sankey.nodePadding").as_deref().and_then(parse_dim);
    meta.pie_text_position = get("pie.textPosition").as_deref().and_then(parse_number);
    meta.pie_donut_hole = get("pie.donutHole").as_deref().and_then(parse_number);
    meta.pie_legend_position = get("pie.legendPosition");
    meta.quadrant_chart_width = get("quadrantChart.chartWidth")
        .as_deref()
        .and_then(parse_dim);
    meta.quadrant_chart_height = get("quadrantChart.chartHeight")
        .as_deref()
        .and_then(parse_dim);
    meta.quadrant_point_radius = get("quadrantChart.pointRadius")
        .as_deref()
        .and_then(parse_dim);

    for (k, v) in &meta.config {
        if let Some(name) = k.strip_prefix("themeVariables.") {
            meta.theme_variables.insert(name.to_string(), v.clone());
        }
    }

    let g = &mut meta.git_graph;
    g.main_branch_name = meta.config.get("gitGraph.mainBranchName").cloned();
    g.show_branches = meta
        .config
        .get("gitGraph.showBranches")
        .and_then(|v| parse_flag(v));
    g.show_commit_label = meta
        .config
        .get("gitGraph.showCommitLabel")
        .and_then(|v| parse_flag(v));
    g.rotate_commit_label = meta
        .config
        .get("gitGraph.rotateCommitLabel")
        .and_then(|v| parse_flag(v));
    g.parallel_commits = meta
        .config
        .get("gitGraph.parallelCommits")
        .and_then(|v| parse_flag(v));
    g.main_branch_order = meta
        .config
        .get("gitGraph.mainBranchOrder")
        .and_then(|v| v.trim().parse().ok());
}

/// Find a per-diagram `<diagram>.useMaxWidth` value. Upstream's schema defines
/// `useMaxWidth` only per diagram (`flowchart.useMaxWidth`, `sequence.useMaxWidth`,
/// …), never at the top level; a static render targets a single diagram, so the
/// first such key applies.
fn per_diagram_use_max_width(
    config: &std::collections::BTreeMap<String, String>,
) -> Option<String> {
    config
        .iter()
        .find(|(k, _)| {
            k.ends_with(".useMaxWidth") && !k[..k.len() - ".useMaxWidth".len()].contains('.')
        })
        .map(|(_, v)| v.clone())
}

/// Parse a `fontSize` value that may carry a `px` suffix (`"16px"` / `"16"`).
fn parse_font_size(s: &str) -> Option<f64> {
    let s = s.trim();
    let num = s.strip_suffix("px").unwrap_or(s).trim();
    num.parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && *n > 0.0)
}

/// Parse a positive numeric dimension (`width`/`height`/`nodeWidth`/…).
fn parse_dim(s: &str) -> Option<f64> {
    s.trim()
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && *n > 0.0)
}

/// Parse a finite floating-point config value.
fn parse_number(s: &str) -> Option<f64> {
    s.trim().parse::<f64>().ok().filter(|n| n.is_finite())
}

/// Parse a boolean flag value (`true`/`false`, plus common aliases).
fn parse_flag(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Some(true),
        "false" | "0" | "no" => Some(false),
        _ => None,
    }
}
