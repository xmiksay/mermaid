# mermaid-svg

Single-crate Rust library that renders [Mermaid](https://mermaid.js.org/)
diagrams to SVG. No Node.js, no JVM, no native binaries. Ships a `mermaid-svg`
binary alongside the library.

## Layout

```
src/
├── lib.rs           public API: render*/parse/Diagram/ast::*/Theme/errors
├── bin/
│   └── mermaid-svg.rs   CLI (stdin/file → stdout/file, --theme/-f|--font/--font-size flags)
├── parse/           Mermaid source → Diagram AST (line-oriented scanners)
│   ├── mod.rs       parse()/parse_with_meta() dispatcher, ParseError + SyntaxKind, ast re-export
│   ├── ast/         all AST types (pub via lib.rs as `ast::*`) incl. DiagramMeta —
│   │                mod + block/c4/charts/class/er/flowchart/gantt/sequence/state/structure
│   ├── preamble.rs  strips frontmatter/%%{init}%%/accTitle/accDescr → DiagramMeta
│   ├── style.rs     `classDef`/`class`/`:::className`/`style`/`linkStyle` parsing
│   ├── token.rs     quote-aware tokenizing: unquote/unquote_any/find_unquoted/split_unquoted
│   ├── {sequence,flowchart,state,class,c4,block,zenuml}/  multi-file per-diagram parsers (mod + submodules)
│   └── {pie,er,gantt,journey,timeline,sankey,quadrant,xychart,radar,packet,
│        mindmap,gitgraph,requirement,architecture,kanban,treemap}.rs
├── svg/             Diagram AST → SVG string
│   ├── mod.rs       render*/render_diagram* dispatchers, RenderError, pub Theme
│   ├── builder.rs   string-based SVG writer (escape, fnum, SvgBuilder)
│   ├── geometry.rs  shared edge-clip (clip_rect/circle/rhombus) + polyline_midpoint
│   ├── label.rs     decode_label: `#…;` entity codes (markdown emphasis → markup.rs)
│   ├── markup.rs    inline-HTML labels → styled tspans (b/i/u/span/a); strip_tags
│   ├── metrics.rs   shared text_width/font_scale (per-glyph widths track font_size)
│   ├── decorate.rs  post-render role/aria + <title>/<desc> injection from DiagramMeta
│   ├── theme.rs     Theme struct + default_theme/dark/forest/neutral + with_font*
│   ├── style.rs     resolves classDef/style/linkStyle into inline fill/stroke
│   ├── gantt_date.rs civil day-count date math (days_from_civil/format_date/Excludes)
│   ├── interact.rs  shared click/link wrappers (open_click/close_click)
│   ├── {sequence,flowchart,state,class,c4,block}/  multi-file per-diagram renderers (mod + submodules)
│   └── {pie,er,gantt,journey,timeline,sankey,quadrant,xychart,radar,packet,
│        mindmap,gitgraph,requirement,architecture,kanban,treemap}.rs
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
| Inline HTML labels (`<b>`/`<i>`/`<u>`/`<span style=color>`/`<a href>`) | done |

## Build & test

```bash
cargo build              # library + binary
cargo test               # unit + integration + doctest (564 tests: 548 lib + 15 integration + 1 doctest)
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

Color/font fields are `Cow<'static, str>` (not `&'static str`): built-in
constructors stay `const` (`Cow::Borrowed(...)`), but `themeVariables`/
`fontFamily` config and downstream overrides supply owned runtime strings
(`fg: "#000".into()`). `Theme` is thus `Clone`, **not** `Copy`. Renderers read a
color as `&theme.fg` (a `&Cow<str>` that deref-coerces to `&str`), so
`let fg = &theme.fg;` keeps the `format!("{fg}")` idiom working.
`Theme::apply_theme_variables(&mut self, vars)` recolors a base theme from the
upstream `themeVariables` names; `theme_from_meta` in `src/svg/mod.rs` wires
theme name → `themeVariables` → `fontFamily`/`fontSize` → `useMaxWidth` onto the
effective theme. `Theme::responsive` (default `true`) is cleared by
`config.useMaxWidth: false`, making `SvgBuilder::finish` emit a fixed pixel
`width`/`height`; every renderer adopts font + responsiveness via
`SvgBuilder::new(w, h).theme(theme)`.

## Conventions

- No extra comments — only where the *why* is non-obvious from the code.
- No `#[allow(dead_code)]` in library code.
- Tests: unit tests in `#[cfg(test)] mod tests` at the end of each file;
  end-to-end tests in `tests/integration.rs`; private-API sugiyama tests in
  `src/sugiyama/tests.rs`.
- Errors via `thiserror`. No stringly-typed errors. `ParseError::Syntax`
  carries a typed `kind: SyntaxKind` (`MissingHeader`/`UnknownStatement`/
  `InvalidNumber`/`Unclosed`/`Malformed`) beside the free-form `message`;
  construct it through the `ParseError::{header,unknown,number,unclosed,
  malformed}(line, msg)` helpers rather than the raw struct literal so the
  classification stays consistent.
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

Direction transform in `src/svg/flowchart/mod.rs`: sugiyama only knows
top-down, so for `LR`/`RL` we **swap input sizes** `(w, h) → (h, w)` and
**output coordinates** `(sx, sy) → (sy, sx)`. For `BT`/`RL` we flip the axis.

Edge clipping (`clip_to_node`, in `src/svg/flowchart/edges.rs`) has per-shape variants:
- rect: `t = min(hw/|dx|, hh/|dy|)`
- circle: normalize to radius
- rhombus: `t = 1 / (|dx|/hw + |dy|/hh)`
- other shapes fall back to rect

## Things to remember

- **Source preamble** (`src/parse/preamble.rs`) is stripped by
  `parse_with_meta` *before* per-diagram dispatch, yielding a `DiagramMeta`
  (title, `acc_title`, `acc_descr`, and the config-derived fields): YAML
  frontmatter (`--- title: … / config: { … } ---`), `%%{init: {…}}%%`
  directives, and `accTitle:`/`accDescr:` (line + `accDescr { … }` block).
  `parse()` still returns just the `Diagram`; a frontmatter `title` is copied
  onto the diagram's own `title` field when it has one (flowchart gained a
  `title`).
- **The whole `config:` tree is flattened** (frontmatter YAML via `flatten_yaml`
  indentation, `%%{init}%%` via `parse_init_object`'s JSON-ish recursion) into
  `DiagramMeta.config`, a dotted `key → value` map (`themeVariables.primaryColor`,
  `gitGraph.mainBranchName`, `kanban.ticketBaseUrl`, `flowchart.htmlLabels`, …;
  frontmatter/first-init wins). `derive_typed_fields` reads the honored subset
  out of it: `theme`, `theme_variables`, `font_family`, `font_size`,
  `use_max_width`, `look`/`layout`/`security_level` (parsed, not yet honored),
  `ticket_base_url`, `value_format`, `git_graph.*`. Closing a per-diagram config
  gap is a `meta.config` lookup, not new scanning.
- **Rendering is `parse_with_meta` → `render_body` (per-diagram match) →
  `decorate::apply`.** `theme_from_meta` builds the effective theme: a preamble
  `theme` overrides the caller's, then `themeVariables`/`fontFamily`/`fontSize`/
  `useMaxWidth` layer on top.
  `decorate` (string surgery on the finished doc) always adds
  `role="graphics-document document"` + `aria-roledescription="<kind>"`, and
  when meta carries accTitle/accDescr injects `<title>`/`<desc>` + the matching
  `aria-labelledby`/`aria-describedby`. `render_diagram_with` (no meta) still
  gets role/aria but no title/desc.
- **Output is responsive**: `SvgBuilder::finish()` emits `width="100%"` +
  `style="max-width: {w}px;"` + `viewBox` and **no fixed height** (upstream
  shape). Tests must not assert a root `height="…"`.
- **Label text is decoded** in `SvgBuilder::text()` via `decode_label`
  (`src/svg/label.rs`), which now only resolves `#…;` entity codes
  (`#quot;`→`"`, `#35;`→`#`, `#9829;`/`#x2665;`→`♥`, named set). Backtick-fenced
  markdown *strings* and their `**bold**`/`*italic*`/`__`/`_` emphasis are
  handled one layer up by `parse_spans` (`src/svg/markup.rs`): a fenced line is
  routed to `parse_markdown_spans`, which toggles bold/italic into styled
  `<tspan>`s instead of flattening the markers to plain text. A marker-free
  fenced label still collapses to one plain run (bare `<text>` fast path). Bare
  labels with `_`/`*` (e.g. `snake_case`) are never touched.
- **Inline HTML labels** (`htmlLabels`, `src/svg/markup.rs`): `SvgBuilder::text`
  first line-splits, then `parse_spans` walks each line into styled runs mapped
  onto `<tspan>`s — `<b>`/`<strong>`→`font-weight="bold"`, `<i>`/`<em>`→italic,
  `<u>`→underline, `<span style="color:…">`→`fill`, `<a href>`→wraps the run in
  an SVG `<a>`. Tag scanning runs on the raw source *before* entity decoding, so
  `#lt;`-encoded brackets never masquerade as tags; the per-run text is then
  `decode_label`-ed. Unknown tags are **stripped** (not escaped), so unsupported
  markup degrades to plain text; a bare `<` that doesn't open a well-formed tag
  stays literal. A tag-free single-line label keeps the bare `<text>` fast path,
  so the whole gallery stays byte-identical. `strip_tags` gives the visible text
  for width estimation (`node_size`). Renderers that emit *literal* angle
  brackets (class generics `List<int>`, C4 `<<stereotype>>`) entity-encode them
  (`#lt;`/`#gt;`) at the source so the markup pass keeps them intact.
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
  second `<tspan>`. Each node gets its **own palette color** (`pie_color(node
  index)`), no longer one flat fill. `config.sankey.linkColor` and
  `config.sankey.nodeAlignment` (frontmatter/`%%{init}%%`) flow through the
  preamble → `DiagramMeta.sankey_link_color`/`sankey_node_alignment` → copied
  onto `SankeyDiagram` in `parse_with_meta`. `linkColor` (`LinkColor::parse`,
  default `source`): `source`/`target` tint each link from that node's color,
  `gradient` emits a per-link `<linearGradient>` in `<defs>`, any other value is
  a literal stroke color. `nodeAlignment` (`Alignment::parse`, default
  `justify`) maps onto the column-assignment step (`assign_columns`, using
  `column_depths`/`column_heights`): `left` = depth from source, `right` =
  distance to sink, `justify` pushes sinks to the last column, `center` nudges
  source-less nodes toward their earliest target (d3-sankey semantics).
- xychart series accept an optional **quoted title** — `bar "Revenue" [..]` /
  `line "Trend" [..]` parses into `XySeries.title` (previously a hard error);
  upstream draws no legend, so the renderer ignores it. Category lists split
  **quote-aware** (`split_unquoted`) so a `"a, b"` cell survives the comma.
- Treemap honors `classDef <name> <props>` (into `TreemapDiagram.class_defs`)
  and a node's trailing `:::name` (into `TreemapNode.class_name`, stripped
  before the label/value colon split). The renderer resolves the class through
  the shared `resolve_style`, overriding the palette fill/stroke — the raw
  `:::name` no longer leaks into the label text. Layout is **squarified**
  (Bruls/Huizing/van Wijk worst-aspect-ratio row packing in `squarify`/`worst`,
  `src/svg/treemap.rs`), not slice-and-dice, so rectangles stay near square.
  `config.treemap.valueFormat` (frontmatter) flows through
  `DiagramMeta.value_format` → `TreemapDiagram.value_format` and formats leaf
  values via `format_value`: `$` prefix, `,` thousands, `.N` decimals, `%`
  percent (the common d3-format subset).
- **Parser unknown-line policy is hard-error everywhere.** Every diagram
  parser (flowchart included) returns `ParseError::Syntax { line }` on an
  unparseable statement — the honest library equivalent of upstream rendering
  its error diagram — rather than silently dropping it, so a typo can't vanish.
  Flowchart also errors on a recognized keyword with an incomplete body (bare
  `style`/`classDef`/`class`/`linkStyle`/`click`, unknown `direction` token).
  Two deliberate tolerances remain (documented in
  `src/parse/flowchart/mod.rs`): a top-level `direction` is a validated no-op,
  and unknown keys / `shape:` names inside a v11 `id@{ … }` block fall back to
  `Rect` for forward compatibility.
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
  - Mermaid v11 edge ids and attributes: the `e1@` prefix in `A e1@--> B`
    (`consume_edge_id`) names the edge — recorded in an `edge_ids` set *and*
    stored on `FlowEdge.id` — and a standalone `e1@{ animate: …, curve: … }`
    statement (`edge_attr_stmt`) applies those attributes to the matching edge
    (`apply_edge_attrs`) instead of spawning a phantom node. `animate: true`
    sets `FlowEdge.animate` (a SMIL `<animate>` on `stroke-dashoffset` in
    `draw_edge`, needing a dash pattern — falls back to `8 8`); `curve: <name>`
    sets `FlowEdge.curve` (`EdgeCurve::from_name`). `linkStyle N interpolate
    <curve>` / `linkStyle default interpolate <curve>` fill
    `FlowchartDiagram.edge_interpolate`/`default_interpolate`. The renderer
    resolves the effective curve per edge — `@{ curve }` → per-index
    interpolate → default interpolate → basis — and `curve_basis_path`,
    `curve_linear_path` (straight segments), and `curve_step_path` (orthogonal
    right-angle steps) in `src/svg/builder.rs` build the path. Any other
    upstream curve name (`cardinal`, `natural`, …) falls back to basis.
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
  `rgba(0,0,0,0.05)` when no color given). Block frames and rect bands are
  **sized to the participants they enclose**, not the whole diagram (#123):
  `draw_block_frames`/`draw_rect_bands` compute `min_x`/`max_x` from the
  participant ids referenced by the events between each open/close pair
  (`collect_ids`/`extents`, falling back to `all_extents` when the block
  encloses no message). The frame-label tab fill is theme-driven
  (`theme.frame_label_fill`) instead of a hardcoded `#EEE`.
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
  loop are flushed down to `lifeline_bottom`. The band fill/stroke are
  theme-driven (`theme.activation_fill`/`activation_stroke`).
  - The `->>+`/`-->>-` **activation shorthand** is handled in the parser
    (`parse_message` in `src/parse/sequence/message.rs`): a leading `+`/`-` on the
    target id is stripped (not part of the participant name) and
    `parse_line_to_items` synthesizes the paired event — `+` appends
    `Activate(target)` *after* the message, `-` prepends `Deactivate(target)`
    *before* it, matching upstream ordering.
- Sequence `actor X` (vs `participant X`) renders as a **stick figure** (circle
  head + body/arms/legs, name below) instead of the rounded rect — `draw_actor`
  in `src/svg/sequence/participants.rs` branches on `Participant.kind`.
- Sequence `note` boxes are theme-driven (`theme.note_fill`/`note_stroke`, no
  longer a hardcoded `#FFF5AD`) and **word-wrap** to their box width (#123):
  `note_geometry` (`src/svg/sequence/messages.rs`) computes the box (an `over`
  note spans its participants with a `NOTE_MIN_W` floor; `left/right of` keep
  `NOTE_SIDE_W`), wraps the text to the interior via `wrap_note_text` (honoring
  existing `<br>`/`\n` breaks first), and grows the box height with the line
  count. The layout pass reserves that computed height, so a multi-line note
  pushes later events down.
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
  Parallel regions (`--`) are disconnected components that the shared sugiyama
  layout would otherwise interleave, so `stack_regions` (`svg/state/composite.rs`)
  post-processes `pos`: it left-aligns each region and translates it into its own
  vertical band below the previous one, returning the y of each dashed divider
  (`stroke-dasharray="3 3"`) drawn between adjacent regions in `draw_composites`.
  Every node in a region shares one stacking offset, so `render` shifts each
  routed edge's polyline by its from-node's `node_offset` to keep edges tracking
  the moved states.
- State aliasing `state "description" as X` binds `X`'s display label to the
  quoted text (`parse_quoted_as` in `parse_state_decl`), so the id stays clean
  and a later transition referencing `X` reuses the same state — no phantom box
  named literally `"…" as X`. The composite header `state "label" as X {` reuses
  the same `parse_quoted_as`, so the cluster id is `X` (labelled with the quoted
  text) instead of the raw text becoming the id.
- A **bare state-id** on its own line (`s1`) declares that state (upstream's
  `statement: idStatement`); `is_bare_id` gates it to a single identifier token
  so a genuinely unknown multi-word statement still hard-errors, and a bare id
  inside a composite body joins the active region.
- State stereotypes: `parse_stereotype` (`parse/state/decl.rs`) maps both the
  `<<choice/fork/join/history>>` form and the `[[fork]]`/`[[join]]`/`[[choice]]`
  bracket alternates (upstream lexes the brackets as exact aliases) onto
  `StateKind`, so `state f [[fork]]` yields a real fork instead of a garbage
  `f [[fork]]` state.
- State `click X href "url"` / `click X call fn()` binds a `State.click`
  (`ClickAction`, reusing the flowchart `parse_click` — now `pub(crate)` in
  `parse/flowchart/click.rs`); the renderer wraps the state in the shared
  `open_click`/`close_click` (`svg/interact.rs`) `<a>`/`<g onclick>`. Parsing the
  `click` line before the `X : text` description branch keeps a URL's `://` colon
  from misrouting into a phantom state. `hide empty description` is consumed as a
  no-op (the static renderer always draws the id).
- State history pseudo-states parse to `StateKind::History { deep }`:
  `<<history>>` and `[H]` are shallow (`deep: false`), `[H*]` is deep. The
  bracket forms are handled in `canonicalize` like `[*]` (unique `__hist_N`
  id per occurrence); the stereotype form in `parse_state_decl`. The renderer
  draws a small circle with `H`/`H*` inside.
- State `note right of X: text` (one-liner) and `note left of X\n…\nend note`
  (multi-line) both land in `notes`.
- Class `namespace X { class A; class B }` is stored in `namespaces`; the
  renderer draws a dashed rect around the members. `namespace Name["label"]`
  splits into a clean id + display `Namespace.label` (via `extract_class_label`,
  like `class Name["label"]`), the renderer showing the label. Nested
  namespaces work: each class is registered with **every** namespace on the
  stack, so an outer frame's bounds enclose the inner one's classes;
  `Namespace.depth` (0 = outermost) makes the renderer draw shallower frames
  with more padding so the outer visibly wraps the inner.
- Class **two-way relations** (`relationType lineType relationType`, e.g.
  `<|--|>`, `*--*`, `o--o`, `<-->`, `<..>`) glue a mirror marker onto the base
  token; `detect_two_way` (`src/parse/class/relation.rs`) consumes that trailing
  `|>`/`>`/`*`/`o` (only when the base is left-decorated/reversed) so it can't
  leak into the right class name, and fills `ClassRelation.to_kind`. `kind`
  marks the `from` end (reversed), `to_kind` the `to` end; the renderer draws
  its marker as `marker-end`.
- Class **one-line body** `class Duck { +swim() }` opens and closes on the same
  line: `handle_class_decl` parses the inline members (shared `add_member_line`
  helper) and keeps the block **closed** instead of leaving `in_block` set — so
  the block no longer swallows every following statement. An empty `{}` closes
  with no members.
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
- Class lollipop-interface `()` (`bar ()-- foo` / `foo --() bar`) is stripped
  off the token-adjacent side in `src/parse/class/relation.rs`
  (`split_trailing_lollipop`/`split_leading_lollipop`, before the multiplicity),
  keeping the class names clean and setting `ClassRelation.lollipop_from`/
  `lollipop_to`. The renderer overrides that end's marker with a hollow socket
  circle (`cls-lollipop`), so `()--|>` still draws the inheritance triangle at
  the far end plus the socket at the interface end.
- Class generics `~T~` are converted to angle brackets at render time
  (`convert_generics` in `src/svg/class/members.rs`) for class names and member/return
  types — `List~int~` → `List<int>`, nested `List~List~int~~` →
  `List<List<int>>`, `Map~string, int~` → `Map<string, int>` (innermost pair
  first; a lone unmatched `~` is left alone). The same `member_display` pass
  strips the trailing UML classifier (`*` abstract → `font-style="italic"`,
  `$` static → `text-decoration="underline"`).
- Class notes/annotations/labels/interactivity (`src/parse/class/`):
  `note "text"` (free) and `note for <Class> "text"` (attached) fill
  `ClassDiagram.notes` (`ClassNote { target, text }`); the renderer draws them
  as yellow sticky boxes in a row below the diagram, with a dashed connector to
  the target class. Standalone annotations parse in **either** order —
  `<<interface>> Shape` and `Shape <<interface>>` — via
  `parse_standalone_annotation`. A `class Name["label"]` sets `UmlClass.label`
  (the display text), keeping `name` clean — no phantom duplicate box.
  `click`/`link`/`callback` lines bind a `UmlClass.click` (reusing the flowchart
  `ClickAction`), parsed before the `:`-shorthand split so a URL's `https://`
  colon can't misroute the line. The **keyword drives the shape** (`split_interaction`
  keeps it): `callback` **always** binds a JS callback (a quoted first arg is the
  function name, never a URL), `link` always a hyperlink, `click` decides by the
  argument (`href`/`call`/quoted-URL/bare-name). The shared `open_click`/
  `close_click` wrappers live in `src/svg/interact.rs` (used by both the
  flowchart and class renderers).
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
- ER styling: `classDef <name> <props>` fills `ErDiagram.class_defs`, `class
  <ids> <name>` fills `Entity.classes`, and `style <id> <props>` fills
  `Entity.style` (`entity_index` materializes a placeholder for a
  forward-referenced id, like the flowchart). The renderer resolves them
  through the shared `resolve_style` — the entity box fill/stroke, header text
  color, and separator/attribute stroke follow the class; unstyled entities
  stay byte-identical to the theme defaults.
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
  Config keywords `includes …`, `inclusiveEndDates`, `topAxis` are consumed
  in `parse()` (informational only) so they don't fall through to the task path.
- Gantt `weekend friday|saturday`, `weekday <day>`, `tickInterval Nday|Nweek|Nmonth`
  and `displayMode[:] compact` are parsed via `strip_kw` (space- or colon-separated)
  into `GanttDiagram.{weekend,weekday,tick_interval,display_mode}` — previously
  `weekend`/`displayMode` hard-errored and `weekday`/`tickInterval` were dropped.
  Honored in `src/svg/gantt.rs`: `weekend` shifts the `excludes weekends` day pair
  (`weekend_days_for` in `gantt_date.rs`: `friday` → Fri+Sat, else Sat+Sun),
  `tickInterval` overrides `pick_tick_step` (`parse_tick_interval` → days), and
  `weekday` offsets the first axis tick onto that weekday (`weekday_tick_offset`).
  `display_mode` is stored but the compact row-packing layout is a follow-up.
- Asymmetric flowchart shapes are fully supported: parallelogram `[/text/]`,
  parallelogram-alt `[\text\]`, trapezoid `[/text\]`, trapezoid-alt
  `[\text/]`, and the asymmetric flag `>text]` — parsed in
  `src/parse/flowchart/node.rs` and rendered in `src/svg/flowchart/nodes.rs`.
- Flowchart v11 attribute syntax `id@{ shape: …, label: … }` is handled in
  `parse_at_node` (`src/parse/flowchart/node.rs`): the `@{…}` block right after a
  node id is split into `key: value` pairs (quote-aware comma/colon split), the
  `shape` name mapped onto a `NodeShape` by `shape_from_name` (aliases like
  `rounded`/`diam`/`cyl`/`lean-r`/`trap-b`/`dbl-circ`/`subproc`), and
  `label`/`title` set the node text. `icon`/`img` forms are dropped but their
  `label` is preserved so content is never lost. Beyond the classic geometries,
  ~19 v11 shapes have their own `NodeShape` variant and are drawn in
  `src/svg/flowchart/shapes.rs` (kept out of `nodes.rs`; `draw_node` delegates
  its non-classic arm there): `notch-rect`/`card`, `doc`, `docs`, `tag-doc`,
  `bolt`, `hourglass`, `comment`/`braces`, `delay`, `das` (horizontal cylinder),
  `lin-cyl`/`disk`, `lin-rect`, `div-rect`, `win-pane`, `tri`, `flip-tri`,
  `f-circ`, `cross-circ`, `paper-tape`, `bow-rect`/`stored-data` (+ their
  aliases). The round ones (`f-circ`/`cross-circ`) get a circle edge-clip; every
  other new shape uses the rect-boundary clip. Names still without a variant
  (e.g. `text`, `fork`, `sm-circ`) fall back to Rect.
- Label line breaks: `split_label_lines()` in `src/svg/builder.rs` splits any
  label on `<br>`/`<br/>`/`<br />` (case-insensitive) and `\n` (real newline or
  the two-char literal escape). `SvgBuilder::text()` auto-emits stacked
  `<tspan>`s for multi-line labels, so every renderer honors `<br>` for free;
  flowchart also sizes nodes from the resulting line count / widest line.
- Text width scales with the font size: `src/svg/metrics.rs` owns the shared
  `text_width(s, base_char_w, font_size)` / `font_scale(font_size)` helpers
  (`= font_size / BASE_FONT_SIZE`, `BASE_FONT_SIZE = 14`). Every renderer keeps
  its own per-glyph `base_char_w` (flowchart/er/class/state `7.5`, sequence
  actor `8.0`, ER bold PK/FK `8.0`, mindmap `7.0`, requirement label `5.5`,
  edge labels `7.0`) but routes the estimate through `text_width` so node/label
  boxes grow with `--font-size` instead of overflowing (#122). `SvgBuilder::text`
  and timeline scale the `LABEL_LINE_H` line spacing by `font_scale` the same
  way. Because `font_scale(14) == 1`, every default-theme render — and the whole
  gallery — is byte-identical; only a non-default font size changes.
- C4 supports the full `{System,Container,Component} × {Db,Queue} × {_Ext}`
  element matrix; the `_Ext` variants reuse the same shape with the gray
  external palette. `UpdateElementStyle` / `UpdateRelStyle` /
  `UpdateLayoutConfig` are stored on `C4Diagram` (`element_styles`,
  `rel_styles`, `layout`) and applied at draw time: element `$bgColor`/
  `$fontColor`/`$borderColor`, rel `$textColor`/`$lineColor`/`$offsetX/Y`.
  `$c4ShapeInRow`/`$c4BoundaryInRow` override the row-flow wrap counts
  (`flow_layout`'s `shape_in_row`/`boundary_in_row`). `C4Relation.direction`
  (`Rel_U/D/L/R`) is parsed but not used by the row-flow layout. `Rel_Back`
  reverses the arrow (from/to swapped at parse time so the head lands on
  `from`); `RelIndex(index, from, to, …)` is the C4Dynamic step form — the
  leading index shifts every positional slot by one and is prepended to the
  label (`"{index}: {label}"`). Both were previously swallowed by the tolerant
  unknown-line arm.
  C4Deployment's `Node(...)`/`Node_L(...)`/`Node_R(...)` boundary openers alias
  `Deployment_Node` in `parse_boundary_open` (checked `Node_L`/`Node_R` before
  the bare `Node` prefix), so their children nest instead of leaking to top
  level. A boundary/`Deployment_Node`'s optional third arg (`type`) lands on
  `C4Element.boundary_type` and, when present, overrides the fixed per-kind
  `[label]` header tag (`boundary_tag_text` in `src/svg/c4/mod.rs`) — e.g.
  `Deployment_Node(n, "Web Server", "Ubuntu 16.04 LTS")` shows `[Ubuntu 16.04 LTS]`.
  - C4 element/relation macros are **not** positional-only: `split_macro_args`
    (`src/parse/c4/calls.rs`) pulls the `$descr`/`$techn`/`$sprite`/`$tags`/
    `$link` keyword args out of the arg list before slotting the positional
    ones, so a `$sprite=` placed before the positional technology no longer
    shifts every later field. `$descr`/`$techn` override their positional slot;
    `$sprite`/`$tags`/`$link` land on `C4Element`/`C4Relation` (rendering
    deferred). `UpdateBoundaryStyle(alias, $bgColor/$fontColor/$borderColor)`
    fills `C4Diagram.boundary_styles` (reusing `C4ElementStyle` via
    `parse_style_directive`) and restyles the boundary frame fill/stroke/label
    in `draw_boundary_rect`. `SHOW_LEGEND()` sets `C4Diagram.show_legend`
    (consumed; legend rendering is a follow-up).
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
  newer commits sit higher. **Statement keywords match on a word boundary**
  (`keyword()`: the keyword must end the line or be followed by whitespace), so
  `commitxyz`/`branches foo` hard-error instead of masquerading as
  `commit`/`branch`. Branch names are **unquoted everywhere** a `(REFERENCE |
  STRING)` is allowed — `branch`/`checkout`/`switch`/`merge` all route the name
  through `take_value`, so `branch "feat x"` + `checkout "feat x"` +
  `merge "feat x"` reference one lane.
- gitGraph **config directives** (`config.gitGraph.*`, from `%%{init}%%` or
  frontmatter `config:`) flow through the preamble: `preamble.rs` fills a
  `DiagramMeta.git_graph` (`GitGraphMeta`, all-`Option`), and
  `parse_with_meta` overlays them onto `GitGraphDiagram.config`
  (`GitGraphConfig`, whose `Default` keeps upstream's own defaults). Honored:
  `mainBranchName` (initial/default branch — the renderer no longer hardcodes
  `"main"`), `showBranches` (branch labels + lane lines), `showCommitLabel`
  (per-commit id label), `rotateCommitLabel` (rotates the id label -45° in the
  horizontal layout), `parallelCommits` (`assign_col`: a commit's column is one
  past its deepest parent so independent branches can share a column, instead of
  a strictly advancing global counter), and `mainBranchOrder`
  (`GitGraphConfig.main_branch_order`, seeded into `branch_orders[0]` so main
  sorts among the `order:`-ed lanes instead of being pinned to lane 0).
- gitGraph `merge` and `cherry-pick` render with **dedicated glyphs** (no longer
  reusing `Highlight`/`Reverse`): `CommitKind::Merge` → double concentric circle
  (`draw_merge_glyph`), `CommitKind::CherryPick` → the two-cherry glyph
  (`draw_cherry_pick_glyph`). `cherry-pick id:"x" parent:"y" tag:"t"` keeps its
  `parent`/`tag` on `GitEvent::CherryPick` (`parse_cherry_pick_attrs`); the tag
  renders as the node label. `commit`/`merge` share `parse_commit_attrs(s,
  default_kind)`: `tag:` **accumulates into a `Vec`** (`GitEvent::{Commit,
  Merge}.tags`, upstream `tags+=STRING`; the renderer stacks them upward), and a
  `merge <branch> type: NORMAL|REVERSE|HIGHLIGHT` overrides the merge glyph via
  `GitEvent::Merge.kind` (default `CommitKind::Merge`).
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
- block-beta styling & edges (`src/parse/block/` / `src/svg/block/`):
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
  - Shape delimiters (`parse_shape`, via the `strip_pair` helper): beyond the
    classic set, `[[..]]`→`Subroutine`, `(((..)))`→`DoubleCircle`, `>..]`→`Odd`
    (asymmetric flag), and the parallelogram/trapezoid family `[/../]`→
    `LeanRight`, `[\..\]`→`LeanLeft`, `[/..\]`→`Trapezoid`, `[\../]`→
    `TrapezoidAlt`. Longest openers are matched first so `[[`/`(((` win over `[`/
    `((`. The node tokenizer (`parse_block_line`) floors bracket depth at 0 so
    the unmatched `]` of a `>text]` shape can't glue the rest of the line into
    one token.
  - Links carry a `BlockLinkStyle` (`Solid`/`Dotted`/`Thick`/`Invisible`):
    `parse_edge` matches `~~~`/`-.->`/`-.-`/`==>`/`===`/`-->`/`---` longest-first
    and reads an inline `-- / -. / ==` label opener off the tail. The renderer
    draws dotted (`stroke-dasharray`) / thick (wider stroke) styles and skips
    drawing an invisible link (which still shapes layout).
  - `columns auto` (no longer a hard-error) packs every direct cell into one row
    — `auto_column_count` sums block/group spans + space counts. `space` is a
    keyword only as `space`/`space:N`, so ids like `spaceship` survive.
    `style a,b <props>` takes a comma id-list like `class`.
- mindmap `:::class1 class2` and `::icon(fa fa-book)` are **attachment lines**,
  not child nodes (`src/parse/mindmap.rs`): both attach to the most-recent node
  (`stack.last_mut()` / `root`), `:::` filling `MindmapNode.classes` and `::icon`
  filling `MindmapNode.icon`. The renderer (`src/svg/mindmap.rs`) never prints the
  raw Font Awesome class string — `draw_mindmap_icon` maps `icon_name()` (the last
  `fa-`-prefixed token) onto a small builtin glyph set (book/star/clock/user/cog/
  cloud/database/check/heart), unknown names falling back to a generic tag glyph.
  The layout (`src/svg/mindmap.rs`) is **two-sided**: first-level branches are
  dealt alternately onto the right and left of a centred root (`layout` builds a
  canonical right-growing subtree, `mirror` reflects the left ones about the root
  centre and flags them `dir = -1`), so the map fans out on both sides instead of
  only rightward. `draw_edges` picks the parent's right or left edge by the
  child's `dir`; `bounds` frames both halves (plus icon glyphs) into positive
  space. A top-level `classDef <name> <props>` line fills
  `MindmapDiagram.class_defs`; `draw_nodes` resolves each node's `:::` classes
  through the shared `resolve_style`, overriding the node fill/stroke and label
  color (unstyled nodes stay byte-identical).
- zenuml (`src/parse/zenuml/`: `mod.rs` header/tokenize/dispatch + declarations,
  `message.rs` calls/returns/assignment, `blocks.rs` if/try chains) is a
  **brace-structured** translation to a
  `SequenceDiagram` (reuses the sequence renderer). After the `zenuml` header the
  body is `tokenize`d into `{`/`}`/statement `Tok`s (braces inside `(…)`/quotes
  stay literal; `\n`/`;` end statements; `//` and `%%` are comments), then a
  recursive `Parser::parse_items(ctx, ret)` walks them. `ctx` is the current
  caller (the enclosing method's *receiver*, or the top-level starter); `ret` is
  who a `return` replies to.
  - Annotators: `@Actor X` declares an actor; `@Boundary`/`@Control`/`@Entity`/
    `@Database X` declare the matching UML stereotype (each drawn with its own
    glyph by `draw_stereotype` in `src/svg/sequence/glyphs.rs` — boundary
    circle-with-bar, control arrow-circle, entity underlined circle, database
    cylinder); any other `@Type X` is a plain participant. `@Starter(X)` sets the
    top-level caller. A bare/`A.method()` call with no explicit `A -> B` source
    originates from the starter — a synthetic `Starter` lane, created lazily,
    when none is declared.
  - Participant declarations (`try_declaration` in `mod.rs`): a bare identifier
    `Bob` declares the participant, and `A as Alice` is an alias (id `A`,
    displayed `Alice` — `split_alias`, quoted display allowed). Declaration
    order is column order. A statement carrying `(` or `->` is never a
    declaration (it stays a call), so these no longer fall through to
    `parse_call` and materialize as phantom Starter self-messages.
  - `new A1` / `new A2(with, parameters)` (`parse_new` in `message.rs`)
    materialize the participant and emit a `SequenceItem::Create` plus a
    `«create»` creation message from the current context — no longer a Starter
    self-call.
  - Method calls carry a context: `Recv.method()` → `ctx -> Recv`, `method()`
    (no dot) is a self-call on `ctx`. A `{ … }` body after a call runs in the
    receiver's context and, on close, draws an implicit dashed **return** to the
    caller; an `x = call()` assignment draws a dashed return labeled `x`
    (self-calls get no return arrow). A **typed** assignment `SomeType a = A.m()`
    (`split_assignment` accepts a multi-word identifier LHS) labels the return
    with the trailing variable (`a`), not a participant named `SomeType a = A`.
  - `return <v>` (and the `@return`/`@reply <v>` annotation aliases) emits a
    dashed reply from `ctx` to `ret`; a caller-less bare-value `return` (no
    enclosing method-call body) is a `ParseError::Syntax`, not silently dropped.
    The explicit directed form `return A -> B: result` / `@return A -> B: result`
    (upstream reply form 3) emits a dashed `A`→`B` message and is valid at top
    level (no enclosing caller needed). Control structures map onto existing
    `SequenceItem` frames: `if/else if/else` → `Alt`,
    `while/for/forEach/foreach/loop` → `Loop`, `opt` → `Opt`, `par` → `Par`,
    `try/catch/finally` → `Critical` (catch/finally as option branches). The
    `else`/`catch`/`finally` chain tokens are consumed by their opener's handler.
- architecture-beta icons: `draw_arch_icon` (`src/svg/architecture.rs`) draws
  five built-in glyphs (`cloud`, `database`/`db`/`disk`, `server`,
  `internet`/`globe`, `queue`/`kafka`) and returns `false` for anything else. A
  static renderer can't fetch Iconify packs (`logos:aws-lambda`, `mdi:…`), so an
  unrecognized name falls back to the generic box **plus** the name as a caption
  (`truncate_icon_name`: segment after the last `:`, capped at 16 chars) — the
  icon identity is shown, not silently lost. `ArchEdge` has no `label` field:
  upstream architecture-beta has no edge-label syntax, so it was dropped as dead
  weight.
- timeline header accepts a v11.14+ direction token — `timeline LR`/`timeline TD`
  (also `TB`/`BT`/`RL`) parse into `TimelineDiagram.direction` (`parse_header` in
  `src/parse/timeline.rs`), validated against the known set (unknown tokens still
  hard-error). The horizontal renderer treats it as a no-op. A **sectionless**
  timeline advances its color per time-period (upstream `isWithoutSections`)
  instead of one flat fill: `src/svg/timeline.rs` picks `pie_color(period idx)`
  when no section is named, `pie_color(section idx)` otherwise.
  `config.timeline.disableMulticolor` (frontmatter/`%%{init}%%` →
  `DiagramMeta.timeline_disable_multicolor` → `TimelineDiagram.disable_multicolor`)
  forces the old flat single color back on.
