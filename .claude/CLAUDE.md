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
│   ├── mod.rs       parse() dispatcher, ParseError, ast re-export
│   ├── ast.rs       all AST types (pub via lib.rs as `ast::*`)
│   └── {pie,sequence,flowchart,state,class,er,gantt,
│        journey,timeline,sankey,quadrant,xychart,radar,packet,mindmap,
│        gitgraph,requirement,c4,block,architecture,kanban,treemap,zenuml}.rs
├── svg/             Diagram AST → SVG string
│   ├── mod.rs       render*/render_diagram* dispatchers, RenderError, pub Theme
│   ├── builder.rs   string-based SVG writer (escape, fnum, SvgBuilder)
│   ├── theme.rs     Theme struct + default/dark/forest/neutral
│   └── {pie,sequence,flowchart,state,class,er,gantt,
│        journey,timeline,sankey,quadrant,xychart,radar,packet,mindmap,
│        gitgraph,requirement,c4,block,architecture,kanban,treemap}.rs
├── sugiyama/        layered graph layout (private)
│   ├── mod.rs       Graph/Layout/LayoutConfig/LayoutError + layout_with()
│   ├── tests.rs
│   └── {cycle,layer,order,coord,route,work}.rs
examples/render_user.rs   small one-shot example
tests/integration.rs      end-to-end tests; writes samples to target/test-samples/
```

Cargo manifest: single `[package]`. Crate is published to crates.io as
`mermaid-svg`.

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

## Build & test

```bash
cargo build              # library + binary
cargo test               # unit + integration + doctest (178 tests)
cargo run --bin mermaid-svg -- --help
cargo bench              # criterion benches: parse + render per diagram
cargo package --allow-dirty
```

Bench layout: `benches/render.rs` drives criterion; one `.mmd` per diagram
type lives in `benches/samples/`. Two groups: `parse/<kind>` (parse only)
and `render/<kind>` (parse + render to SVG). Sized inputs use realistic
non-trivial examples (typically 10-30 lines).

Integration tests write sample SVGs to `target/test-samples/`:
- `pie_browsers.svg`, `sequence_api.svg`
- `flowchart_td.svg`, `flowchart_lr.svg`
- `state_lifecycle.svg`, `class_uml.svg`
- `er_customer.svg`, `gantt_release.svg`
- `journey_day.svg`, `timeline_history.svg`
- `sankey_energy.svg`, `quadrant_campaigns.svg`
- `xychart_sales.svg`, `radar_skills.svg`, `packet_tcp.svg`
- `mindmap_tree.svg`, `gitgraph_branches.svg`
- `requirement_test.svg`, `c4_context.svg`
- `block_grid.svg`, `architecture_api.svg`
- `kanban_board.svg`, `treemap_drinks.svg`, `zenuml_auth.svg`

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

- Sugiyama waypoints include **endpoints** (center of src, center of dst).
  The SVG renderer clips them to the node boundary itself.
- Flowchart `FlowEdge` has separate `line` (Solid/Dotted/Thick) and `head`
  (None/Arrow/Circle/Cross) — covers `-->`, `---`, `-.->`, `==>`, `--o`,
  `--x` plus all no-head variants.
- `A & B --> C & D` produces 4 edges (cross product) — multi-source/target.
- Flowchart `subgraph` is tracked in `FlowchartDiagram.subgraphs` including
  nesting. The renderer draws a dashed bounding rect around the group.
- Sequence parser has **nested items** (`Vec<SequenceItem>`) — `Alt`/`Par`/
  `Critical` blocks have branches; `Loop`/`Opt` have label + items. Renderer
  draws labeled frames with tab labels.
- Sequence `autonumber` prefixes each message text with a sequence number.
- Sequence `activate`/`deactivate` is paired and drawn as an activation band
  on the lifeline.
- State `state X { ... }` is stored in `composites`; parallel regions are
  separated by `--`. Renderer draws a dashed rounded outline with a label.
- State `note right of X: text` (one-liner) and `note left of X\n…\nend note`
  (multi-line) both land in `notes`.
- Class `namespace X { class A; class B }` is stored in `namespaces`; the
  renderer draws a dashed rect around the members.
- Class `direction` (TD/BT/LR/RL) drives the transpose the same way the
  flowchart does.
- ER `EntityAttribute.comment` is populated from a quoted string after the
  attribute (`string name "the customer name"`).
- Gantt `excludes` (weekends) and `todayMarker YYYY-MM-DD` are in the AST;
  the renderer draws the today marker as a vertical red line.

## Known parser limitation still listed

- Asymmetric flowchart shapes `[/text/]` and `[\text\]` are not supported.
  Tests use the regular `[text]` form.
