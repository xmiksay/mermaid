# mermaid-svg — Implementation plan

> **Historical design plan from the start of the project. The current
> architecture is a single `mermaid-svg` crate (no workspace, no separate
> server crate) — see `README.md` and `.claude/CLAUDE.md` for what actually
> shipped.**

Goal: Rust library (`mermaid-svg` crate) for rendering Mermaid diagrams
directly to SVG. Optionally an HTTP service compatible with the
kroki-mermaid protocol (POST `/svg`, POST `/png`, GET `/health` on port
8002). No Node.js, no JVM, no external native binaries.

---

## Architecture

```
mermaid-rs/
├── crates/
│   ├── sugiyama/       # layout engine, pure Rust, no HTTP
│   ├── mermaid-parse/  # lexer + parser for Mermaid syntax
│   ├── mermaid-svg/    # SVG generator (the main library)
│   └── mermaid-server/ # Axum HTTP server (optional binary)
└── Cargo.toml          # workspace
```

Each crate is independently testable. `mermaid-server` is the only binary
crate; the others are libraries.

Primary use as a library:

```rust
use mermaid_svg::render;

let svg = render("classDiagram\n  A <|-- B")?;
```

---

## Crate dependencies

```
mermaid-server
    └── mermaid-svg
            ├── mermaid-parse
            └── sugiyama
```

`sugiyama` has no dependency on anything else in the project — pure layout
engine.

---

## Rust crates used

| Crate | Purpose |
|---|---|
| `axum` | HTTP server (mermaid-server only) |
| `tokio` | async runtime (mermaid-server only) |
| `petgraph` | graph data structures for sugiyama |
| `pest` | PEG parser for Mermaid syntax |
| `quick-xml` | XML/SVG builder |
| `resvg` | SVG rasterizer for PNG output |
| `serde` / `serde_json` | configuration, error response |
| `tracing` | structured logging |
| `thiserror` | error types |

---

## Phase 1 — `sugiyama` crate

Input: directed graph (possibly cyclic), node sizes.
Output: `(x, y)` for each node, waypoints for each edge.

### 1.1 Data types

```rust
pub struct Graph {
    pub nodes: Vec<NodeId>,
    pub edges: Vec<(NodeId, NodeId)>,
    pub node_size: HashMap<NodeId, (f64, f64)>,
}

pub struct Layout {
    pub node_pos: HashMap<NodeId, (f64, f64)>,
    pub edge_points: HashMap<(NodeId, NodeId), Vec<(f64, f64)>>,
}
```

### 1.2 Cycle removal

- DFS traversal of the graph
- Back edges (edges going up the DFS tree) are marked as reversed
- After layout, reversed edges are restored (waypoints inverted)
- Implementation: recursive DFS with `visited` and `stack` bitmaps over
  `petgraph::Graph`

### 1.3 Layer assignment

Implement **Longest Path** as the baseline:

- Nodes with no incoming edges → rank 0
- Iterative propagation: `rank[v] = max(rank[u] + 1)` for each predecessor `u`
- Edges spanning multiple layers → insert dummy nodes on each intermediate
  layer
- Dummy nodes have `size = (0, 0)` and are removed after layout

### 1.4 Crossing minimization

Iterative barycenter sweep:

```
for iter in 0..MAX_ITER:
    sweep up (from layer 0):
        for each node v in layer L:
            barycenter = mean of positions of neighbors in layer L-1
        sort nodes of layer L by barycenter
    sweep down (from the last layer):
        same, but neighbors in layer L+1
    if no improvement: break
```

### 1.5 Node positioning — Brandes-Köpf

Four passes combining `{left, right} × {up, down}`:

1. Mark type-1 conflicts — crossings of edges between inner segments
   (dummy→dummy)
2. Vertical alignment — each node aligns to its median neighbor unless there
   is a conflict
3. Horizontal compaction — assign X coordinates within blocks
4. Average the results of the four passes

### 1.6 Edge routing

- Edge waypoints = coordinates of dummy nodes along the path
- Remove dummy nodes from the output
- Cubic Bezier through waypoints (de Casteljau)
- Orthogonal routing as an alternative

### 1.7 Testing

Unit tests on each step separately + integration test: simple graph → valid
`Layout` struct.

---

## Phase 2 — `mermaid-parse` crate

PEG grammar via `pest`. The reference grammar is in the Mermaid.js repo
(`packages/mermaid/src/diagrams/`).

### 2.1 Supported types (priority)

| Type | Layout |
|---|---|
| `sequenceDiagram` | custom (linear) |
| `pie` | custom (circle) |
| `gantt` | custom (timeline) |
| `flowchart` / `graph` | sugiyama |
| `classDiagram` | sugiyama + UML symbols |
| `erDiagram` | sugiyama + cardinality |
| `stateDiagram` | sugiyama |

### 2.2 AST types

```rust
pub enum Diagram {
    Sequence(SequenceDiagram),
    Class(ClassDiagram),
    Er(ErDiagram),
    Flowchart(FlowchartDiagram),
    Pie(PieDiagram),
    Gantt(GanttDiagram),
    State(StateDiagram),
}
```

### 2.3 Class diagram specifics

AST captures:
- Classes with attributes and methods (visibility: `+`, `-`, `#`, `~`)
- Relations: inheritance (`<|--`), composition (`*--`), aggregation (`o--`),
  association (`-->`), dependency (`..>`), realization (`<|..`)
- Interface, abstract modifiers
- Annotations (`<<interface>>`, `<<abstract>>`, custom)
- Namespace blocks

### 2.4 ER diagram specifics

AST captures:
- Entities with attributes (type, name, key: PK/FK/UK)
- Relations with cardinality (`||--o{`, `}o--||`, etc.) — 8 combinations of
  Crow's Foot notation
- Relationship label

---

## Phase 3 — `mermaid-svg` crate

AST → SVG string via `quick-xml`.

### 3.1 SVG infrastructure

```rust
pub struct SvgCanvas {
    width: f64,
    height: f64,
    defs: Vec<SvgDef>,
    elements: Vec<SvgEl>,
}
```

Markers in `<defs>` for arrows — defined once, referenced via `marker-url`.

### 3.2 Per-type renderer

**Sequence** — no sugiyama: lifelines, arrows, activation boxes, alt/loop/par
blocks.

**Pie** — no sugiyama: SVG `<path>` arc segments, legend.

**Gantt** — no sugiyama: timeline, rectangles per task.

**Flowchart** — sugiyama: nodes by `[]`, `()`, `{}`, `(())` syntax, edges by
type.

**Class** — sugiyama + UML: three sections (name/attributes/methods), relation
markers in `<defs>`.

**ER** — sugiyama + Crow's Foot: entity as a table, Crow's Foot markers on
both edge ends.

### 3.3 Themes

Constants for `default`, `dark`, `forest`, `neutral` — colors and fonts.

### 3.4 PNG output

`resvg` — pure-Rust SVG rasterizer, no system dependency.

---

## Phase 4 — `mermaid-server` binary (optional)

### 4.1 HTTP interface (kroki-mermaid compatible)

```
POST /svg
POST /png
GET  /health
```

### 4.2 Error response format

```json
{
  "message": "Parse error at line 3: unexpected token '}'",
  "name": "ParseError",
  "stack": "..."
}
```

HTTP status: 400 parse error, 408 timeout, 500 internal.

### 4.3 Configuration (env vars)

| Variable | Default | Description |
|---|---|---|
| `KROKI_MERMAID_PORT` | `8002` | Listen port |
| `KROKI_MERMAID_CONVERT_TIMEOUT` | `10000` | Timeout ms |
| `KROKI_MAX_BODY_SIZE` | `1048576` | Max body bytes |
| `KROKI_MERMAID_THEME` | `default` | Default theme |

---

## Implementation order

1. `sugiyama` crate — layout engine with unit tests
2. `mermaid-parse` — sequence + pie
3. `mermaid-svg` — sequence + pie renderer
4. `mermaid-server` — HTTP layer (optional)
5. `mermaid-parse` + `mermaid-svg` — flowchart (first use of sugiyama)
6. class diagram
7. ER diagram
8. PNG output via `resvg`
9. Themes, configuration, edge cases

---

## Reference material

- Sugiyama et al. 1981 — *Methods for Visual Understanding of Hierarchical System Structures*
- Brandes & Köpf 2001 — *Fast and Simple Horizontal Coordinate Assignment*
- Dagre JS — reference implementation for each step of Sugiyama
- Mermaid.js `packages/mermaid/src/diagrams/` — grammars and AST structures
- kroki-mermaid `src/index.js` — protocol reference
