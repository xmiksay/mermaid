# mermaid-svg

Single-crate Rust library that renders [Mermaid](https://mermaid.js.org/)
diagrams to SVG. No Node.js, no JVM, no native binaries. Ships a `mermaid-svg`
binary alongside the library. Published to crates.io as `mermaid-svg` (single
`[package]` manifest).

## Architecture overview

Pipeline: `parse_with_meta` strips the cross-cutting preamble (frontmatter,
`%%{init}%%`, accTitle/accDescr → `DiagramMeta` with a flattened dotted
`config` map) and dispatches to a per-diagram line-oriented parser in
`src/parse/` producing the `ast::*` types; `render_body` matches the `Diagram`
onto a per-diagram renderer in `src/svg/` (all writing through the string-based
`SvgBuilder`); `decorate::apply` finishes with role/aria + `<title>`/`<desc>`.
Graph-shaped diagrams (flowchart, state, class, ER, requirement) share the
private `src/sugiyama/` layered layout. Theming: every renderer takes
`&Theme`; `theme_from_meta` layers preamble theme → `themeVariables` →
`fontFamily`/`fontSize` → `useMaxWidth` onto the caller's theme. Output is
responsive (`width="100%"`, `viewBox`, no fixed root height).

Extension seams:
- **New diagram kind** = parser module + `Diagram` variant + renderer +
  `SAMPLES` entry in `gallery_build.rs` + the matching `#![doc]` include in
  `src/lib.rs`. Public `ast::*` enums are `#[non_exhaustive]`.
- **New theme color** = `Theme` field + every built-in constructor in
  `src/svg/theme.rs` (`base` inherits via struct-update for free).
- **Closing a per-diagram config gap** = a `meta.config` lookup in
  `parse_with_meta`, not new scanning.

> Deep reference — module map, gallery pipeline, theme internals (`Cow`
> contract, `themeVariables`), and cross-cutting parse/render behavior — lives
> in [docs/architecture.md](../docs/architecture.md); per-diagram behavior
> notes live in one file per diagram kind under
> [docs/architecture/](../docs/architecture/) (e.g. `flowchart.md`,
> `sequence.md`). Read the matching file before touching a parser or renderer,
> and keep it current in the same change.

## Done

| Feature | Status |
|---|---|
| sugiyama layout (cycle/layer/order/coord/route) | done |
| pie · sequence · flowchart · state · class · ER · gantt parsers | done |
| journey · timeline · sankey · quadrant · xychart · radar · packet parsers | done |
| mindmap · gitGraph · requirement · C4 · block · architecture · kanban · treemap · zenuml parsers | done |
| Matching SVG renderers (zenuml reuses sequence renderer) | done |
| Themes (default, base, dark, forest, neutral + user-defined) | done |
| CLI binary (`mermaid-svg`) | done |
| Cross-cutting preamble (frontmatter title/theme, `%%{init}%%`, accTitle/accDescr) | done |
| Responsive SVG output + `role`/`aria`/`<title>`/`<desc>` accessibility | done |
| `#…;` entity codes + markdown-string emphasis in labels | done |
| Inline HTML labels (`<b>`/`<i>`/`<u>`/`<span style=color>`/`<a href>`) | done |

## Build & test

Drive everything through the Makefile (`make help` lists all targets):

```bash
make build              # debug build: library + binary
make test               # all tests (706: 690 lib + 15 integration + 1 doctest)
make test-unit          # in-module #[cfg(test)] only
make test-integration   # tests/integration.rs → target/test-samples/<stem>.svg
make lint               # cargo fmt --check + clippy --all-targets -D warnings
make verify             # lint + test — the hard gate before any push
make bench              # criterion benches: parse/<kind> + render/<kind>
make gallery            # regenerate assets/gallery/*.md
make run ARGS="--help"  # run the CLI
make package            # cargo package --allow-dirty
```

`CARGO_BUILD_JOBS` is pinned to 4 in the Makefile (override per invocation).

Integration tests write one sample SVG per diagram kind to
`target/test-samples/<stem>.svg` (one stem per `SAMPLES` entry in
`gallery_build.rs`); the `doc_gallery_up_to_date` test names the stale stem if
a committed gallery file drifts from the samples.

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
- Parser unknown-line policy is **hard-error everywhere**: an unparseable
  statement is a `ParseError::Syntax { line }`, never silently dropped
  (deliberate tolerances are documented in `docs/architecture.md`).
- When adding new functionality, refresh the relevant docs in the same change:
  this file, `docs/architecture.md` and the touched diagram's file under
  `docs/architecture/`, `README.md`, and `Cargo.toml` (description/keywords).
- Always write tests for new functionality, and make sure the full suite
  (`make test`) passes before committing.
- Run `cargo fmt` before every commit, and keep `cargo clippy` clean — no
  warnings (treat them as errors before committing).
- **Hard gate: never push until `make verify` passes** (fmt-check + clippy
  `-D warnings` + the full test suite). No exceptions — a red `verify` means
  the branch stays local.
