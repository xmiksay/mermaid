# mermaid-rs — Implementační plán

Cíl: Rust knihovna (`mermaid-svg` crate) pro renderování Mermaid diagramů přímo na SVG. Volitelně HTTP služba kompatibilní s kroki-mermaid protokolem (POST `/svg`, POST `/png`, GET `/health` na portu 8002). Bez Node.js, bez JVM, bez externích binárních závislostí.

---

## Architektura

```
mermaid-rs/
├── crates/
│   ├── sugiyama/       # layout engine, pure Rust, bez HTTP
│   ├── mermaid-parse/  # lexer + parser Mermaid syntaxe
│   ├── mermaid-svg/    # SVG generátor (hlavní knihovna)
│   └── mermaid-server/ # Axum HTTP server (volitelný binary)
└── Cargo.toml          # workspace
```

Každý crate je samostatně testovatelný. `mermaid-server` je jediný binary crate, ostatní jsou library crates.

Primární použití jako knihovna:

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

`sugiyama` nemá závislost na ničem z projektu — čistý layout engine.

---

## Použité Rust crates

| Crate | Účel |
|---|---|
| `axum` | HTTP server (pouze mermaid-server) |
| `tokio` | async runtime (pouze mermaid-server) |
| `petgraph` | graph datové struktury pro sugiyama |
| `pest` | PEG parser pro Mermaid syntaxi |
| `quick-xml` | XML/SVG builder |
| `resvg` | SVG rasterizer pro PNG výstup |
| `serde` / `serde_json` | konfigurace, error response |
| `tracing` | structured logging |
| `thiserror` | error typy |

---

## Fáze 1 — `sugiyama` crate

Vstup: orientovaný graf (může být cyklický), rozměry nodů.
Výstup: `(x, y)` pro každý nod, waypoints pro každou hranu.

### 1.1 Datové typy

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

### 1.2 Cycle Removal

- DFS traversal grafu
- Back edges (hrany zpět v DFS stromu) se označí jako reversed
- Po layoutu se reversed hrany vrátí (waypoints se invertují)
- Implementace: rekurzivní DFS s `visited` a `stack` bitmapou nad `petgraph::Graph`

### 1.3 Layer Assignment

Implementovat **Longest Path** jako základ:

- Nody bez příchozích hran → rank 0
- Iterativní propagace: `rank[v] = max(rank[u] + 1)` pro každého předchůdce `u`
- Hrany přeskakující více vrstev → vložit dummy nody na každé mezilehlé vrstvě
- Dummy nody mají `size = (0, 0)` a jsou odstraněny po layoutu

### 1.4 Crossing Minimization

Iterativní barycenter sweep:

```
for iter in 0..MAX_ITER:
    sweep nahoru (od vrstvy 0):
        pro každý nod v vrstvě L:
            barycenter = průměr pozic sousedů ve vrstvě L-1
        seřadit nody vrstvy L podle barycenter
    sweep dolů (od poslední vrstvy):
        stejně, ale sousedé ve vrstvě L+1
    if no improvement: break
```

### 1.5 Node Positioning — Brandes-Köpf

Čtyři průchody kombinací `{left, right} × {up, down}`:

1. Označit type 1 conflicts — křížení hran mezi inner segments (dummy→dummy)
2. Vertical alignment — každý nod se zarovná ke svému mediánovému sousedu pokud není konflikt
3. Horizontal compaction — přiřadit X souřadnice v rámci bloků
4. Zprůměrovat výsledky čtyř průchodů

### 1.6 Edge Routing

- Waypoints hrany = souřadnice dummy nodů na trase
- Odstranit dummy nody z výsledku
- Cubic Bezier přes waypoints (de Casteljau)
- Orthogonální routing jako alternativa

### 1.7 Testování

Unit testy na každý krok samostatně + integration test: jednoduchý graf → validní `Layout` struct.

---

## Fáze 2 — `mermaid-parse` crate

PEG gramatika přes `pest`. Referenční gramatika je v Mermaid.js repozitáři (`packages/mermaid/src/diagrams/`).

### 2.1 Podporované typy (priorita)

| Typ | Layout |
|---|---|
| `sequenceDiagram` | vlastní (lineární) |
| `pie` | vlastní (kruh) |
| `gantt` | vlastní (časová osa) |
| `flowchart` / `graph` | sugiyama |
| `classDiagram` | sugiyama + UML symboly |
| `erDiagram` | sugiyama + kardinalita |
| `stateDiagram` | sugiyama |

### 2.2 AST typy

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

### 2.3 Class diagram specifika

AST zachytí:
- Třídy s atributy a metodami (viditelnost: `+`, `-`, `#`, `~`)
- Vztahy: inheritance (`<|--`), composition (`*--`), aggregation (`o--`), association (`-->`), dependency (`..>`), realization (`<|..`)
- Interface, abstract modifikátory
- Anotace (`<<interface>>`, `<<abstract>>`, vlastní)
- Namespace bloky

### 2.4 ER diagram specifika

AST zachytí:
- Entity s atributy (typ, název, klíč: PK/FK/UK)
- Vztahy s kardinalitou (`||--o{`, `}o--||`, atd.) — 8 kombinací Crow's Foot notace
- Relationship label

---

## Fáze 3 — `mermaid-svg` crate

Převod AST → SVG string přes `quick-xml`.

### 3.1 SVG infrastruktura

```rust
pub struct SvgCanvas {
    width: f64,
    height: f64,
    defs: Vec<SvgDef>,
    elements: Vec<SvgEl>,
}
```

Markery v `<defs>` pro šipky — jednou definovat, referencovat přes `marker-url`.

### 3.2 Renderer per typ

**Sequence** — bez sugiyama: lifelines, šipky, activation boxy, Alt/loop/par bloky.

**Pie** — bez sugiyama: SVG `<path>` arc segmenty, legend.

**Gantt** — bez sugiyama: časová osa, obdélníky per task.

**Flowchart** — sugiyama: nody dle syntaxe `[]`, `()`, `{}`, `(())`, hrany dle typu.

**Class** — sugiyama + UML: tři sekce (název/atributy/metody), vztahové markery v `<defs>`.

**ER** — sugiyama + Crow's Foot: entita jako tabulka, Crow's Foot markery na obou koncích hran.

### 3.3 Témata

Konstanty pro `default`, `dark`, `forest`, `neutral` — barvy a fonty.

### 3.4 PNG výstup

`resvg` — čistý Rust SVG rasterizer, žádná systémová závislost.

---

## Fáze 4 — `mermaid-server` binary (volitelné)

### 4.1 HTTP rozhraní (kroki-mermaid kompatibilní)

```
POST /svg
POST /png
GET  /health
```

### 4.2 Error response formát

```json
{
  "message": "Parse error at line 3: unexpected token '}'",
  "name": "ParseError",
  "stack": "..."
}
```

HTTP status: 400 parse error, 408 timeout, 500 internal.

### 4.3 Konfigurace (env proměnné)

| Proměnná | Default | Popis |
|---|---|---|
| `KROKI_MERMAID_PORT` | `8002` | Listen port |
| `KROKI_MERMAID_CONVERT_TIMEOUT` | `10000` | Timeout ms |
| `KROKI_MAX_BODY_SIZE` | `1048576` | Max body bytes |
| `KROKI_MERMAID_THEME` | `default` | Výchozí téma |

---

## Implementační pořadí

1. `sugiyama` crate — layout engine s unit testy
2. `mermaid-parse` — sequence + pie
3. `mermaid-svg` — sequence + pie renderer
4. `mermaid-server` — HTTP vrstva (volitelné)
5. `mermaid-parse` + `mermaid-svg` — flowchart (první využití sugiyama)
6. class diagram
7. ER diagram
8. PNG výstup přes `resvg`
9. Témata, konfigurace, edge cases

---

## Referenční materiály

- Sugiyama et al. 1981 — *Methods for Visual Understanding of Hierarchical System Structures*
- Brandes & Köpf 2001 — *Fast and Simple Horizontal Coordinate Assignment*
- Dagre JS — referenční implementace pro každý krok Sugiyama
- Mermaid.js `packages/mermaid/src/diagrams/` — gramatiky a AST struktury
- kroki-mermaid `src/index.js` — protokol reference
