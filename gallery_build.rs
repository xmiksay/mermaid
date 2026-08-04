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
    (
        "Nested subgraphs",
        "flowchart/02-nested-subgraphs",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/flowchart/02-nested-subgraphs.mmd"
        )),
    ),
    (
        "Basic shapes and arrows",
        "flowchart/03-basic-shapes-and-arrows",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/flowchart/03-basic-shapes-and-arrows.mmd"
        )),
    ),
    (
        "Decision tree",
        "flowchart/04-decision-tree",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/flowchart/04-decision-tree.mmd"
        )),
    ),
    (
        "Multi-source/target fanout",
        "flowchart/05-multi-source-target-fanout",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/flowchart/05-multi-source-target-fanout.mmd"
        )),
    ),
    (
        "V11 shape gallery",
        "flowchart/06-v11-shape-gallery",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/flowchart/06-v11-shape-gallery.mmd"
        )),
    ),
    (
        "Asymmetric and flag shapes",
        "flowchart/07-asymmetric-and-flag-shapes",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/flowchart/07-asymmetric-and-flag-shapes.mmd"
        )),
    ),
    (
        "Edge ids, animate and curve",
        "flowchart/08-edge-ids-animate-curve",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/flowchart/08-edge-ids-animate-curve.mmd"
        )),
    ),
    (
        "ClassDef and inline style",
        "flowchart/09-classdef-and-inline-style",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/flowchart/09-classdef-and-inline-style.mmd"
        )),
    ),
    (
        "Click interactions",
        "flowchart/10-click-interactions",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/flowchart/10-click-interactions.mmd"
        )),
    ),
    (
        "Direction aliases and local direction",
        "flowchart/11-direction-aliases-and-local-direction",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/flowchart/11-direction-aliases-and-local-direction.mmd"
        )),
    ),
    (
        "Long labels and quoted text",
        "flowchart/12-long-labels-and-quoted-text",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/flowchart/12-long-labels-and-quoted-text.mmd"
        )),
    ),
    (
        "Semicolons and inline edge labels",
        "flowchart/13-semicolons-and-inline-edge-labels",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/flowchart/13-semicolons-and-inline-edge-labels.mmd"
        )),
    ),
    (
        "Basic messages",
        "sequence/02-basic-messages",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/sequence/02-basic-messages.mmd"
        )),
    ),
    (
        "Activation stack",
        "sequence/03-activation-stack",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/sequence/03-activation-stack.mmd"
        )),
    ),
    (
        "Alt, else and opt",
        "sequence/04-alt-else-opt",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/sequence/04-alt-else-opt.mmd"
        )),
    ),
    (
        "Par and critical",
        "sequence/05-par-and-critical",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/sequence/05-par-and-critical.mmd"
        )),
    ),
    (
        "Nested blocks combined",
        "sequence/06-nested-blocks-combined",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/sequence/06-nested-blocks-combined.mmd"
        )),
    ),
    (
        "Notes word wrap",
        "sequence/07-notes-word-wrap",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/sequence/07-notes-word-wrap.mmd"
        )),
    ),
    (
        "Autonumber variants",
        "sequence/08-autonumber-variants",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/sequence/08-autonumber-variants.mmd"
        )),
    ),
    (
        "Create/destroy lifecycle",
        "sequence/09-create-destroy-lifecycle",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/sequence/09-create-destroy-lifecycle.mmd"
        )),
    ),
    (
        "Actor box grouping",
        "sequence/10-actor-box-grouping",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/sequence/10-actor-box-grouping.mmd"
        )),
    ),
    (
        "Half arrows and bidirectional",
        "sequence/11-half-arrows-and-bidirectional",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/sequence/11-half-arrows-and-bidirectional.mmd"
        )),
    ),
    (
        "Rect background bands",
        "sequence/12-rect-background-bands",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/sequence/12-rect-background-bands.mmd"
        )),
    ),
    (
        "Participant type metadata",
        "sequence/13-participant-type-metadata",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/sequence/13-participant-type-metadata.mmd"
        )),
    ),
    (
        "Basic call chain",
        "zenuml/02-basic-call-chain",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/zenuml/02-basic-call-chain.mmd"
        )),
    ),
    (
        "Stereotype annotators",
        "zenuml/03-stereotype-annotators",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/zenuml/03-stereotype-annotators.mmd"
        )),
    ),
    (
        "Create new participant",
        "zenuml/04-create-new-participant",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/zenuml/04-create-new-participant.mmd"
        )),
    ),
    (
        "Typed assignment returns",
        "zenuml/05-assignment-returns-typed",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/zenuml/05-assignment-returns-typed.mmd"
        )),
    ),
    (
        "Control structures gallery",
        "zenuml/06-control-structures-gallery",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/zenuml/06-control-structures-gallery.mmd"
        )),
    ),
    (
        "Directed return and starter",
        "zenuml/07-directed-return-and-starter",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/zenuml/07-directed-return-and-starter.mmd"
        )),
    ),
    (
        "Basic transitions",
        "state/02-basic-transitions",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/state/02-basic-transitions.mmd"
        )),
    ),
    (
        "Composite nested state",
        "state/03-composite-nested-state",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/state/03-composite-nested-state.mmd"
        )),
    ),
    (
        "Parallel regions",
        "state/04-parallel-regions",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/state/04-parallel-regions.mmd"
        )),
    ),
    (
        "Choice, fork and join",
        "state/05-choice-fork-join",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/state/05-choice-fork-join.mmd"
        )),
    ),
    (
        "Shallow and deep history",
        "state/06-history-shallow-and-deep",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/state/06-history-shallow-and-deep.mmd"
        )),
    ),
    (
        "Quoted state aliasing",
        "state/07-quoted-state-aliasing",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/state/07-quoted-state-aliasing.mmd"
        )),
    ),
    (
        "Notes and bare id",
        "state/08-notes-and-bare-id",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/state/08-notes-and-bare-id.mmd"
        )),
    ),
    (
        "Click and classDef styling",
        "state/09-click-and-classdef-styling",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/state/09-click-and-classdef-styling.mmd"
        )),
    ),
    (
        "Direction transpose",
        "state/10-direction-transpose",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/state/10-direction-transpose.mmd"
        )),
    ),
    (
        "Basic inheritance",
        "class/02-basic-inheritance",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/class/02-basic-inheritance.mmd"
        )),
    ),
    (
        "Nested namespaces",
        "class/03-namespaces-nested",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/class/03-namespaces-nested.mmd"
        )),
    ),
    (
        "Relation kinds gallery",
        "class/04-relation-kinds-gallery",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/class/04-relation-kinds-gallery.mmd"
        )),
    ),
    (
        "Two-way relations",
        "class/05-two-way-relations",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/class/05-two-way-relations.mmd"
        )),
    ),
    (
        "Multiplicities and labels",
        "class/06-multiplicities-and-labels",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/class/06-multiplicities-and-labels.mmd"
        )),
    ),
    (
        "Generics and lollipop interfaces",
        "class/07-generics-and-lollipop-interfaces",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/class/07-generics-and-lollipop-interfaces.mmd"
        )),
    ),
    (
        "Stereotypes and notes",
        "class/08-stereotypes-and-notes",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/class/08-stereotypes-and-notes.mmd"
        )),
    ),
    (
        "One-line body and classDef",
        "class/09-one-line-body-and-classdef",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/class/09-one-line-body-and-classdef.mmd"
        )),
    ),
    (
        "Click, link, callback and direction",
        "class/10-click-link-callback-and-direction",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/class/10-click-link-callback-and-direction.mmd"
        )),
    ),
    (
        "Basic context diagram",
        "c4/02-context-diagram-basic",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/c4/02-context-diagram-basic.mmd"
        )),
    ),
    (
        "Container diagram",
        "c4/03-container-diagram",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/c4/03-container-diagram.mmd"
        )),
    ),
    (
        "Component diagram",
        "c4/04-component-diagram",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/c4/04-component-diagram.mmd"
        )),
    ),
    (
        "Dynamic numbered steps",
        "c4/05-dynamic-numbered-steps",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/c4/05-dynamic-numbered-steps.mmd"
        )),
    ),
    (
        "Deployment nested nodes",
        "c4/06-deployment-nested-nodes",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/c4/06-deployment-nested-nodes.mmd"
        )),
    ),
    (
        "Boundaries and element styling",
        "c4/07-boundaries-and-element-styling",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/c4/07-boundaries-and-element-styling.mmd"
        )),
    ),
    (
        "Rel direction and rel styling",
        "c4/08-rel-direction-and-rel-styling",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/c4/08-rel-direction-and-rel-styling.mmd"
        )),
    ),
    (
        "Layout row wrap config",
        "c4/09-layout-row-wrap-config",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/c4/09-layout-row-wrap-config.mmd"
        )),
    ),
    (
        "Keyword args and tags",
        "c4/10-keyword-args-and-tags",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/c4/10-keyword-args-and-tags.mmd"
        )),
    ),
    (
        "Identifying vs non-identifying",
        "er/02-identifying-vs-non-identifying",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/er/02-identifying-vs-non-identifying.mmd"
        )),
    ),
    (
        "Attribute keys and comments",
        "er/03-attribute-keys-and-comments",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/er/03-attribute-keys-and-comments.mmd"
        )),
    ),
    (
        "Verbal cardinality phrases",
        "er/04-verbal-cardinality-phrases",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/er/04-verbal-cardinality-phrases.mmd"
        )),
    ),
    (
        "Numeric and shorthand cardinalities",
        "er/05-numeric-and-shorthand-cardinalities",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/er/05-numeric-and-shorthand-cardinalities.mmd"
        )),
    ),
    (
        "Quoted entity names and aliasing",
        "er/06-quoted-entity-names-and-aliasing",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/er/06-quoted-entity-names-and-aliasing.mmd"
        )),
    ),
    (
        "ClassDef and style",
        "er/07-classdef-and-style",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/er/07-classdef-and-style.mmd"
        )),
    ),
    (
        "Direction transpose",
        "er/08-direction-transpose",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/er/08-direction-transpose.mmd"
        )),
    ),
    (
        "Crow's-foot glyph gallery",
        "er/09-crowfoot-glyph-gallery",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/er/09-crowfoot-glyph-gallery.mmd"
        )),
    ),
    (
        "Basic schedule",
        "gantt/02-basic-schedule",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/gantt/02-basic-schedule.mmd"
        )),
    ),
    (
        "Dependencies: after and until",
        "gantt/03-dependencies-after-and-until",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/gantt/03-dependencies-after-and-until.mmd"
        )),
    ),
    (
        "Status and crit combinations",
        "gantt/04-status-and-crit-combinations",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/gantt/04-status-and-crit-combinations.mmd"
        )),
    ),
    (
        "Milestones and vert markers",
        "gantt/05-milestones-and-vert-markers",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/gantt/05-milestones-and-vert-markers.mmd"
        )),
    ),
    (
        "Excludes, weekend and weekday",
        "gantt/06-excludes-weekend-weekday",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/gantt/06-excludes-weekend-weekday.mmd"
        )),
    ),
    (
        "Time-of-day schedule",
        "gantt/07-time-of-day-schedule",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/gantt/07-time-of-day-schedule.mmd"
        )),
    ),
    (
        "Tick interval and top axis",
        "gantt/08-tickinterval-and-topaxis",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/gantt/08-tickinterval-and-topaxis.mmd"
        )),
    ),
    (
        "Today marker styling",
        "gantt/09-today-marker-styling",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/gantt/09-today-marker-styling.mmd"
        )),
    ),
    (
        "Click interactions",
        "gantt/10-click-interactions",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/gantt/10-click-interactions.mmd"
        )),
    ),
    (
        "Basic branch and merge",
        "gitgraph/02-basic-branch-and-merge",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/gitgraph/02-basic-branch-and-merge.mmd"
        )),
    ),
    (
        "Branch ordering",
        "gitgraph/03-branch-ordering",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/gitgraph/03-branch-ordering.mmd"
        )),
    ),
    (
        "Cherry-pick with parent and tag",
        "gitgraph/04-cherry-pick-with-parent-and-tag",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/gitgraph/04-cherry-pick-with-parent-and-tag.mmd"
        )),
    ),
    (
        "Merge type overrides",
        "gitgraph/05-merge-type-overrides",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/gitgraph/05-merge-type-overrides.mmd"
        )),
    ),
    (
        "Multiple tags accumulate",
        "gitgraph/06-multiple-tags-accumulate",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/gitgraph/06-multiple-tags-accumulate.mmd"
        )),
    ),
    (
        "Config directives",
        "gitgraph/07-config-directives",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/gitgraph/07-config-directives.mmd"
        )),
    ),
    (
        "Direction BT",
        "gitgraph/08-direction-bt-and-lr",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/gitgraph/08-direction-bt-and-lr.mmd"
        )),
    ),
    (
        "Title and header forms",
        "gitgraph/09-title-and-header-forms",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/gitgraph/09-title-and-header-forms.mmd"
        )),
    ),
    (
        "Basic requirement and element",
        "requirement/02-basic-requirement-and-element",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/requirement/02-basic-requirement-and-element.mmd"
        )),
    ),
    (
        "All requirement kinds",
        "requirement/03-all-requirement-kinds",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/requirement/03-all-requirement-kinds.mmd"
        )),
    ),
    (
        "All relation kinds",
        "requirement/04-all-relation-kinds",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/requirement/04-all-relation-kinds.mmd"
        )),
    ),
    (
        "Reverse relation syntax",
        "requirement/05-reverse-relation-syntax",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/requirement/05-reverse-relation-syntax.mmd"
        )),
    ),
    (
        "Quoted names and docref",
        "requirement/06-quoted-names-and-docref",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/requirement/06-quoted-names-and-docref.mmd"
        )),
    ),
    (
        "Direction and styling",
        "requirement/07-direction-and-styling",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/requirement/07-direction-and-styling.mmd"
        )),
    ),
    (
        "Contains hierarchy",
        "requirement/08-contains-hierarchy",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/requirement/08-contains-hierarchy.mmd"
        )),
    ),
    (
        "Basic grid columns",
        "block/02-basic-grid-columns",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/block/02-basic-grid-columns.mmd"
        )),
    ),
    (
        "Column span and auto",
        "block/03-column-span-and-auto",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/block/03-column-span-and-auto.mmd"
        )),
    ),
    (
        "Composite groups",
        "block/04-composite-groups",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/block/04-composite-groups.mmd"
        )),
    ),
    (
        "Shape gallery",
        "block/05-shape-gallery",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/block/05-shape-gallery.mmd"
        )),
    ),
    (
        "Directional arrow blocks",
        "block/06-directional-arrow-blocks",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/block/06-directional-arrow-blocks.mmd"
        )),
    ),
    (
        "Edge styles and labels",
        "block/07-edge-styles-and-labels",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/block/07-edge-styles-and-labels.mmd"
        )),
    ),
    (
        "ClassDef and style",
        "block/08-classdef-and-style",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/block/08-classdef-and-style.mmd"
        )),
    ),
    (
        "Basic services in a group",
        "architecture/02-basic-services-in-group",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/architecture/02-basic-services-in-group.mmd"
        )),
    ),
    (
        "Multi-group topology",
        "architecture/03-multi-group-topology",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/architecture/03-multi-group-topology.mmd"
        )),
    ),
    (
        "Titled edges",
        "architecture/04-titled-edges",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/architecture/04-titled-edges.mmd"
        )),
    ),
    (
        "Align row and column",
        "architecture/05-align-row-and-column",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/architecture/05-align-row-and-column.mmd"
        )),
    ),
    (
        "Built-in icon gallery",
        "architecture/06-builtin-icon-gallery",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/architecture/06-builtin-icon-gallery.mmd"
        )),
    ),
    (
        "Unknown icon fallback",
        "architecture/07-unknown-icon-fallback",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/architecture/07-unknown-icon-fallback.mmd"
        )),
    ),
    (
        "Basic columns and tasks",
        "kanban/02-basic-columns-and-tasks",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/kanban/02-basic-columns-and-tasks.mmd"
        )),
    ),
    (
        "Id label bracket form",
        "kanban/03-id-label-bracket-form",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/kanban/03-id-label-bracket-form.mmd"
        )),
    ),
    (
        "Task metadata full",
        "kanban/04-task-metadata-full",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/kanban/04-task-metadata-full.mmd"
        )),
    ),
    (
        "Ticket base url config",
        "kanban/05-ticket-base-url-config",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/kanban/05-ticket-base-url-config.mmd"
        )),
    ),
    (
        "Priority color gallery",
        "kanban/06-priority-color-gallery",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/kanban/06-priority-color-gallery.mmd"
        )),
    ),
    (
        "Basic sections",
        "timeline/02-basic-sections",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/timeline/02-basic-sections.mmd"
        )),
    ),
    (
        "Multi event periods",
        "timeline/03-multi-event-periods",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/timeline/03-multi-event-periods.mmd"
        )),
    ),
    (
        "Vertical direction",
        "timeline/04-vertical-direction",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/timeline/04-vertical-direction.mmd"
        )),
    ),
    (
        "Sectionless multicolor",
        "timeline/05-sectionless-multicolor",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/timeline/05-sectionless-multicolor.mmd"
        )),
    ),
    (
        "Disable multicolor config",
        "timeline/06-disable-multicolor-config",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/timeline/06-disable-multicolor-config.mmd"
        )),
    ),
    (
        "Basic branches",
        "mindmap/02-basic-branches",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/mindmap/02-basic-branches.mmd"
        )),
    ),
    (
        "Shape gallery",
        "mindmap/03-shape-gallery",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/mindmap/03-shape-gallery.mmd"
        )),
    ),
    (
        "Icons and classes",
        "mindmap/04-icons-and-classes",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/mindmap/04-icons-and-classes.mmd"
        )),
    ),
    (
        "Deep nesting",
        "mindmap/05-deep-nesting",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/mindmap/05-deep-nesting.mmd"
        )),
    ),
    (
        "Multiline markdown labels",
        "mindmap/06-multiline-markdown-labels",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/mindmap/06-multiline-markdown-labels.mmd"
        )),
    ),
    (
        "Bar and line combo",
        "xychart/02-bar-and-line-combo",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/xychart/02-bar-and-line-combo.mmd"
        )),
    ),
    (
        "Numeric axis both sides",
        "xychart/03-numeric-axis-both-sides",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/xychart/03-numeric-axis-both-sides.mmd"
        )),
    ),
    (
        "Per point labels",
        "xychart/04-per-point-labels",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/xychart/04-per-point-labels.mmd"
        )),
    ),
    (
        "Series titles and legend",
        "xychart/05-series-titles-and-legend",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/xychart/05-series-titles-and-legend.mmd"
        )),
    ),
    (
        "Horizontal orientation",
        "xychart/06-horizontal-orientation",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/xychart/06-horizontal-orientation.mmd"
        )),
    ),
    (
        "Basic day",
        "journey/02-basic-day",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/journey/02-basic-day.mmd"
        )),
    ),
    (
        "Single actor focus",
        "journey/03-single-actor-focus",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/journey/03-single-actor-focus.mmd"
        )),
    ),
    (
        "Many actors per task",
        "journey/04-many-actors-per-task",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/journey/04-many-actors-per-task.mmd"
        )),
    ),
    (
        "Score extremes",
        "journey/05-score-extremes",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/journey/05-score-extremes.mmd"
        )),
    ),
    (
        "Basic hierarchy",
        "treemap/02-basic-hierarchy",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/treemap/02-basic-hierarchy.mmd"
        )),
    ),
    (
        "Deep nesting multi level",
        "treemap/03-deep-nesting-multi-level",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/treemap/03-deep-nesting-multi-level.mmd"
        )),
    ),
    (
        "ClassDef styling",
        "treemap/04-classdef-styling",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/treemap/04-classdef-styling.mmd"
        )),
    ),
    (
        "Value format and hide values",
        "treemap/05-value-format-and-hide-values",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/treemap/05-value-format-and-hide-values.mmd"
        )),
    ),
    (
        "Basic four quadrants",
        "quadrant/02-basic-four-quadrants",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/quadrant/02-basic-four-quadrants.mmd"
        )),
    ),
    (
        "Point inline styling",
        "quadrant/03-point-inline-styling",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/quadrant/03-point-inline-styling.mmd"
        )),
    ),
    (
        "ClassDef point classes",
        "quadrant/04-classdef-point-classes",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/quadrant/04-classdef-point-classes.mmd"
        )),
    ),
    (
        "Chart size config",
        "quadrant/05-chart-size-config",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/quadrant/05-chart-size-config.mmd"
        )),
    ),
    (
        "Basic multi curve",
        "radar/02-basic-multi-curve",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/radar/02-basic-multi-curve.mmd"
        )),
    ),
    (
        "Keyed curve values",
        "radar/03-keyed-curve-values",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/radar/03-keyed-curve-values.mmd"
        )),
    ),
    (
        "Graticule polygon and options",
        "radar/04-graticule-polygon-and-options",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/radar/04-graticule-polygon-and-options.mmd"
        )),
    ),
    (
        "Config sizing and tension",
        "radar/05-config-sizing-and-tension",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/radar/05-config-sizing-and-tension.mmd"
        )),
    ),
    (
        "Basic flow",
        "sankey/02-basic-flow",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/sankey/02-basic-flow.mmd"
        )),
    ),
    (
        "Csv header and quoted field",
        "sankey/03-csv-header-and-quoted-field",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/sankey/03-csv-header-and-quoted-field.mmd"
        )),
    ),
    (
        "Link color and alignment config",
        "sankey/04-link-color-and-alignment-config",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/sankey/04-link-color-and-alignment-config.mmd"
        )),
    ),
    (
        "Absolute bit ranges",
        "packet/02-absolute-bit-ranges",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/packet/02-absolute-bit-ranges.mmd"
        )),
    ),
    (
        "Relative plus n fields",
        "packet/03-relative-plus-n-fields",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/packet/03-relative-plus-n-fields.mmd"
        )),
    ),
    (
        "Config bits per row",
        "packet/04-config-bits-per-row",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/packet/04-config-bits-per-row.mmd"
        )),
    ),
    (
        "Basic slices",
        "pie/02-basic-slices",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/pie/02-basic-slices.mmd"
        )),
    ),
    (
        "Showdata and standalone title",
        "pie/03-showdata-and-standalone-title",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/pie/03-showdata-and-standalone-title.mmd"
        )),
    ),
    (
        "Donut and legend config",
        "pie/04-donut-and-legend-config",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/pie/04-donut-and-legend-config.mmd"
        )),
    ),
];

/// The `kind` portion of a sample stem: the part before the first `/`, e.g.
/// `"flowchart"` for both `"flowchart"` and `"flowchart/02-nested-subgraphs"`.
fn kind_of(stem: &str) -> &str {
    stem.split('/').next().unwrap()
}

/// A primary sample is the flat `<kind>.mmd` file (implicitly "example 01"),
/// embedded into `src/lib.rs`'s rustdoc page. A secondary sample lives under
/// `<kind>/NN-slug.mmd` and exists only as a render-fixture + index entry.
fn is_primary(stem: &str) -> bool {
    !stem.contains('/')
}

/// A human-readable link label derived from a sample's stem. The primary has
/// no slug of its own (it's implicitly "example 01"), so it's always
/// labeled "Basic"; a secondary's `NN-slug` suffix has its number stripped,
/// dashes turned into spaces, and its first letter capitalized.
fn slug_title(stem: &str) -> String {
    if is_primary(stem) {
        return "Basic".to_string();
    }
    let slug = stem.rsplit('/').next().unwrap();
    let slug = slug.split_once('-').map_or(slug, |(_, rest)| rest);
    let mut words = slug.split('-');
    let mut out = String::new();
    if let Some(first) = words.next() {
        let mut chars = first.chars();
        if let Some(c) = chars.next() {
            out.push(c.to_ascii_uppercase());
            out.push_str(chars.as_str());
        }
    }
    for w in words {
        out.push(' ');
        out.push_str(w);
    }
    out
}

/// Browsable index of every sample grouped by kind, in first-appearance
/// order, primary first then secondaries in stem order — links only, no
/// re-embedded images (that would recreate the same doc-bloat problem the
/// primary/secondary split exists to avoid).
fn gallery_index() -> String {
    let mut kinds: Vec<&str> = Vec::new();
    for (_, stem, _) in SAMPLES {
        let k = kind_of(stem);
        if !kinds.contains(&k) {
            kinds.push(k);
        }
    }

    let mut out = String::from("# Diagram gallery\n\n\
        One or more rendered examples per supported diagram kind. The first \
        example of each kind is also embedded in the crate's rustdoc page; \
        the rest are additional fixtures linked here only, to keep that page \
        from ballooning.\n\n");
    for kind in kinds {
        let primary = SAMPLES
            .iter()
            .find(|(_, stem, _)| *stem == kind)
            .unwrap_or_else(|| panic!("no primary sample for kind {kind}"));
        out.push_str(&format!("## {}\n\n", primary.0));

        let mut entries: Vec<&(&str, &str, &str)> =
            SAMPLES.iter().filter(|(_, stem, _)| kind_of(stem) == kind).collect();
        entries.sort_by_key(|(_, stem, _)| (!is_primary(stem), *stem));
        for (_, stem, _) in entries {
            out.push_str(&format!("- [{}]({stem}.md)\n", slug_title(stem)));
        }
        out.push('\n');
    }
    out
}

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
