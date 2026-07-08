# Sankey — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/sankey.rs` · Renderer: `src/svg/sankey.rs`.

- Sankey nodes render their **throughput value** after the name
  (`Name\n<prefix>42<suffix>`, upstream `showValues` — on by default; the value
  is the node's `max(in, out)` flow, wrapped by `config.sankey.prefix`/`suffix`;
  `showValues: false` shows only the name). The `SvgBuilder::text` multi-line
  path stacks the value as a second `<tspan>`. Each node gets its **own palette
  color** (`cscale_color(node index)`, the generic `cScale` scale), no longer
  one flat fill.
  `config.sankey.{linkColor,nodeAlignment,showValues,prefix,suffix,width,height,
  nodeWidth,nodePadding}` (frontmatter/`%%{init}%%`) flow through the preamble →
  the matching `DiagramMeta.sankey_*` fields → copied onto `SankeyDiagram` in
  `parse_with_meta`. `linkColor` (`LinkColor::parse`, **default `gradient`** to
  match upstream): `source`/`target` tint each link from that node's color,
  `gradient` emits a per-link `<linearGradient>` in `<defs>`, any other value is
  a literal stroke color. Geometry is config-driven: `nodeWidth` (`NODE_W`,
  upstream default `10`), `nodePadding` (`ROW_GAP`), `height` (`CHART_H`), and
  `width` (recomputes the per-column gap). `nodeAlignment` (`Alignment::parse`, default
  `justify`) maps onto the column-assignment step (`assign_columns`, using
  `column_depths`/`column_heights`): `left` = depth from source, `right` =
  distance to sink, `justify` pushes sinks to the last column, `center` nudges
  source-less nodes toward their earliest target (d3-sankey semantics).
