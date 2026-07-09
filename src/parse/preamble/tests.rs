use super::*;

/// Read-only view of the flattened config map for the tests below.
fn config_of(src: &str) -> std::collections::BTreeMap<String, String> {
    strip(src).0.config
}

#[test]
fn no_preamble_is_identity() {
    let (m, s) = strip("flowchart TD\nA --> B\n");
    assert_eq!(m, DiagramMeta::default());
    assert_eq!(s, "flowchart TD\nA --> B");
}

#[test]
fn frontmatter_title_and_theme() {
    let src = "---\ntitle: My Flow\nconfig:\n  theme: forest\n---\nflowchart TD\nA --> B\n";
    let (m, s) = strip(src);
    assert_eq!(m.title.as_deref(), Some("My Flow"));
    assert_eq!(m.theme.as_deref(), Some("forest"));
    assert_eq!(s, "flowchart TD\nA --> B");
}

#[test]
fn quoted_frontmatter_title() {
    let (m, _) = strip("---\ntitle: \"Quoted: Title\"\n---\npie\n");
    assert_eq!(m.title.as_deref(), Some("Quoted: Title"));
}

#[test]
fn init_directive_theme() {
    let src = "%%{init: {'theme': 'dark'}}%%\nflowchart TD\nA --> B\n";
    let (m, s) = strip(src);
    assert_eq!(m.theme.as_deref(), Some("dark"));
    assert_eq!(s, "flowchart TD\nA --> B");
}

#[test]
fn frontmatter_theme_variables() {
    let src = "---\nconfig:\n  theme: base\n  themeVariables:\n    primaryColor: \"#ff0000\"\n    lineColor: \"#00ff00\"\n---\nflowchart TD\nA --> B\n";
    let (m, _) = strip(src);
    assert_eq!(m.theme.as_deref(), Some("base"));
    assert_eq!(
        m.theme_variables.get("primaryColor").map(String::as_str),
        Some("#ff0000")
    );
    assert_eq!(
        m.theme_variables.get("lineColor").map(String::as_str),
        Some("#00ff00")
    );
}

#[test]
fn init_directive_nested_theme_variables() {
    let src = "%%{init: {'theme': 'base', 'themeVariables': {'primaryColor': '#abcdef'}}}%%\nflowchart TD\nA --> B\n";
    let (m, _) = strip(src);
    assert_eq!(m.theme.as_deref(), Some("base"));
    assert_eq!(
        m.theme_variables.get("primaryColor").map(String::as_str),
        Some("#abcdef")
    );
}

#[test]
fn font_family_and_use_max_width() {
    let src = "---\nconfig:\n  fontFamily: \"Courier New\"\n  fontSize: 18\n  useMaxWidth: false\n---\npie\n\"A\": 1\n";
    let (m, _) = strip(src);
    assert_eq!(m.font_family.as_deref(), Some("Courier New"));
    assert_eq!(m.font_size, Some(18.0));
    assert_eq!(m.use_max_width, Some(false));
}

#[test]
fn generic_config_map_captures_per_diagram_keys() {
    let cfg = config_of("---\nconfig:\n  flowchart:\n    htmlLabels: false\n    curve: linear\n---\nflowchart TD\nA --> B\n");
    assert_eq!(
        cfg.get("flowchart.htmlLabels").map(String::as_str),
        Some("false")
    );
    assert_eq!(
        cfg.get("flowchart.curve").map(String::as_str),
        Some("linear")
    );
}

#[test]
fn acc_title_and_descr_line() {
    let src = "flowchart TD\naccTitle: The Title\naccDescr: The description\nA --> B\n";
    let (m, s) = strip(src);
    assert_eq!(m.acc_title.as_deref(), Some("The Title"));
    assert_eq!(m.acc_descr.as_deref(), Some("The description"));
    assert_eq!(s, "flowchart TD\nA --> B");
}

#[test]
fn acc_descr_block() {
    let src = "flowchart TD\naccDescr {\n  line one\n  line two\n}\nA --> B\n";
    let (m, s) = strip(src);
    assert_eq!(m.acc_descr.as_deref(), Some("line one\nline two"));
    assert_eq!(s, "flowchart TD\nA --> B");
}

#[test]
fn frontmatter_treemap_value_format() {
    let src = "---\nconfig:\n  treemap:\n    valueFormat: \"$0,0\"\n---\ntreemap-beta\n\"A\": 5\n";
    let (m, s) = strip(src);
    assert_eq!(m.value_format.as_deref(), Some("$0,0"));
    assert_eq!(s, "treemap-beta\n\"A\": 5");
}

#[test]
fn frontmatter_treemap_show_values() {
    let src = "---\nconfig:\n  treemap:\n    showValues: false\n---\ntreemap-beta\n\"A\": 5\n";
    let (m, _) = strip(src);
    assert_eq!(m.show_values, Some(false));
}

#[test]
fn init_directive_pie_config() {
    let src = "%%{init: {\"pie\": {\"textPosition\": 0.2, \"donutHole\": 0.5, \"legendPosition\": \"bottom\"}}}%%\npie\n\"A\": 1\n";
    let (m, _) = strip(src);
    assert_eq!(m.pie_text_position, Some(0.2));
    assert_eq!(m.pie_donut_hole, Some(0.5));
    assert_eq!(m.pie_legend_position.as_deref(), Some("bottom"));
}

#[test]
fn init_directive_git_graph_config() {
    let src = "%%{init: {'gitGraph': {'mainBranchName': 'master', 'showCommitLabel': false, 'parallelCommits': true}}}%%\ngitGraph\ncommit\n";
    let (m, s) = strip(src);
    assert_eq!(m.git_graph.main_branch_name.as_deref(), Some("master"));
    assert_eq!(m.git_graph.show_commit_label, Some(false));
    assert_eq!(m.git_graph.parallel_commits, Some(true));
    assert_eq!(m.git_graph.show_branches, None);
    assert_eq!(s, "gitGraph\ncommit");
}

#[test]
fn frontmatter_git_graph_config() {
    let src = "---\nconfig:\n  gitGraph:\n    mainBranchName: trunk\n    showBranches: false\n---\ngitGraph\ncommit\n";
    let (m, _) = strip(src);
    assert_eq!(m.git_graph.main_branch_name.as_deref(), Some("trunk"));
    assert_eq!(m.git_graph.show_branches, Some(false));
}

#[test]
fn quadrant_chart_config() {
    let src = "---\nconfig:\n  quadrantChart:\n    chartWidth: 300\n    chartHeight: 320\n    pointRadius: 10\n---\nquadrantChart\nA: [0.3, 0.6]\n";
    let (m, _) = strip(src);
    assert_eq!(m.quadrant_chart_width, Some(300.0));
    assert_eq!(m.quadrant_chart_height, Some(320.0));
    assert_eq!(m.quadrant_point_radius, Some(10.0));
}

#[test]
fn kanban_ticket_base_url() {
    let src =
        "---\nconfig:\n  kanban:\n    ticketBaseUrl: 'https://example.com/#TICKET#'\n---\nkanban\n";
    let (m, _) = strip(src);
    assert_eq!(
        m.ticket_base_url.as_deref(),
        Some("https://example.com/#TICKET#")
    );
}

#[test]
fn multiline_init_directive() {
    let src = "%%{init: {\n  \"theme\": \"dark\",\n  \"flowchart\": { \"useMaxWidth\": false }\n}}%%\nflowchart TD\nA --> B\n";
    let (m, s) = strip(src);
    assert_eq!(m.theme.as_deref(), Some("dark"));
    assert_eq!(m.use_max_width, Some(false));
    // The continuation lines must be stripped, not leaked into dispatch.
    assert_eq!(s, "flowchart TD\nA --> B");
}

#[test]
fn per_diagram_use_max_width() {
    let (m, _) =
        strip("%%{init: {\"flowchart\": {\"useMaxWidth\": false}}}%%\nflowchart TD\nA --> B\n");
    assert_eq!(m.use_max_width, Some(false));
    assert_eq!(
        config_of(
            "---\nconfig:\n  sequence:\n    useMaxWidth: false\n---\nsequenceDiagram\nA->>B: hi\n"
        )
        .get("sequence.useMaxWidth")
        .map(String::as_str),
        Some("false")
    );
}

#[test]
fn directive_overrides_frontmatter_and_last_init_wins() {
    // Frontmatter sets the theme; a later init directive overrides it, and
    // among multiple inits the last one wins.
    let src = "---\nconfig:\n  theme: forest\n---\n%%{init: {'theme': 'dark'}}%%\n%%{init: {'theme': 'neutral'}}%%\nflowchart TD\nA --> B\n";
    let (m, _) = strip(src);
    assert_eq!(m.theme.as_deref(), Some("neutral"));
}

#[test]
fn frontmatter_only_when_at_top() {
    // A `---` that is not the first content is not frontmatter.
    let src = "flowchart TD\n---\nA --> B\n";
    let (m, s) = strip(src);
    assert_eq!(m, DiagramMeta::default());
    assert_eq!(s, src.trim_end());
}

#[test]
fn multibyte_lines_do_not_panic() {
    // `accDescr` is 8 bytes; byte 8 of `A["漢字漢字"]` falls inside a
    // CJK char, which used to panic the prefix slice.
    let src = "flowchart TD\n  A[\"漢字漢字\"]\n";
    let (m, s) = strip(src);
    assert_eq!(m, DiagramMeta::default());
    assert!(s.contains("漢字漢字"));

    // Shorter-than-prefix multibyte lines must not panic either.
    let (_, s) = strip("pie\n\"漢\" : 1\n");
    assert!(s.contains('漢'));
}
