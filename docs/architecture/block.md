# Block (block-beta) — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/block/` · Renderer: `src/svg/block/`.

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
  (the label no longer swallows the arrow) and drawn through the shared
  `label::draw_edge_label` so they sit on an opaque `edgeLabelBackground` rect
  (#260). Edges resolve endpoints against a
  `Geom` map that indexes **groups by id too** (so `ID --> D` where `ID` is a
  `block:ID … end` group works) and clip to the node boundary (`clip`) so
  arrowheads land on the edge, not the center. `block:id:span` keeps the span on
  `BlockGroup.span` (min group width).
  - Layout & composite treatment (#259): columns share one width from
    `cell_dims` — the widest label's hug box (`text + PAD_X*2`, floored at
    `MIN_CELL_W`), divided down for multi-span blocks so `d["Wide"]:2` never
    inflates every column; rows share `CELL_H`. Boxes hug text rather than
    filling a fixed 100×60 cell. A composite (`block:id … end`) takes **one grid
    slot** (its `span`, default 1) like any leaf — not a whole row — drawn as a
    solid pale container (`flow_cluster_fill`/`flow_cluster_stroke`, no dashed
    frame, no title text). Its children lay out in their own frame and are
    scaled down to hug the slot via a `<g transform="translate scale">`; the
    `Geom` collector composes each `Transform` so a scaled child still reports
    its on-screen box for edge routing.
  - Shape delimiters (`parse_shape`, via the `strip_pair` helper): beyond the
    classic set, `[[..]]`→`Subroutine`, `(((..)))`→`DoubleCircle`, `>..]`→`Odd`
    (asymmetric flag), and the parallelogram/trapezoid family `[/../]`→
    `LeanRight`, `[\..\]`→`LeanLeft`, `[/..\]`→`Trapezoid`, `[\../]`→
    `TrapezoidAlt`. Longest openers are matched first so `[[`/`(((` win over `[`/
    `((`. The node tokenizer (`parse_block_line`) floors bracket depth at 0 so
    the unmatched `]` of a `>text]` shape can't glue the rest of the line into
    one token.
  - Links carry a `BlockLinkStyle` (`Solid`/`Dotted`/`Thick`/`Invisible`) plus
    a `tail`/`head` `EdgeHead` (shared with the flowchart AST). `parse_edge`
    scans the line for the first link operator: `~~~`, dotted `-.->`/`-.-`, and
    the head-bearing solid (`--` core) / thick (`==` core) forms whose head char
    is `>`→`Arrow`, `x`→`Cross`, `o`→`Circle`, or the filler (`-`/`=`)→no head
    (`---`/`===`). Solid links also take an optional **tail** marker
    (`<--`/`x--`/`o--`, upstream `[xo<]?--+[-xo>]`); a leading `x`/`o`/`<`
    counts as a tail only at a token boundary so an id ending in `o` (`foo`)
    isn't misread. It still reads an inline `-- / -. / ==` label opener off the
    tail. The renderer draws dotted (`stroke-dasharray`) / thick (wider stroke)
    styles, skips an invisible link (which still shapes layout), and emits
    `blockarrow`/`blockcross`/`blockcircle` `<marker>`s at the head
    (`marker-end`) and tail (`marker-start`) ends.
  - `columns auto` (no longer a hard-error) packs every direct cell into one row
    — `auto_column_count` sums block/group spans + space counts. `space` is a
    keyword only as `space`/`space:N`, so ids like `spaceship` survive.
    `style a,b <props>` takes a comma id-list like `class`.
