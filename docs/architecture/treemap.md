# Treemap — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/treemap.rs` · Renderer: `src/svg/treemap.rs`.

- Treemap honors `classDef <name> <props>` (into `TreemapDiagram.class_defs`)
  and a node's trailing `:::name` (into `TreemapNode.class_name`, stripped
  before the label/value colon split). The renderer resolves the class through
  the shared `resolve_style`, overriding the palette fill/stroke — the raw
  `:::name` no longer leaks into the label text. Layout is **squarified**
  (Bruls/Huizing/van Wijk worst-aspect-ratio row packing in `squarify`/`worst`,
  `src/svg/treemap.rs`), not slice-and-dice, so rectangles stay near square.
  `config.treemap.valueFormat` (frontmatter) flows through
  `DiagramMeta.value_format` → `TreemapDiagram.value_format` and formats leaf
  values via `format_value`: `$` prefix, `,` thousands, `.N` decimals, `%`
  percent (the common d3-format subset). Absent a `valueFormat`, upstream
  defaults it to `,` (thousands grouping), so bare leaf values still render
  grouped (`1,234,567`). `config.treemap.showValues` flows the same way
  (`DiagramMeta.show_values` → `TreemapDiagram.show_values`); `Some(false)`
  suppresses the leaf value text (upstream gates on `showValues !== false`).
