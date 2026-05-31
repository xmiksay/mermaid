# mermaid-rs

Rust knihovny pro renderování Mermaid diagramů do SVG. Bez Node.js / JVM / nativních
binárek. Implementace dle `plan.md` (kořen projektu).

## Workspace

```
crates/
├── sugiyama/       layered layout engine (no deps on the rest)
├── mermaid-parse/  Mermaid syntax → AST
├── mermaid-svg/    AST → SVG (sequence, pie, flowchart)
└── (mermaid-server, mermaid-rs nejsou hotové)
```

Cargo workspace v `Cargo.toml`. Žádné jiné build systémy.

## Co je hotové

| Fáze | Crate | Status |
|---|---|---|
| 1 | sugiyama | hotová |
| 2 + 3 sequence/pie | mermaid-parse + svg | hotová |
| 5 flowchart | mermaid-parse + svg | hotová |
| 6 state diagram | mermaid-parse + svg | hotová |
| 6 class diagram | mermaid-parse + svg | hotová |
| 7 ER diagram | mermaid-parse + svg | hotová |
| ext. gantt | mermaid-parse + svg | hotová |
| 4 HTTP server | mermaid-server | **nedělané** |
| 8 PNG via resvg | mermaid-svg | nedělané |
| 9 themes + config | mermaid-svg | nedělané (jen default theme) |

## Build & test

```bash
cargo build              # workspace build
cargo test               # všechny crates
cargo test -p sugiyama   # jen jeden crate
```

Integration testy v `mermaid-svg` zapisují sample SVG do
`crates/mermaid-svg/target/test-samples/`:
- `pie_browsers.svg`, `sequence_api.svg`
- `flowchart_td.svg`, `flowchart_lr.svg`
- `state_lifecycle.svg`
- `class_uml.svg`
- `er_customer.svg`
- `gantt_release.svg`

## Známé odchylky od plánu

Vědomá zjednodušení pro v0.1. API se nemění, lze nahradit později:

1. **sugiyama X-coords**: `coord.rs` používá median-relaxation s hard non-overlap
   constraints místo plné Brandes-Köpf (1.5 v plánu). Validní layouty, ale ne
   tak vyrovnané jako 4-pass BK.
2. **mermaid-parse**: hand-rolled line scannery místo `pest` PEG (2.x v plánu).
   Mermaid syntax je line-oriented, scanner je kratší a snáz rozšiřitelný.
   AST je stejný.
3. **mermaid-svg**: vlastní string SVG builder místo `quick-xml` (3.1 v plánu).
   Žádný runtime parsing, write-only, escaping je v `svg::escape`.

## Konvence

- Žádné komentáře navíc — jen tam kde *proč* není zřejmý z kódu.
- Žádné `#[allow(dead_code)]` v library kódu (kromě malých `_use_*` placeholderů,
  které dokumentují, že symbol je veřejný API ale nepoužitý uvnitř).
- Testy: unit testy v `#[cfg(test)] mod tests` na konci každého souboru,
  end-to-end testy v `crates/*/tests/integration.rs`.
- Chyby přes `thiserror`, žádné `String` errory.
- `NodeId = u32` v sugiyama; vyšší vrstvy si dělají vlastní mapování stringů → u32.

## Pipeline pro flowchart (důležité)

Direction transformace v `mermaid-svg/src/flowchart.rs`: sugiyama umí jen
top-down, takže pro `LR`/`RL` se **swapují vstupní rozměry** `(w, h) → (h, w)` a
**výstupní souřadnice** `(sx, sy) → (sy, sx)`. Pro `BT`/`RL` se flippuje osa.

Edge clipping (`clip_to_node`) má per-shape variantu:
- rect: výpočet `t = min(hw/|dx|, hh/|dy|)`
- circle: normalizace na poloměr
- rhombus: `t = 1 / (|dx|/hw + |dy|/hh)`
- ostatní tvary fallback na rect

## Co je třeba pamatovat

- `mermaid-parse` neumí asymetrické tvary `[/text/]`, `[\text\]` atd. — `(` `[`
  `{` `((` `{{` `[[` `[(` `([` jsou jediné podporované.
- Flowchart parser tichounce přeskakuje `subgraph`/`end`/`style`/`classDef`/
  `class`/`click`/`linkStyle`. Nejsou implementované, ale parser nepadne.
- `sequence_diagram` neumí `alt`/`loop`/`par`/`opt` bloky, notes, activate/
  deactivate, autonumber.
- Sugiyama waypoints obsahují **endpointy** (center src, center dst). SVG
  renderer si je sám clipuje na boundary nodu.
