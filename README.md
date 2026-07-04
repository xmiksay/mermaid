# mermaid-svg

[![Crates.io](https://img.shields.io/crates/v/mermaid-svg.svg)](https://crates.io/crates/mermaid-svg)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Pure-Rust [Mermaid](https://mermaid.js.org/) → SVG renderer. No Node.js,
no JVM, no native binaries — just `cargo add mermaid-svg`.

## Quick start

```rust
use mermaid_svg::render;

let svg = render(r#"
graph TD
    A[Start] --> B{Decision}
    B -->|Yes| C[Execute]
    B -->|No| D[End]
    C --> D
"#).unwrap();

std::fs::write("diagram.svg", svg).unwrap();
```

## CLI

The crate also installs a `mermaid-svg` binary:

```bash
cargo install mermaid-svg

mermaid-svg diagram.mmd diagram.svg               # files
mermaid-svg < diagram.mmd > diagram.svg           # stdin/stdout
mermaid-svg --theme dark diagram.mmd > out.svg    # pick a theme
mermaid-svg -f 'Inter, sans-serif' diagram.mmd    # override the font family
mermaid-svg --font-size 16 diagram.mmd            # override the base font size
echo 'pie\n"A":1\n"B":2' | mermaid-svg
```

Options: `-t/--theme <NAME>`, `-f/--font <FAMILY>`, `--font-size <PX>` (backed by
`Theme::with_font` / `Theme::with_font_size`).

Run `mermaid-svg --help` for the full usage.

## Themes

Five built-in themes: `default`, `base`, `dark`, `forest`, `neutral`.

```rust
use mermaid_svg::{render_with, Theme};

let svg = render_with("classDiagram\n  A <|-- B", &Theme::dark())?;
```

Custom themes: construct a [`Theme`] with the colors you want
(typically using struct-update syntax from a built-in):

```rust
let custom = Theme {
    flow_node_fill: "#fffbe6".into(),
    flow_node_stroke: "#caa400".into(),
    ..Theme::default_theme()
};
let svg = render_with(source, &custom)?;
```

The color/font fields are `Cow<'static, str>`, so the built-ins stay `const`
while overrides accept owned runtime strings (`"#fffbe6".into()`).

The built-in constructors are `Theme::default_theme()`, `Theme::base()`,
`Theme::dark()`, `Theme::forest()`, and `Theme::neutral()`;
`Theme::with_font(family)` and `Theme::with_font_size(px)` return a copy with
the font overridden. `Theme::by_name(name)` selects a built-in by string;
`base` is upstream's designated customization palette (warm `#fff4dd`
primary, visibly distinct from `default`, meant to be recolored via
`themeVariables`); the `mermaid-svg --theme` flag uses the same lookup.

## Supported diagrams

23 diagram types are supported:

| Type | Header keyword(s) | Notable features |
|---|---|---|
| Pie | `pie` | `showData`, title, entries (slices under 1% dropped) |
| Sequence | `sequenceDiagram` | participants/actors, `autonumber [start [step]]`/`off`, `activate`/`deactivate`, nested `alt`/`par`/`critical`/`loop`/`opt`/`break`, `rect <color>` bands, notes |
| Flowchart | `flowchart`, `graph` | directions TD/BT/LR/RL, all edge styles (`-->`, `---`, `-.->`, `==>`, `--o`, `--x` + no-head variants), invisible links (`~~~`), bidirectional edges (`<-->`, `o--o`, `x--x`), multi-source/target (`A & B --> C & D`), nested `subgraph`, `click` links/callbacks, v11 `A@{ shape: …, label: … }` node syntax, v11 edge ids + `e1@{ animate, curve }` attributes, `linkStyle … interpolate`, frontmatter `title` |
| State | `stateDiagram`, `stateDiagram-v2` | composite states with parallel regions (`--`), one-line and multi-line notes |
| Class | `classDiagram` | namespaces, `direction` directive, visibility (`+`/`-`/`#`/`~`), full relation set (`<\|--`, `*--`, `o--`, `-->`, `..>`, `<\|..`), notes, standalone annotations, `Name["label"]`, `click`/`link`/`callback` |
| ER | `erDiagram` | attribute keys (PK/FK/UK), Crow's Foot cardinality, attribute comments |
| Gantt | `gantt` | sections, task end as duration (`ms`/`s`/`m`/`h`/`d`/`w`/`M`/`y`)/end-date/`until <id>`, duration-only tasks, `after <id> [<id> …]` (multiple predecessors), `milestone`/`vert` markers, sub-day `dateFormat HH:mm`, `click <id> href/call`, `excludes` (weekends), `weekend friday`, `tickInterval`, `weekday`, `todayMarker` (CSS style / `off`) |
| Journey | `journey` | sections, tasks with scores and actors |
| Timeline | `timeline` | sections, time periods with multiple events |
| Sankey | `sankey-beta`, `sankey` | weighted source→target flows |
| Quadrant | `quadrantChart` | axes, labelled quadrants, plotted points |
| XY chart | `xychart-beta`, `xychart` | bar and line series, x/y axes |
| Radar | `radar-beta`, `radar` | multiple axes and curves, `min`/`max`/`ticks`/`graticule`/`showLegend` options |
| Packet | `packet-beta`, `packet` | byte/bit field ranges |
| Mindmap | `mindmap` | nested nodes, node shapes |
| Git graph | `gitGraph` | commits, branches, merges, checkouts |
| Requirement | `requirementDiagram` | requirements, elements, relationships, `direction`, `classDef`/`class`/`style` styling |
| C4 | `C4Context`, `C4Container`, `C4Component`, `C4Dynamic`, `C4Deployment` | people, systems (`Db`/`Queue`/`_Ext` variants), boundaries, relations, `Update*Style` overrides |
| Block | `block-beta`, `block` | grid layout, spanning blocks, edges |
| Architecture | `architecture-beta`, `architecture` | groups, services, junctions, edges |
| Kanban | `kanban` | columns and cards, `id[Label]` form, `@{…}` metadata (assigned/priority/ticket), priority-colored borders, hyperlinked tickets |
| Treemap | `treemap-beta`, `treemap` | squarified nested weighted rectangles, `config.treemap.valueFormat` value formatting (defaults to `,` thousands grouping), `config.treemap.showValues` toggle, `classDef` + `:::class` fill/stroke overrides |
| ZenUML | `zenuml` | annotators, method calls, nesting braces, `return`, if/while/opt/par/try; rendered via the sequence renderer |

### Cross-cutting features

These work on every diagram type:

- **YAML frontmatter** — a leading `--- … ---` block sets the diagram `title:`
  and a nested `config:` block.
- **`%%{init: {…}}%%` directives** — the same config, inline and anywhere.
- **Config** — the whole `config:` tree (frontmatter *and* `init`) is honored:
  `theme` (a built-in name — `default`, `base` (the warm-cream customization
  palette, distinct from `default`), `dark`, `forest`, `neutral`),
  `themeVariables` (upstream's `theme: base` recoloring path — the generic
  `primaryColor`/`lineColor`/`primaryBorderColor`/`noteBkgColor`/`titleColor`/
  `edgeLabelBackground`/`fontFamily`/`fontSize`, plus the documented per-diagram
  names: sequence `actorBkg`/`actorBorder`/`actorTextColor`/`actorLineColor`/
  `signalColor`/`signalTextColor`/`labelBoxBkgColor`/`activationBkgColor`, pie
  `pie{1..12}`/`pieStrokeColor`/`pieOpacity`/`pieTitleTextColor`, git
  `git{0..7}`/`commitLabelColor`/`tagLabelColor`, and the `cScale{0..11}`
  categorical scale), top-level
  `fontFamily`/`fontSize`, `useMaxWidth: false` (top-level *or* per diagram —
  `flowchart.useMaxWidth`, `sequence.useMaxWidth`, … — emit a fixed-size SVG
  instead of the responsive envelope), and `flowchart.curve` (the diagram-level
  default edge curve). A directive overrides frontmatter and the last
  `%%{init}%%` wins, matching upstream; an `%%{init}%%` block may span multiple
  lines. Every other key is still parsed into a flattened dotted map
  (`DiagramMeta::config`, e.g. `flowchart.htmlLabels`, `kanban.ticketBaseUrl`,
  `treemap.valueFormat`) for per-diagram consumers.
- **`accTitle:` / `accDescr:`** (and the `accDescr { … }` block form) — emitted
  as `<title>`/`<desc>` and linked with `aria-labelledby`/`aria-describedby`;
  the root `<svg>` always carries `role` + `aria-roledescription`.
- **Responsive output** — `width="100%"` + `style="max-width: Npx;"` + `viewBox`
  (no fixed height), so diagrams scale to their container (unless
  `config.useMaxWidth: false` pins a fixed pixel `width`/`height`).
- **Entity codes & markdown strings in labels** — `#quot;`, `#35;`, `#x2665;`
  etc. are decoded; backtick-fenced markdown emphasis (`**bold**`, `*italic*`)
  renders as styled `<tspan>`s.
- **Inline HTML in labels** (`htmlLabels`) — `<b>`/`<strong>`, `<i>`/`<em>`,
  `<u>`, `<span style="color:…">`, and `<a href>` style the label via `<tspan>`s
  and SVG links; unknown tags degrade to plain text.

Note: pie charts drop slices under 1% of the total, matching upstream.

### Styling (flowchart, class, state, quadrant, block, requirement, treemap)

Inline CSS-style overrides are supported and resolved into concrete SVG
attributes (`fill`, `stroke`, `stroke-width`, `stroke-dasharray`, text `color`,
`font-size`):

- **`style <id> fill:#f9f,stroke:#333,stroke-width:4px`** — style one node.
- **`classDef <name> …` + `class <id> <name>`** — define a reusable class and
  attach it (multiple classes layer in order).
- **`<id>:::<name>`** — shorthand class attachment inline on a node.
- **`linkStyle <n> stroke:#f00,…`** — style the n-th edge.
- **`linkStyle <n> interpolate linear|step|basis …`** — set the n-th edge's
  curve interpolation (also `linkStyle default interpolate …`).
- **`A e1@--> B` + `e1@{ animate: true, curve: step }`** — v11 edge id and
  attributes: `animate` draws a flowing dash animation, `curve` sets the
  per-edge interpolation.

Layering is `default` classDef → each named class in order → the node's inline
`style` (later layers win per property).

## API

```rust
pub fn render(input: &str) -> Result<String, RenderError>;
pub fn render_with(input: &str, theme: &Theme) -> Result<String, RenderError>;
pub fn render_diagram(d: &Diagram) -> Result<String, RenderError>;
pub fn render_diagram_with(d: &Diagram, theme: &Theme) -> Result<String, RenderError>;
pub fn parse(input: &str) -> Result<Diagram, ParseError>;
pub fn parse_with_meta(input: &str) -> Result<(Diagram, DiagramMeta), ParseError>;

pub use ast;  // all AST types: ArrowKind, FlowNode, ClassMember, DiagramMeta, ...
```

`ParseError::Syntax` carries a `kind: SyntaxKind` (`MissingHeader`,
`UnknownStatement`, `InvalidNumber`, `Unclosed`, `Malformed`) alongside the
human-readable `message`, so callers can branch on the class of failure without
string-matching. Both `ParseError` and `RenderError` are `#[non_exhaustive]`.

## Implementation notes

- Layered diagrams (flowchart, state, class, ER) go through a built-in
  [Sugiyama](https://en.wikipedia.org/wiki/Layered_graph_drawing) layout
  engine. There is no public layout API — the engine is a private module.
- Sequence, pie, and gantt have hand-tuned layouts that do not need
  Sugiyama.
- The SVG is built as a plain string. No XML library at runtime.

## Development

Day-to-day commands are wrapped in the `Makefile` — run `make` (or `make
help`) to list them:

| Target | What it does |
|---|---|
| `make build` | Debug build (library + `mermaid-svg` binary) |
| `make run ARGS="…"` | Run the CLI (defaults to `--help`) |
| `make check` | Fast typecheck of all targets |
| `make fmt` / `make lint` | Apply formatting / fmt-check + clippy (`-D warnings`) |
| `make test-unit` | Unit tests (in-module `#[cfg(test)]` + sugiyama) |
| `make test-integration` | Integration tests (`tests/integration.rs`; writes `target/test-samples/*.svg`) |
| `make test-doc` | Doctests |
| `make test` | All of the above |
| `make bench` | Criterion benches (`parse/<kind>` + `render/<kind>`) |
| `make gallery` | Regenerate `assets/gallery/*.md` from `samples/` |
| `make doc` | Build rustdoc with the embedded gallery |
| `make package` | Dry-run crates.io packaging |
| `make verify` | Hard gate (`lint` + `test`) — must pass before pushing |
| `make clean` | Remove build artifacts |

`CARGO_BUILD_JOBS` defaults to 4; override per invocation, e.g.
`make build CARGO_BUILD_JOBS=8`.

## License

MIT. See [LICENSE](LICENSE).
