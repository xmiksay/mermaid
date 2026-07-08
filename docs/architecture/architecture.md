# Architecture (architecture-beta) — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/architecture.rs` · Renderer: `src/svg/architecture.rs`.

- Layout is driven by the edge **port hints**, not sugiyama (#257). `grid_place`
  (`src/svg/architecture.rs`) assigns each node integer `(col, row)` grid
  coordinates: following an edge `from:S₁ -- S₂:to`, the neighbour sits one cell
  away in the direction of the anchored node's named side (`side_delta`: `L`→left,
  `R`→right, `T`→up, `B`→down), so an `L`/`R` pair shares a row (`db:L -- R:server`
  ⇒ server left of db, joined horizontally) and a `T`/`B` pair shares a column
  (`disk1:T -- B:server` ⇒ disk1 hangs below server). Each connected component is
  grown breadth-first from its source-order seed; separate components (and
  edge-less nodes) start in fresh columns. Grid columns/rows are compressed to
  compact ranks, then each node is placed at its cell centre. Edges are routed
  with straight orthogonal segments between the pinned sides (`ortho_route`):
  same-axis ports get a two-segment jog (straight when already aligned), mixed
  axes a single elbow — never a free-angle diagonal.
- architecture-beta icons: `draw_arch_icon` (`src/svg/architecture.rs`) draws
  six built-in glyphs (`cloud`, `database`/`db`, `disk` — a distinct concentric
  platter, `server`, `internet`/`globe`, `queue`/`kafka`) at a caller-chosen
  size, and returns `false` for anything else. Service glyphs render white on a
  solid blue tile (`ICON_TILE`); a group's `(icon)` renders to the left of its
  title. Service ids are no longer printed as captions — only the label. A
  static renderer can't fetch Iconify packs (`logos:aws-lambda`, `mdi:…`), so an
  unrecognized name falls back to the generic box **plus** the name as a caption
  (`truncate_icon_name`: segment after the last `:`, capped at 16 chars) — the
  icon identity is shown, not silently lost. A quoted icon name
  (`("logos:aws-lambda")`) is unquoted in `parse_id_icon_label`, so the caption
  never keeps a stray `"`. The titled edge form `id:S -[title]- S:id` (upstream
  langium Arrow `'--' | '-' title=ARCH_TITLE '-'`) fills `ArchEdge.label`
  (`split_titled_edge`), rendered at the edge midpoint. `align row|column id id…`
  (v11.16+) is parsed by `parse_align` into `ArchitectureDiagram.aligns` and
  honored by `apply_aligns` (`src/svg/architecture.rs`): within a group, the
  listed nodes are repositioned into a shared row (common y, boxes left→right) or
  column (common x, boxes top→bottom), anchored at their current top-left, after
  the grid pass. Directives naming fewer than two in-group nodes are ignored.
