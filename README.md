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

The `render` function returns an `SVG` string for any supported diagram
source. If you already have a parsed AST, call `render_diagram` instead.

## Supported diagrams

| Type | Header keyword(s) | Notable features |
|---|---|---|
| Pie | `pie` | `showData`, title, entries |
| Sequence | `sequenceDiagram` | participants/actors, `autonumber`, `activate`/`deactivate`, nested `alt`/`par`/`critical`/`loop`/`opt`, notes |
| Flowchart | `flowchart`, `graph` | directions TD/BT/LR/RL, all edge styles (`-->`, `---`, `-.->`, `==>`, `--o`, `--x` + no-head variants), multi-source/target (`A & B --> C & D`), nested `subgraph` |
| State | `stateDiagram`, `stateDiagram-v2` | composite states with parallel regions (`--`), one-line and multi-line notes |
| Class | `classDiagram` | namespaces, `direction` directive, visibility (`+`/`-`/`#`/`~`), full relation set (`<\|--`, `*--`, `o--`, `-->`, `..>`, `<\|..`) |
| ER | `erDiagram` | attribute keys (PK/FK/UK), Crow's Foot cardinality, attribute comments |
| Gantt | `gantt` | sections, `excludes` (weekends), `todayMarker` |

## API

```rust
pub fn render(input: &str) -> Result<String, RenderError>;
pub fn render_diagram(d: &Diagram) -> Result<String, RenderError>;
pub fn parse(input: &str) -> Result<Diagram, ParseError>;

pub use ast;  // all AST types: ArrowKind, FlowNode, ClassMember, ...
```

## Known gaps

- Asymmetric flowchart shapes `[/text/]` and `[\text\]` are not parsed; use
  the standard `[text]` form.
- PNG output is not yet implemented (the plan is to add it via `resvg`).
- Only the default theme is available. Custom themes / `%%{init: ...}%%`
  blocks are not yet honored.

## Implementation notes

- Layered diagrams (flowchart, state, class, ER) go through a built-in
  [Sugiyama](https://en.wikipedia.org/wiki/Layered_graph_drawing) layout
  engine. There is no public layout API — the engine is a private module.
- Sequence, pie, and gantt have hand-tuned layouts that do not need
  Sugiyama.
- The SVG is built as a plain string. No XML library at runtime.
- See `plan.md` for the original design and `.claude/CLAUDE.md` for the
  current architecture summary.

## License

MIT. See [LICENSE](LICENSE).
