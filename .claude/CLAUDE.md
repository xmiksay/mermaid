# mermaid-svg

Single-crate Rust library that renders [Mermaid](https://mermaid.js.org/)
diagrams to SVG. No Node.js, no JVM, no native binaries. Ships a `mermaid-svg`
binary alongside the library.

## Layout

```
src/
├── lib.rs           public API: render*/parse/Diagram/ast::*/Theme/errors
├── bin/
│   └── mermaid-svg.rs   CLI (stdin/file → stdout/file, --theme flag)
├── parse/           Mermaid source → Diagram AST (line-oriented scanners)
│   ├── mod.rs       parse()/parse_with_meta() dispatcher, ParseError, ast re-export
│   ├── ast.rs       all AST types (pub via lib.rs as `ast::*`) incl. DiagramMeta
│   ├── preamble.rs  strips frontmatter/%%{init}%%/accTitle/accDescr → DiagramMeta
│   ├── style.rs     `classDef`/`class`/`:::className`/`style`/`linkStyle` parsing
│   ├── token.rs     quote-aware tokenizing: unquote/unquote_any/find_unquoted/split_unquoted
│   └── {pie,sequence,flowchart,state,class,er,gantt,
│        journey,timeline,sankey,quadrant,xychart,radar,packet,mindmap,
│        gitgraph,requirement,c4,block,architecture,kanban,treemap,zenuml}.rs
├── svg/             Diagram AST → SVG string
│   ├── mod.rs       render*/render_diagram* dispatchers, RenderError, pub Theme
│   ├── builder.rs   string-based SVG writer (escape, fnum, SvgBuilder)
│   ├── geometry.rs  shared edge-clip (clip_rect/circle/rhombus) + polyline_midpoint
│   ├── label.rs     decode_label: `#…;` entity codes + markdown-string emphasis
│   ├── decorate.rs  post-render role/aria + <title>/<desc> injection from DiagramMeta
│   ├── theme.rs     Theme struct + default_theme/dark/forest/neutral + with_font*
│   ├── style.rs     resolves classDef/style/linkStyle into inline fill/stroke
│   └── {pie,sequence,flowchart,state,class,er,gantt,
│        journey,timeline,sankey,quadrant,xychart,radar,packet,mindmap,
│        gitgraph,requirement,c4,block,architecture,kanban,treemap}.rs
├── sugiyama/        layered graph layout (private)
│   ├── mod.rs       Graph/Layout/LayoutConfig/LayoutError + layout_with()
│   ├── tests.rs
│   └── {cycle,layer,order,coord,route,work}.rs
examples/render_user.rs        small one-shot example
examples/gen-doc-diagrams.rs   regenerates assets/gallery.md (the rustdoc gallery)
tests/integration.rs           end-to-end tests; writes samples to target/test-samples/
samples/                       one `.mmd` per diagram kind, shared by benches + gallery
assets/gallery/<stem>.md       one rendered gallery section per SAMPLES entry,
                               embedded into rustdoc via src/lib.rs
gallery_build.rs               shared `SAMPLES` list + section helper, `include!`'d into
                               examples/gen-doc-diagrams.rs and tests/integration.rs
```

Cargo manifest: single `[package]`. Crate is published to crates.io as
`mermaid-svg`.

## Gallery pipeline

`gallery_build.rs` is not a module — it is `include!`'d verbatim into both
`examples/gen-doc-diagrams.rs` and `tests/integration.rs`, so its `SAMPLES`
list (one `(stem, source)` per diagram kind) and `gallery_section()` helper are
shared. `cargo run --example gen-doc-diagrams` regenerates one
`assets/gallery/<stem>.md` per `SAMPLES` entry (23 files), rewriting only the
files whose content changed and printing each rewrite — so `git status` after a
regen shows exactly which diagrams a change affected. `src/lib.rs` embeds them
into the crate rustdoc with one `#![doc = include_str!("../assets/gallery/<stem>.md")]`
per stem in `SAMPLES` order (`#![doc]` attributes concatenate in order). The
`doc_gallery_up_to_date` integration test names the stale stem if any committed
file drifts from the samples.

The split (one file per diagram, `assets/gallery/*.md`) keeps parallel
renderer PRs from conflicting on a shared base64 blob: a PR touching one
diagram regenerates exactly one gallery file. `.gitattributes` marks
`assets/gallery/*.md linguist-generated=true` so the blobs stay collapsed in
GitHub diffs. Changing `SAMPLES` itself (add/remove/reorder a stem) fans out to
the `lib.rs` include lines, so treat it as a serial-window change.

## Done

| Feature | Status |
|---|---|
| sugiyama layout (cycle/layer/order/coord/route) | done |
| pie · sequence · flowchart · state · class · ER · gantt parsers | done |
| journey · timeline · sankey · quadrant · xychart · radar · packet parsers | done |
| mindmap · gitGraph · requirement · C4 · block · architecture · kanban · treemap · zenuml parsers | done |
| Matching SVG renderers (zenuml reuses sequence renderer) | done |
| Themes (default, dark, forest, neutral + user-defined) | done |
| CLI binary (`mermaid-svg`) | done |
| Cross-cutting preamble (frontmatter title/theme, `%%{init}%%`, accTitle/accDescr) | done |
| Responsive SVG output + `role`/`aria`/`<title>`/`<desc>` accessibility | done |
| `#…;` entity codes + markdown-string emphasis in labels | done |

## Build & test

```bash
cargo build              # library + binary
cargo test               # unit + integration + doctest (307 tests)
cargo run --bin mermaid-svg -- --help
cargo bench              # criterion benches: parse + render per diagram
cargo package --allow-dirty
```

Bench layout: `benches/render.rs` drives criterion; it `include_str!`s the same
top-level `samples/` `.mmd` files (one per diagram kind) used by the gallery.
Two groups: `parse/<kind>` (parse only)
and `render/<kind>` (parse + render to SVG). Sized inputs use realistic
non-trivial examples (typically 10-30 lines).

Integration tests write one sample SVG per diagram kind to
`target/test-samples/<stem>.svg`, one stem per `SAMPLES` entry in
`gallery_build.rs`:
- `pie.svg`, `sequence.svg`
- `flowchart.svg`, `state.svg`
- `class.svg`, `er.svg`
- `gantt.svg`, `journey.svg`
- `timeline.svg`, `sankey.svg`
- `quadrant.svg`, `xychart.svg`
- `radar.svg`, `packet.svg`
- `mindmap.svg`, `gitgraph.svg`
- `requirement.svg`, `c4.svg`
- `block.svg`, `architecture.svg`
- `kanban.svg`, `treemap.svg`, `zenuml.svg`

## Themes — internal contract

Each per-diagram `render(d, theme: &Theme)` and any helper that touches a
theme color receives `theme: &Theme` and starts with local bindings:

```rust
fn draw_thing(svg: &mut SvgBuilder, …, theme: &Theme) {
    let fg = theme.fg;
    let flow_node_fill = theme.flow_node_fill;
    …
}
```

`format!` strings then use plain identifiers (`{fg}`), since Rust's named
format args don't support field access.

When adding a new color to `Theme`, also add it to all four built-in
constructors in `src/svg/theme.rs`. Custom themes use struct-update syntax
from one of the built-ins, so adding a field is non-breaking.

## Conventions

- No extra comments — only where the *why* is non-obvious from the code.
- No `#[allow(dead_code)]` in library code.
- Tests: unit tests in `#[cfg(test)] mod tests` at the end of each file;
  end-to-end tests in `tests/integration.rs`; private-API sugiyama tests in
  `src/sugiyama/tests.rs`.
- Errors via `thiserror`. No stringly-typed errors.
- Every public `ast::*` enum is `#[non_exhaustive]` so adding a diagram kind,
  shape, or variant stays a minor release. Keep new public AST enums marked
  the same way; downstream `match`es must carry a `_` arm.
- `NodeId = u32` in sugiyama; upper layers map their own `String → u32`.
- Keep files small — under 500 LoC. Split a module before it grows past that.
- DRY and KISS: factor out repetition into shared helpers, and prefer the
  simplest approach that works over clever or over-general designs.
- Stay faithful to the original JS-rendered Mermaid output — match its visual
  layout and styling rather than inventing a new look.
- When adding new functionality, refresh the relevant docs in the same change:
  this file (CLAUDE.md), `README.md`, and `Cargo.toml` (description/keywords).
- Always write tests for new functionality, and make sure the full suite
  (`cargo test`) passes before committing.
- Run `cargo fmt` before every commit, and keep `cargo clippy` clean — no
  warnings (treat them as errors before committing).

## Flowchart pipeline (important)

Direction transform in `src/svg/flowchart.rs`: sugiyama only knows top-down,
so for `LR`/`RL` we **swap input sizes** `(w, h) → (h, w)` and **output
coordinates** `(sx, sy) → (sy, sx)`. For `BT`/`RL` we flip the axis.

Edge clipping (`clip_to_node`) has per-shape variants:
- rect: `t = min(hw/|dx|, hh/|dy|)`
- circle: normalize to radius
- rhombus: `t = 1 / (|dx|/hw + |dy|/hh)`
- other shapes fall back to rect

## Things to remember

- **Source preamble** (`src/parse/preamble.rs`) is stripped by
  `parse_with_meta` *before* per-diagram dispatch, yielding a `DiagramMeta`
  (title, `acc_title`, `acc_descr`, `theme`): YAML frontmatter (`--- title: …
  / config: { theme: … } ---`), `%%{init: {theme: …}}%%` directives, and
  `accTitle:`/`accDescr:` (line + `accDescr { … }` block). `parse()` still
  returns just the `Diagram`; a frontmatter `title` is copied onto the
  diagram's own `title` field when it has one (flowchart gained a `title`).
- **Rendering is `parse_with_meta` → `render_body` (per-diagram match) →
  `decorate::apply`.** A preamble `theme` overrides the caller's theme.
  `decorate` (string surgery on the finished doc) always adds
  `role="graphics-document document"` + `aria-roledescription="<kind>"`, and
  when meta carries accTitle/accDescr injects `<title>`/`<desc>` + the matching
  `aria-labelledby`/`aria-describedby`. `render_diagram_with` (no meta) still
  gets role/aria but no title/desc.
- **Output is responsive**: `SvgBuilder::finish()` emits `width="100%"` +
  `style="max-width: {w}px;"` + `viewBox` and **no fixed height** (upstream
  shape). Tests must not assert a root `height="…"`.
- **Label text is decoded** in `SvgBuilder::text()` via `decode_label`
  (`src/svg/label.rs`):
  `#…;` entity codes (`#quot;`→`"`, `#35;`→`#`, `#9829;`/`#x2665;`→`♥`, named
  set) and backtick-fenced markdown *strings* have their `**`/`*`/`_` emphasis
  stripped. Bare labels with `_`/`*` (e.g. `snake_case`) are left untouched.
- Pie drops slices `< 1%` of the total (`MIN_SLICE`, matching upstream
  `createPieArcs`); insertion order and per-slice palette color are preserved.
- Quadrant points carry optional styling on `QuadrantPoint`: a third array
  value `[x, y, r]` sets `radius`; trailing `radius:`/`color:`/`stroke-color:`/
  `stroke-width:` attributes and a `:::class` ref (resolved against
  `QuadrantDiagram::classes`, filled from top-level `classDef <name> …` lines)
  set the rest. Inline attrs override the array radius and the class default;
  the renderer falls back to `r=6`, the palette fill, and a white 1.5px stroke.
- Sankey nodes render their **throughput value** after the name
  (`Name\n42`, upstream `showValues` — on by default). The value is the node's
  `max(in, out)` flow; the `SvgBuilder::text` multi-line path stacks it as a
  second `<tspan>`.
- xychart series accept an optional **quoted title** — `bar "Revenue" [..]` /
  `line "Trend" [..]` parses into `XySeries.title` (previously a hard error);
  upstream draws no legend, so the renderer ignores it. Category lists split
  **quote-aware** (`split_unquoted`) so a `"a, b"` cell survives the comma.
- Treemap honors `classDef <name> <props>` (into `TreemapDiagram.class_defs`)
  and a node's trailing `:::name` (into `TreemapNode.class_name`, stripped
  before the label/value colon split). The renderer resolves the class through
  the shared `resolve_style`, overriding the palette fill/stroke — the raw
  `:::name` no longer leaks into the label text.
- Sugiyama waypoints include **endpoints** (center of src, center of dst).
  The SVG renderer clips them to the node boundary itself.
- Flowchart `;` is a **statement terminator/separator** anywhere a newline is
  accepted (upstream grammar). `parse()` flattens each source line into its
  `;`-separated statements via `split_semicolons` before dispatch, so `graph
  TD;`, `A-->B;`, and `graph LR; A-->B` (header + statements on one line) all
  parse. A `;` inside a quoted string, a shape bracket, or an edge-label `|…|`
  run is left intact (so `["a;b"]` and `#59;` entity codes survive).
- Flowchart `~~~` is the **invisible link** (`EdgeLine::Invisible`): `parse_arrow`
  accepts `~` as an opener, requires ≥3 tildes, and forbids any head/tail. It is
  a real edge (so it shapes the sugiyama layout) but `draw_edge` returns early
  for `Invisible`, drawing nothing. A `~`/`~~` run under 3 is not an edge.
- Flowchart `FlowEdge` has separate `line` (Solid/Dotted/Thick), `head`
  (None/Arrow/Circle/Cross), and `tail` (start-side head, same enum) — covers
  `-->`, `---`, `-.->`, `==>`, `--o`, `--x` plus all no-head variants, and the
  bidirectional forms `<-->`, `o--o`, `x--x` (`tail` set). `parse_arrow` reads
  an optional leading `<`/`o`/`x` before the line dashes; `o`/`x` count as a
  tail marker only when a line char (`-`/`=`/`.`) immediately follows, so a
  bare node id like `o` stays a node. The renderer emits `marker-start` (the
  markers' `orient="auto-start-reverse"` flips them to point outward).
- Flowchart edge labels come in two forms: the pipe form `A -->|text| B` and
  the inline form `A -- text --> B` (also `-. text .->`, `== text ==>`). The
  inline form is recognized in `parse_arrow` via `read_inline_label`: a
  two-char opener (`--`/`-.`/`==`) with no head, followed by text and a
  matching closer, captures the text as the edge label instead of a chain
  node. A head-less solid/thick closer needs ≥3 connectors so a plain
  `A -- B -- C` chain is left untouched.
- `A & B --> C & D` produces 4 edges (cross product) — multi-source/target.
- Flowchart node `(text)` (round) renders as a small `rx="5"` rounded rect;
  only stadium `([text])` is a full pill (`rx = h/2`) — the two shapes are
  visually distinct (`draw_node`).
- Flowchart `subgraph` is tracked in `FlowchartDiagram.subgraphs` including
  nesting. The renderer draws a solid rounded cluster frame with the themed
  `flow_cluster_fill`/`flow_cluster_stroke` and a centered bold top label
  (`draw_subgraphs`).
  - `style <id>`/`class <id> <name>` naming a subgraph id styles the cluster
    frame: the directive lands on the phantom node dropped during subgraph-id
    cleanup, so the parser moves its `style`/`classes` onto `Subgraph.style`/
    `Subgraph.classes` first; the renderer resolves them through the shared
    `resolve_style` (fill/stroke override the theme cluster colors).
  - Mermaid v11 edge ids parse and are ignored: the `e1@` prefix in
    `A e1@--> B` (`consume_edge_id`, recorded in an `edge_ids` set) and a
    standalone `e1@{ … }` edge-attribute statement (`edge_attr_stmt_id`, dropped
    when the id is a known edge) — so v11 documents render instead of erroring.
  - `direction X` inside a subgraph body fills `Subgraph.direction`. The
    renderer works in screen space and, for a cluster whose flow axis differs
    from the diagram's, transposes just that cluster's members (and their
    internal edges) about the cluster centre (`apply_local_directions`) — a TD
    chain inside a `direction LR` subgraph becomes a horizontal row.
  - An edge endpoint naming a subgraph id refers to the cluster, not a node.
    The parser drops any node materialized for a subgraph id (forward ref or
    edge target); the renderer routes such an edge as a straight connector
    clipped to the cluster bounding box (`endpoint_clip` → `EndClip` with a
    `None` shape → rectangle clip).
- Flowchart `click <id>` sets `FlowNode.click` (`ClickAction::Href` for
  `"url"`/`href` forms, `ClickAction::Callback` for a bare name/`call fn()`).
  The renderer wraps hyperlink nodes in `<a href>` and callback nodes in a
  `<g class="clickable" onclick>`; an optional tooltip becomes a `<title>`.
- Sequence parser has **nested items** (`Vec<SequenceItem>`) — `Alt`/`Par`/
  `Critical` blocks have branches; `Loop`/`Opt`/`Break` have label + items;
  `Rect` has a color + items. Renderer draws labeled frames with tab labels
  (`break` reuses the frame with a `break` title); `rect <color>` draws a
  colored background band behind its items via a separate `draw_rect_bands`
  pass (paired `RectOpen`/`RectClose` events, LIFO stack, default fill
  `rgba(0,0,0,0.05)` when no color given).
- Sequence `autonumber` is **positional**: it parses to
  `SequenceItem::AutoNumber(Option<AutoNumberConfig>)` interleaved in `items`.
  `autonumber [start [step]]` → `Some{start,step}` (defaults 1/1) turns numbering
  on and resets the counter to `start`; `autonumber off` → `None` turns it off
  for subsequent messages. The renderer threads a `&mut Numbering { on, step }`
  plus a counter through `layout_items`, emitting `"{n}. {text}"` for numbered
  messages. `SequenceDiagram.autonumber` stays a bool flag ("was ever on").
- Sequence `activate`/`deactivate` is paired and drawn as an activation band
  on the lifeline. `draw_activations` keeps a **stack** of open start-ys per
  participant (`HashMap<String, Vec<f64>>`) so nested/stacked activations (the
  `->>+` shorthand) draw one band per level, each offset `level * 3px` to the
  right instead of overwriting. Activations still open at the end of the event
  loop are flushed down to `lifeline_bottom`.
  - The `->>+`/`-->>-` **activation shorthand** is handled in the parser
    (`parse_message` in `src/parse/sequence.rs`): a leading `+`/`-` on the
    target id is stripped (not part of the participant name) and
    `parse_line_to_items` synthesizes the paired event — `+` appends
    `Activate(target)` *after* the message, `-` prepends `Deactivate(target)`
    *before* it, matching upstream ordering.
- Sequence `actor X` (vs `participant X`) renders as a **stick figure** (circle
  head + body/arms/legs, name below) instead of the rounded rect — `draw_actor`
  in `src/svg/sequence.rs` branches on `Participant.kind`.
- Sequence `box <color> <label>` groups participants: `SequenceBox` carries an
  optional `color` (parsed in `split_box_color` — hex, `rgb()/rgba()`, or a
  named CSS color; else the whole string is the label) plus the member
  `participant_ids` (any participant declared while the box frame is open). The
  renderer (`draw_boxes`) draws a colored background rect spanning the members
  from above the actor row to below the footer, label centered on top; a
  missing color renders transparent. Reserves `BOX_LABEL_H` above the actor row.
- Sequence `create [participant|actor] X [as Y]` / `destroy X` are **positional**
  lifecycle items (`SequenceItem::Create(id)`/`Destroy(id)`, same shape as
  `AutoNumber`). `create` also registers the participant (so it gets a column);
  the renderer draws its actor box **inline** at the create point (not the top
  row) and starts the lifeline there. `destroy` ends the lifeline with an `×`
  cross (`draw_destroy_cross`) and draws no footer box. `parse()` runs
  `reorder_destroys` so each `destroy X` is moved just past the next message
  involving `X` (the `destroy Carl` / `Alice-xCarl` idiom terminates *after*
  that message). Actor menus (`link X: … @ url`, `links X: {json}`) are consumed
  by `is_actor_menu` (not rendered) so they don't hard-error.
- State `state X { ... }` is stored in `composites`; parallel regions are
  separated by `--`. Composites are **clusters, not nodes** (like flowchart
  subgraphs): the composite id is excluded from the sugiyama graph, its members
  (gathered recursively through nested composites in `compute_composite_boxes`)
  lay out inside a dashed rounded frame, and an external transition naming the
  composite id clips to the frame box via `endpoint_clip` (a `StateEndClip` with
  `kind: None` → rect clip). Synthesized `__start_N`/`__end_N`/`__hist_N` ids are
  registered in `existing` by `push_pseudo` so region-tracking counts them as
  members and their circles render inside the frame. Pseudo-state (start/end/
  fork/join) fills use `theme.fg` so they stay visible on the dark theme.
- State aliasing `state "description" as X` binds `X`'s display label to the
  quoted text (`parse_quoted_as` in `parse_state_decl`), so the id stays clean
  and a later transition referencing `X` reuses the same state — no phantom box
  named literally `"…" as X`.
- State history pseudo-states parse to `StateKind::History { deep }`:
  `<<history>>` and `[H]` are shallow (`deep: false`), `[H*]` is deep. The
  bracket forms are handled in `canonicalize` like `[*]` (unique `__hist_N`
  id per occurrence); the stereotype form in `parse_state_decl`. The renderer
  draws a small circle with `H`/`H*` inside.
- State `note right of X: text` (one-liner) and `note left of X\n…\nend note`
  (multi-line) both land in `notes`.
- Class `namespace X { class A; class B }` is stored in `namespaces`; the
  renderer draws a dashed rect around the members.
- Class `direction` (TD/BT/LR/RL) drives the transpose the same way the
  flowchart does.
- Class relation multiplicities (`A "1" --> "*" B`) parse into
  `ClassRelation.from_card`/`to_card`; the renderer draws them as small labels
  near each edge end. Token scanning is quote-aware so cards like `"1..*"`
  (which embed the `..` token) don't split the line.
- Class relation marker orientation: `ClassRelation.reversed` records whether
  the token's decorated end (triangle/diamond/circle/arrow) is on the left, at
  the `from` class — set by `is_reversed_token` for tokens opening with `<`,
  `*`, or `o` (`<|--`, `*--`, `o--`, `<--`, `<..`). `from`→`to` order (hence
  layout) is preserved; only the marker end moves. `style_for(kind, reversed)`
  emits the single decorated marker as `marker-start` (reversed) or `marker-end`
  (forward); `orient="auto-start-reverse"` points it into its node at either
  end. Composition/aggregation draw *only* the diamond — no far-end arrowhead.
- Class generics `~T~` are converted to angle brackets at render time
  (`convert_generics` in `src/svg/class.rs`) for class names and member/return
  types — `List~int~` → `List<int>`, nested `List~List~int~~` →
  `List<List<int>>`, `Map~string, int~` → `Map<string, int>` (innermost pair
  first; a lone unmatched `~` is left alone). The same `member_display` pass
  strips the trailing UML classifier (`*` abstract → `font-style="italic"`,
  `$` static → `text-decoration="underline"`).
- Class notes/annotations/labels/interactivity (`src/parse/class.rs`):
  `note "text"` (free) and `note for <Class> "text"` (attached) fill
  `ClassDiagram.notes` (`ClassNote { target, text }`); the renderer draws them
  as yellow sticky boxes in a row below the diagram, with a dashed connector to
  the target class. Standalone annotations parse in **either** order —
  `<<interface>> Shape` and `Shape <<interface>>` — via
  `parse_standalone_annotation`. A `class Name["label"]` sets `UmlClass.label`
  (the display text), keeping `name` clean — no phantom duplicate box.
  `click`/`link`/`callback` lines bind a `UmlClass.click` (reusing the flowchart
  `ClickAction`), parsed before the `:`-shorthand split so a URL's `https://`
  colon can't misroute the line. The shared `open_click`/`close_click` wrappers
  live in `src/svg/interact.rs` (used by both the flowchart and class renderers).
- ER `EntityAttribute.comment` is populated from a quoted string after the
  attribute (`string name "the customer name"`) and rendered as a fourth
  attribute column (type · name · key · comment). `EntityAttribute.key` holds
  all comma-separated key constraints joined as `PK, FK`.
- ER relations accept both the glyph cardinality form (`||--o{`) and the
  **verbal/numeric** form `LEFT <card> to|optionally to <card> RIGHT : label`
  (`src/parse/er.rs`, `find_reltype` + `split_card_end`/`split_card_start`):
  `to` is identifying, `optionally to` non-identifying; cardinality words
  (`only one`, `zero or one`, `zero or more`, `one or many`, `many(0)`, `0+`,
  `1+`, `1`) map onto the existing `Cardinality` enum.
- ER entity alias `id[Alias] { … }` (and bare `id[Alias]`) sets `Entity.label`
  (display) while `Entity.name` (the id relations reference) stays clean —
  `split_id_label`, mirroring the flowchart/kanban split. `ensure_entity`
  upgrades a placeholder label when the aliased block/decl appears after a
  relation already materialized the entity.
- ER `direction TB/BT/LR/RL` fills `ErDiagram.direction`; the renderer drives
  the same size-swap/transpose the flowchart and class renderers use.
- Gantt dates are **exact civil day-counts from the Unix epoch**
  (`src/svg/gantt_date.rs`: `days_from_civil`/`civil_from_days`/`weekday`, the
  Hinnant algorithms) — no more `365.25`-day drift. `parse_date` honors the
  `dateFormat` field order (`DD-MM-YYYY` etc.), `format_date` renders axis
  ticks with the `axisFormat` d3/`strftime` subset (`%Y %m %d %b %a …`, default
  ISO `%Y-%m-%d`).
- Gantt `excludes` (weekends / weekday names / specific dates) is honored by
  the renderer via `Excludes` (`src/svg/gantt_date.rs`): each non-working day
  gets a light shading band behind the bars, and duration-based tasks are
  **stretched** over excluded days (`Excludes::stretched_end`, matching
  upstream's `getMaxEndTime`). Explicit end-date / `until` tasks are not
  stretched. `todayMarker YYYY-MM-DD` still draws a vertical red line.
- Gantt task tags are consumed as a leading run in `parse_task`: `active`/
  `done` set `TaskStatus`, while `crit` and `milestone` are **orthogonal
  flags** (`GanttTask.crit`/`.milestone`) — upstream combines them, so
  `done, crit` keeps the done fill with a crit (red) border instead of the last
  tag winning. `colors_for(status, crit)` picks the fill from the status and a
  red border for `crit` (crit-only also takes the red fill). A milestone
  renders as a diamond (rotated square `<path>`) centered on the start date with
  the label beside it — duration is ignored.
- Gantt task end is a `TaskEnd` enum (not a bare `duration_days`): `Duration`
  (`Nd`/`Nw`/`Nh`/`Nm`), `Date` (an explicit end date — the renderer computes
  the length from the resolved start), or `UntilId` (`until <taskId>` — ends
  where the named task *starts*, resolved against `id_to_start_end`). `parse_end`
  in `src/parse/gantt.rs` classifies the trailing time token; a task with a
  single time token (`X : 24d` / `X : until id`) implies `TaskStart::AfterPrevious`.
  `until`/end-date resolution happens in `resolve_tasks` (`src/svg/gantt.rs`),
  so forward/unknown refs fall back to a 1-day length like `after` does.
  Config keywords `tickInterval …`, `inclusiveEndDates`, `topAxis` are consumed
  in `parse()` (informational only) so they don't fall through to the task path.
- Asymmetric flowchart shapes are fully supported: parallelogram `[/text/]`,
  parallelogram-alt `[\text\]`, trapezoid `[/text\]`, trapezoid-alt
  `[\text/]`, and the asymmetric flag `>text]` — parsed in
  `src/parse/flowchart.rs` and rendered in `src/svg/flowchart.rs`.
- Flowchart v11 attribute syntax `id@{ shape: …, label: … }` is handled in
  `parse_at_node` (`src/parse/flowchart.rs`): the `@{…}` block right after a
  node id is split into `key: value` pairs (quote-aware comma/colon split), the
  `shape` name mapped onto a `NodeShape` by `shape_from_name` (aliases like
  `rounded`/`diam`/`cyl`/`lean-r`/`trap-b`/`dbl-circ`/`subproc`; unknown or
  visual-only names such as `bolt`/`hourglass`/`notch-rect` fall back to Rect),
  and `label`/`title` set the node text. `icon`/`img` forms are dropped but
  their `label` is preserved so content is never lost.
- Label line breaks: `split_label_lines()` in `src/svg/builder.rs` splits any
  label on `<br>`/`<br/>`/`<br />` (case-insensitive) and `\n` (real newline or
  the two-char literal escape). `SvgBuilder::text()` auto-emits stacked
  `<tspan>`s for multi-line labels, so every renderer honors `<br>` for free;
  flowchart also sizes nodes from the resulting line count / widest line.
- C4 supports the full `{System,Container,Component} × {Db,Queue} × {_Ext}`
  element matrix; the `_Ext` variants reuse the same shape with the gray
  external palette. `UpdateElementStyle` / `UpdateRelStyle` /
  `UpdateLayoutConfig` are stored on `C4Diagram` (`element_styles`,
  `rel_styles`, `layout`) and applied at draw time: element `$bgColor`/
  `$fontColor`/`$borderColor`, rel `$textColor`/`$lineColor`/`$offsetX/Y`.
  `$c4ShapeInRow`/`$c4BoundaryInRow` override the row-flow wrap counts
  (`flow_layout`'s `shape_in_row`/`boundary_in_row`). `C4Relation.direction`
  (`Rel_U/D/L/R`) is parsed but not used by the row-flow layout.
  C4Deployment's `Node(...)`/`Node_L(...)`/`Node_R(...)` boundary openers alias
  `Deployment_Node` in `parse_boundary_open` (checked `Node_L`/`Node_R` before
  the bare `Node` prefix), so their children nest instead of leaking to top
  level.
- requirementDiagram (`src/parse/requirement.rs`) accepts both relation
  directions — forward `src - kind -> dst` and reverse `dst <- kind - src`
  (endpoints swapped so `from`→`to` order, hence layout, is preserved). Kind
  and requirement keywords are matched case-insensitively. The v11 statements
  `direction TB/BT/LR/RL`, `classDef`, `class`, and `style` are consumed
  instead of hard-erroring: `direction` fills `RequirementDiagram.direction`
  (drives the same size-swap/transpose the flowchart uses), while
  `classDef`/`class`/`style` fill `class_defs`/`node_classes`/`node_styles`
  (reusing `parse/style.rs` + `svg/style.rs::resolve_style`). The `contains`
  relation draws upstream's crossed-circle containment head (`req-contains`
  marker) instead of the plain arrow.
- gitGraph header (`src/parse/gitgraph.rs`) tolerates a trailing colon on both
  the keyword and the direction token — `gitGraph:`, `gitGraph TB:`,
  `gitGraph BT:` all parse (the dispatcher in `src/parse/mod.rs` also trims a
  trailing `:` off the head token). `branch <name> order: <n>` consumes the
  `order:`/`tag:` attributes instead of swallowing them into the branch name
  (`parse_branch`); `order` reaches `GitEvent::Branch.order`. The renderer
  (`src/svg/gitgraph.rs`) sorts lanes by explicit `order` (falling back to
  insertion order) and, for `BT`, flips the commit axis (`cols - 1 - col`) so
  newer commits sit higher.
- radar-beta (`src/parse/radar.rs`): multiple `axis` lines **accumulate**
  (`d.axes.extend`, not assign). Option keywords `min`/`max`/`ticks`/
  `graticule circle|polygon`/`showLegend [bool]` are consumed instead of
  hard-erroring. A curve body is either a positional list (`{85, 90}`) or
  `key: value` pairs (`{ Power: 85, Speed: 90 }`, detected by a `:`), the
  latter matched to axes by id then label — order-independent, missing axes
  default to 0. The renderer (`src/svg/radar.rs`) draws `ticks` graticule rings
  as concentric **circles** by default (`graticule polygon` for the old polygon
  rings) and scales curves over `[min, max]` so `min` acts as a scale offset;
  `showLegend false` suppresses the legend.
- Kanban columns and tasks accept the documented `id[Label]` bracket form
  (`split_id_label` in `src/parse/kanban.rs`): the text before `[` is the id,
  the bracketed text the display label (a bare `[Label]` reuses the label as
  id). Task `@{…}` metadata parses `assigned`/`priority`/`ticket`. The renderer
  (`src/svg/kanban.rs`) color-codes the card border by priority
  (`priority_color`: Very High/High/Low/Very Low; others use the default
  stroke) and draws the `ticket` id on the card — hyperlinked when
  `config.kanban.ticketBaseUrl` is set (captured in `preamble.rs` →
  `DiagramMeta.ticket_base_url`, copied onto `KanbanDiagram` in
  `parse_with_meta`; `#TICKET#` in the URL is replaced by the id).
- block-beta styling & edges (`src/parse/block.rs` / `src/svg/block.rs`):
  `classDef <name> <props>` fills `BlockDiagram.class_defs`; `class a,b <name>`
  and the inline `id:::name` shorthand fill `Block.classes`; `style <id> <props>`
  fills `Block.style`. `class`/`style` are **deferred** (a `Ctx` collects them,
  `apply_assignments` walks the item tree afterwards so an id declared *after*
  the assignment still matches). The renderer resolves them through the shared
  `resolve_style`/`ResolvedStyle` (`src/svg/style.rs`). Block arrows
  `id<["label"]>(dir)` parse to `BlockShape::Arrow(BlockArrow{right,left,up,down})`
  (`(x)`→left+right, `(y)`→up+down) and render as a shafted/double-headed path.
  Edge labels `a -- "text" --> b` are captured off the tail side in `parse_edge`
  (the label no longer swallows the arrow). Edges resolve endpoints against a
  `Geom` map that indexes **groups by id too** (so `ID --> D` where `ID` is a
  `block:ID … end` group works) and clip to the node boundary (`clip`) so
  arrowheads land on the edge, not the center. `block:id:span` keeps the span on
  `BlockGroup.span` (min group width).
- mindmap `:::class1 class2` and `::icon(fa fa-book)` are **attachment lines**,
  not child nodes (`src/parse/mindmap.rs`): both attach to the most-recent node
  (`stack.last_mut()` / `root`), `:::` filling `MindmapNode.classes` and `::icon`
  filling `MindmapNode.icon`. The renderer (`src/svg/mindmap.rs`) never prints the
  raw Font Awesome class string — `draw_mindmap_icon` maps `icon_name()` (the last
  `fa-`-prefixed token) onto a small builtin glyph set (book/star/clock/user/cog/
  cloud/database/check/heart), unknown names falling back to a generic tag glyph.
- zenuml (`src/parse/zenuml.rs`) is a **brace-structured** translation to a
  `SequenceDiagram` (reuses the sequence renderer). After the `zenuml` header the
  body is `tokenize`d into `{`/`}`/statement `Tok`s (braces inside `(…)`/quotes
  stay literal; `\n`/`;` end statements; `//` and `%%` are comments), then a
  recursive `Parser::parse_items(ctx, ret)` walks them. `ctx` is the current
  caller (the enclosing method's *receiver*, or the top-level starter); `ret` is
  who a `return` replies to.
  - Annotators: `@Actor X` declares an actor, any other `@Type X` a participant,
    `@Starter(X)` sets the top-level caller. A bare/`A.method()` call with no
    explicit `A -> B` source originates from the starter — a synthetic `Starter`
    lane, created lazily, when none is declared.
  - Method calls carry a context: `Recv.method()` → `ctx -> Recv`, `method()`
    (no dot) is a self-call on `ctx`. A `{ … }` body after a call runs in the
    receiver's context and, on close, draws an implicit dashed **return** to the
    caller; an `x = call()` assignment draws a dashed return labeled `x`
    (self-calls get no return arrow).
  - `return`/`@return <v>` emits a dashed reply from `ctx` to `ret`. Control
    structures map onto existing `SequenceItem` frames: `if/else if/else` →
    `Alt`, `while/for/loop` → `Loop`, `opt` → `Opt`, `par` → `Par`,
    `try/catch/finally` → `Critical` (catch/finally as option branches). The
    `else`/`catch`/`finally` chain tokens are consumed by their opener's handler.
