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
- **Branch color inheritance.** Each top-level section seeds a branch hue from
  the theme `cScale` (`cscale_color(rank)`); descendants inherit it so a whole
  branch stays one color family (in the drinks sample: Cold = purple, Hot =
  yellow). Leaf siblings step through darker shades of the branch hue
  (`darken`), section bands a light tint (`lighten`); `darken`/`lighten` mix a
  `#rgb`/`#rrggbb` color toward black/white and pass any other syntax through.
- **Labels.** Leaves center a dark name over its value (both clipped to the
  cell via a per-cell `<clipPath>`); section bands put the name left and the
  running total right-aligned in italics. `showValues !== false` gates both the
  leaf value and the section total.
  `config.treemap.valueFormat` (frontmatter) flows through
  `DiagramMeta.value_format` → `TreemapDiagram.value_format` and formats leaf
  values via `format_value`: `$` prefix, `,` thousands, `.N` decimals, `%`
  percent (the common d3-format subset). Absent a `valueFormat`, upstream
  defaults it to `,` (thousands grouping), so bare leaf values still render
  grouped (`1,234,567`). `config.treemap.showValues` flows the same way
  (`DiagramMeta.show_values` → `TreemapDiagram.show_values`); `Some(false)`
  suppresses the leaf value text (upstream gates on `showValues !== false`).
