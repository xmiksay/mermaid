# Sankey — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/sankey.rs` · Renderer: `src/svg/sankey/`.

- Sankey nodes render their **throughput value** after the name on a **single
  line** (`Name <prefix>42<suffix>`, upstream `showValues` — on by default; the
  value is the node's `max(in, out)` flow, wrapped by
  `config.sankey.prefix`/`suffix`; `showValues: false` shows only the name).
  Upstream's label string is literally `Name\n42`, but SVG `<text>` collapses
  that newline to whitespace so it renders as one line (`Coal 300`); we emit the
  space directly rather than stacking two `<tspan>`s (#317). Each node gets its
  **own color** from the hardcoded **Tableau-10** scheme
  (`d3.scaleOrdinal(d3.schemeTableau10)`) cycled in first-appearance order — the
  fixed sankey palette upstream uses, independent of the theme's pastel `cScale`
  scale (`TABLEAU10` in `src/svg/sankey/mod.rs`, #317).
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
- **Within-column ordering** matches d3-sankey (`src/svg/sankey_layout.rs`,
  `order_columns`): each column is seeded in **first-appearance order**, then
  d3's `iterations = 6` barycenter relaxation passes
  (`relaxRightToLeft`/`relaxLeftToRight` + collision resolution) re-sort each
  column top-to-bottom by vertical position to minimise link crossings. The
  port reproduces d3-sankey 0.12.3 exactly (init spreading, the recomputed
  `py = min(nodePadding, extent/(maxColLen-1))`, both `reorderLinks` at init and
  `reorderNodeLinks` during relaxation, and the `value * layerDistance` link
  weight); the relaxation runs in d3's own coordinate space and the renderer
  applies the resulting **order** to its own proportional stacking. The layout
  inputs must match upstream's d3-sankey call exactly, because the relaxation is
  padding-sensitive: vertical **extent = `height`** (default 400) and
  **padding = `nodePadding + 15`** when `showValues` is on (default `12 + 15 =
  27`) — upstream reserves the extra 15 for the value line. Feeding our smaller
  proportional drawing gap instead (the pre-#317 bug) reordered columns versus
  JS Mermaid (e.g. left column `Coal, Gas, Solar, Wind` instead of the correct
  `Coal, Solar, Wind, Gas`).
- **Header-skip divergence (deliberate).** A leading `source,target,value` CSV
  line is treated as a header and skipped (`src/parse/sankey.rs`). Upstream
  Mermaid instead renders it as literal `source`/`target` nodes. Those phantom
  nodes form a **disconnected two-node component**, so they don't affect the
  ordering of the real nodes; our output for the real nodes still matches
  d3-sankey (e.g. `samples/sankey.mmd` left column: `Coal, Solar, Wind, Gas`).
  Skipping the header is the sensible divergence.
