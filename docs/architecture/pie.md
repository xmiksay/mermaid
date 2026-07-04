# Pie — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/pie.rs` · Renderer: `src/svg/pie.rs`.

- Pie drops slices `< 1%` of the total (`MIN_SLICE`, matching upstream
  `createPieArcs`); insertion order and per-slice palette color are preserved.
  A **negative** slice value is a `ParseError` (`parse_entry`, upstream's
  "values must be positive"), not a silently-clamped/dropped slice.
  `config.pie.{textPosition,donutHole,legendPosition}` flow through the preamble
  → `DiagramMeta.pie_*` → copied onto `PieDiagram` (`text_position`/`donut_hole`/
  `legend_position`) in `parse_with_meta`. The renderer (`src/svg/pie.rs`)
  honors them: `textPosition` (default 0.75) scales the slice-label radius,
  `donutHole` (default 0, fraction of radius, clamped ≤0.95) makes `slice_path`
  draw annular sectors instead of full wedges, and `legendPosition`
  (`right`/`left`/`top`/`bottom`, default `right`) relays out the legend and
  canvas. Defaults keep the render byte-identical to the pre-config output.
