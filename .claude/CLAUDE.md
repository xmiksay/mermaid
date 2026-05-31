# mermaid-svg

Single-crate Rust library that renders [Mermaid](https://mermaid.js.org/)
diagrams to SVG. No Node.js, no JVM, no native binaries.

## Layout

```
src/
├── lib.rs           public API: render(), parse(), Diagram, ast::*, errors
├── parse/           Mermaid source → Diagram AST  (line-oriented scanners)
│   ├── mod.rs       parse() dispatcher, ParseError, ast re-export
│   ├── ast.rs       all AST types (pub via lib.rs as `ast::*`)
│   └── {pie,sequence,flowchart,state,class,er,gantt}.rs
├── svg/             Diagram AST → SVG string
│   ├── mod.rs       render() / render_diagram() dispatcher, RenderError
│   ├── builder.rs   string-based SVG writer (escape, fnum, SvgBuilder)
│   ├── theme.rs     default colors
│   └── {pie,sequence,flowchart,state,class,er,gantt}.rs
├── sugiyama/        layered graph layout (private — no public API surface)
│   ├── mod.rs       Graph/Layout/LayoutConfig/LayoutError + layout_with()
│   ├── tests.rs     unit tests (private API)
│   └── {cycle,layer,order,coord,route,work}.rs
examples/render_user.rs   throwaway one-off; safe to delete
tests/integration.rs      end-to-end tests; writes samples to target/test-samples/
```

Cargo manifest: single `[package]` (not `[workspace]`). Crate is published to
crates.io as `mermaid-svg`. The historical 3-crate split (`sugiyama` +
`mermaid-parse` + `mermaid-svg`) was consolidated — see `plan.md` for the
original design.

## Done

| Feature | Status |
|---|---|
| sugiyama layout (cycle/layer/order/coord/route) | done |
| pie · sequence · flowchart · state · class · ER · gantt parsers | done |
| Matching SVG renderers | done |
| HTTP server (kroki-style POST /svg) | not started |
| PNG via resvg | not started |
| Themes & config | not started (default theme only) |

## Build & test

```bash
cargo build              # library + example
cargo test               # unit + integration + doctest (113 tests total)
cargo run --example render_user
cargo package --allow-dirty   # publish dry-run; produces .crate
```

Integration tests write sample SVGs to `target/test-samples/`:
- `pie_browsers.svg`, `sequence_api.svg`
- `flowchart_td.svg`, `flowchart_lr.svg`
- `state_lifecycle.svg`, `class_uml.svg`
- `er_customer.svg`, `gantt_release.svg`

## Known deviations from `plan.md`

Deliberate v0.1 simplifications. API does not change if these are revisited.

1. **sugiyama X-coords**: `coord.rs` uses median-relaxation with hard
   non-overlap constraints instead of full Brandes-Köpf (1.5 in the plan).
   Layouts are valid but not as balanced as 4-pass BK.
2. **mermaid parser**: hand-rolled line scanners instead of `pest` PEG (2.x in
   the plan). Mermaid syntax is line-oriented, scanner code is shorter and
   easier to extend. AST is unchanged.
3. **svg writer**: own string SVG builder instead of `quick-xml` (3.1 in the
   plan). No runtime parsing, write-only, escaping in `svg::builder::escape`.

## Conventions

- No extra comments — only where the *why* is non-obvious from the code.
- No `#[allow(dead_code)]` in library code.
- Tests: unit tests in `#[cfg(test)] mod tests` at the end of each file;
  end-to-end tests in `tests/integration.rs`; private-API sugiyama tests in
  `src/sugiyama/tests.rs`.
- Errors via `thiserror`. No stringly-typed errors.
- `NodeId = u32` in sugiyama; upper layers map their own `String → u32`.

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
