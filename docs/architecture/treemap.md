# Treemap — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/treemap.rs` · Renderer: `src/svg/treemap.rs`.

- Treemap honors `classDef <name> <props>` (into `TreemapDiagram.class_defs`)
  and a node's trailing `:::name` (into `TreemapNode.class_name`, stripped
  before the label/value colon split). The renderer resolves the class through
  the shared `resolve_style`, overriding the branch fill/stroke — the raw
  `:::name` no longer leaks into the label text. Layout is **squarified**
  (Bruls/Huizing/van Wijk worst-aspect-ratio row packing in `squarify`/`worst`,
  `src/svg/treemap.rs`), not slice-and-dice, so rectangles stay near square.
  Siblings are sorted by value descending at every level before layout
  (`order_by_value`, stable so ties keep source order), matching upstream.
- **Per-section color.** Every section (any node with children) takes the next
  theme `cScale` hue in traversal order (`cscale_color(next_color)`); its direct
  leaves inherit that hue **uniformly** — no per-sibling shading. A nested
  section switches to its own hue rather than keeping the parent's, and a
  top-level leaf (no parent section) also gets its own hue. In the drinks
  sample: Cold = purple (`cScale0`), Hot = yellow (`cScale1`), the nested Tea
  section = yellow-green (`cScale2`) with its Black/Green/Herbal leaves all that
  same yellow-green. Sections and their leaves share the one flat fill; the
  white `stroke` around every cell keeps same-color neighbors legible. This
  matches upstream, which draws each branch one color and gives nested sections
  a fresh hue.
- **Labels.** Leaves center a name over its value (both clipped to the cell via
  a per-cell `<clipPath>`); section bands put the name left and the running
  total right-aligned in italics. Text color flips by fill luminance
  (`text_color`) — white on dark fills, the theme foreground on light ones.
  `showValues !== false` gates both the leaf value and the section total.
  `config.treemap.valueFormat` (frontmatter) flows through
  `DiagramMeta.value_format` → `TreemapDiagram.value_format` and formats leaf
  values via `format_value`: `$` prefix, `,` thousands, `.N` decimals, `%`
  percent (the common d3-format subset). Absent a `valueFormat`, upstream
  defaults it to `,` (thousands grouping), so bare leaf values still render
  grouped (`1,234,567`). `config.treemap.showValues` flows the same way
  (`DiagramMeta.show_values` → `TreemapDiagram.show_values`); `Some(false)`
  suppresses the leaf value text (upstream gates on `showValues !== false`).
