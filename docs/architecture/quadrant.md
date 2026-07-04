# Quadrant chart — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/quadrant.rs` · Renderer: `src/svg/quadrant.rs`.

- Quadrant points carry optional styling on `QuadrantPoint`: a third array
  value `[x, y, r]` sets `radius`; trailing `radius:`/`color:`/`stroke-color:`/
  `stroke-width:` attributes and a `:::class` ref (resolved against
  `QuadrantDiagram::classes`, filled from top-level `classDef <name> …` lines)
  set the rest. Inline attrs override the array radius and the class default;
  the renderer falls back to the config `pointRadius` (default `r=5`), the
  palette fill, and a white 1.5px stroke. `config.quadrantChart.chartWidth`/
  `chartHeight`/`pointRadius` (frontmatter/`%%{init}%%`) flow through the
  preamble → `DiagramMeta.quadrant_chart_width`/`_height`/`quadrant_point_radius`
  → copied onto `QuadrantDiagram.chart_width`/`chart_height`/`point_radius` in
  `parse_with_meta`; the renderer sizes the plot from them (defaulting to the
  500 square). The `quadrant{1..4}Fill` themeVariables override each quadrant's
  background tint — `Theme.quadrant_fills` (`[Option<Str>; 4]`, filled by
  `apply_theme_variables`) with `Theme::quadrant_fill(quadrant, palette_index)`
  falling back to the pie palette when unset.
