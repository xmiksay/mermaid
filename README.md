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

Four built-in themes: `default`, `dark`, `forest`, `neutral`.

```rust
use mermaid_svg::{render_with, Theme};

let svg = render_with("classDiagram\n  A <|-- B", &Theme::dark())?;
```

Custom themes: construct a [`Theme`] with the colors you want
(typically using struct-update syntax from a built-in):

```rust
let custom = Theme {
    flow_node_fill: "#fffbe6",
    flow_node_stroke: "#caa400",
    ..Theme::default_theme()
};
let svg = render_with(source, &custom)?;
```

The built-in constructors are `Theme::default_theme()`, `Theme::dark()`,
`Theme::forest()`, and `Theme::neutral()`; `Theme::with_font(family)` and
`Theme::with_font_size(px)` return a copy with the font overridden.

## Supported diagrams

23 diagram types are supported:

| Type | Header keyword(s) | Notable features |
|---|---|---|
| Pie | `pie` | `showData`, title, entries (slices under 1% dropped) |
| Sequence | `sequenceDiagram` | participants/actors, `autonumber [start [step]]`/`off`, `activate`/`deactivate`, nested `alt`/`par`/`critical`/`loop`/`opt`/`break`, `rect <color>` bands, notes |
| Flowchart | `flowchart`, `graph` | directions TD/BT/LR/RL, all edge styles (`-->`, `---`, `-.->`, `==>`, `--o`, `--x` + no-head variants), invisible links (`~~~`), bidirectional edges (`<-->`, `o--o`, `x--x`), multi-source/target (`A & B --> C & D`), nested `subgraph`, `click` links/callbacks, v11 `A@{ shape: …, label: … }` node syntax, frontmatter `title` |
| State | `stateDiagram`, `stateDiagram-v2` | composite states with parallel regions (`--`), one-line and multi-line notes |
| Class | `classDiagram` | namespaces, `direction` directive, visibility (`+`/`-`/`#`/`~`), full relation set (`<\|--`, `*--`, `o--`, `-->`, `..>`, `<\|..`) |
| ER | `erDiagram` | attribute keys (PK/FK/UK), Crow's Foot cardinality, attribute comments |
| Gantt | `gantt` | sections, task end as duration/end-date/`until <id>`, duration-only tasks, `excludes` (weekends), `todayMarker` |
| Journey | `journey` | sections, tasks with scores and actors |
| Timeline | `timeline` | sections, time periods with multiple events |
| Sankey | `sankey-beta`, `sankey` | weighted source→target flows |
| Quadrant | `quadrantChart` | axes, labelled quadrants, plotted points |
| XY chart | `xychart-beta`, `xychart` | bar and line series, x/y axes |
| Radar | `radar-beta`, `radar` | multiple axes and curves |
| Packet | `packet-beta`, `packet` | byte/bit field ranges |
| Mindmap | `mindmap` | nested nodes, node shapes |
| Git graph | `gitGraph` | commits, branches, merges, checkouts |
| Requirement | `requirementDiagram` | requirements, elements, relationships |
| C4 | `C4Context`, `C4Container`, `C4Component`, `C4Dynamic`, `C4Deployment` | people, systems (`Db`/`Queue`/`_Ext` variants), boundaries, relations, `Update*Style` overrides |
| Block | `block-beta`, `block` | grid layout, spanning blocks, edges |
| Architecture | `architecture-beta`, `architecture` | groups, services, junctions, edges |
| Kanban | `kanban` | columns and cards |
| Treemap | `treemap-beta`, `treemap` | nested weighted rectangles |
| ZenUML | `zenuml` | rendered via the sequence renderer |

### Cross-cutting features

These work on every diagram type:

- **YAML frontmatter** — a leading `--- … ---` block sets the diagram `title:`
  and, via `config: { theme: … }`, the render theme.
- **`%%{init: {"theme": …}}%%` directives** — select a built-in theme inline.
- **`accTitle:` / `accDescr:`** (and the `accDescr { … }` block form) — emitted
  as `<title>`/`<desc>` and linked with `aria-labelledby`/`aria-describedby`;
  the root `<svg>` always carries `role` + `aria-roledescription`.
- **Responsive output** — `width="100%"` + `style="max-width: Npx;"` + `viewBox`
  (no fixed height), so diagrams scale to their container.
- **Entity codes & markdown strings in labels** — `#quot;`, `#35;`, `#x2665;`
  etc. are decoded; backtick-fenced markdown emphasis (`**bold**`) is stripped
  to plain text.

Note: pie charts drop slices under 1% of the total, matching upstream.

### Styling (flowchart, class, state)

Inline CSS-style overrides are supported and resolved into concrete SVG
attributes (`fill`, `stroke`, `stroke-width`, `stroke-dasharray`, text `color`,
`font-size`):

- **`style <id> fill:#f9f,stroke:#333,stroke-width:4px`** — style one node.
- **`classDef <name> …` + `class <id> <name>`** — define a reusable class and
  attach it (multiple classes layer in order).
- **`<id>:::<name>`** — shorthand class attachment inline on a node.
- **`linkStyle <n> stroke:#f00,…`** — style the n-th edge.

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

## Implementation notes

- Layered diagrams (flowchart, state, class, ER) go through a built-in
  [Sugiyama](https://en.wikipedia.org/wiki/Layered_graph_drawing) layout
  engine. There is no public layout API — the engine is a private module.
- Sequence, pie, and gantt have hand-tuned layouts that do not need
  Sugiyama.
- The SVG is built as a plain string. No XML library at runtime.

## License

MIT. See [LICENSE](LICENSE).
