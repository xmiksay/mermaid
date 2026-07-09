# Quadrant chart — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/quadrant.rs` · Renderer: `src/svg/quadrant.rs`.

- Quadrant points carry optional styling on `QuadrantPoint`: a third array
  value `[x, y, r]` sets `radius`; trailing `radius:`/`color:`/`stroke-color:`/
  `stroke-width:` attributes and a `:::class` ref (resolved against
  `QuadrantDiagram::classes`, filled from top-level `classDef <name> …` lines)
  set the rest. Inline attrs override the array radius and the class default;
  the renderer falls back to the config `pointRadius` (default `r=5`), a solid
  near-black fill (theme `fg`, matching upstream's dark dots rather than the old
  pale categorical tints — #316), and a white 1.5px stroke. Point labels are
  centered below the dot (upstream) instead of jammed to its right where a
  right-edge point's label crossed the outer border. `config.quadrantChart.chartWidth`/
  `chartHeight`/`pointRadius` (frontmatter/`%%{init}%%`) flow through the
  preamble → `DiagramMeta.quadrant_chart_width`/`_height`/`quadrant_point_radius`
  → copied onto `QuadrantDiagram.chart_width`/`chart_height`/`point_radius` in
  `parse_with_meta`; the renderer sizes the plot from them (defaulting to the
  500 square). The `quadrant{1..4}Fill` themeVariables override each quadrant's
  background tint — `Theme.quadrant_fills` (`[Option<Str>; 4]`, filled by
  `apply_theme_variables`) with `Theme::quadrant_fill(quadrant)` falling back to
  the per-theme `quadrant_default_fills` when unset. Those defaults are four
  lightened tints of a single primary-color family (upstream fills all four
  quadrants from one lavender family, not distinct categorical hues — #316), and
  the outer border and cross dividers are thin lines in the theme's primary
  border color rather than a heavy near-black frame.
- Axis labels follow upstream layout: the two x-axis labels are centered under
  each horizontal half; the two y-axis labels are rotated `-90°` and centered
  along each vertical half inside the left margin. Drawing y labels horizontally
  with `text-anchor="end"` clipped long ones off the left edge of the viewBox
  (#243) — rotated, they only extend by the font height, so their `x` stays
  well inside `[0, chart_left]`.
