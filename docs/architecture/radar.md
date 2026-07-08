# Radar — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/radar.rs` · Renderer: `src/svg/radar.rs`.

- radar-beta (`src/parse/radar.rs`): multiple `axis` lines **accumulate**
  (`d.axes.extend`, not assign). Option keywords `min`/`max`/`ticks`/
  `graticule circle|polygon`/`showLegend [bool]` are consumed instead of
  hard-erroring. A curve body is either a positional list (`{85, 90}`) or
  `key: value` pairs (`{ Power: 85, Speed: 90 }`, detected by a `:`), the
  latter matched to axes by id then label — order-independent, missing axes
  default to 0. The renderer (`src/svg/radar.rs`) draws a **filled light-gray
  graticule disc** (`fg_muted` at `fill-opacity 0.12`) behind `ticks` fainter
  ring outlines (`fg_muted` at `stroke-opacity 0.35`), drawn as concentric
  **circles** by default (`graticule polygon` for the old polygon rings) via the
  shared `draw_ring` helper; each spoke is capped with a short dark tick (`fg`)
  perpendicular to it at the outer ring, matching upstream's disc/ring/tick
  styling. Curves scale over `[min, max]` so `min` acts as a scale offset;
  `showLegend false` suppresses the legend.
- **Title quotes are stripped** (`unquote` in `src/parse/radar.rs`), so
  `title "Skills"` renders as `Skills`. This deliberately diverges from
  upstream Mermaid 11.16, which renders the surrounding quotes literally;
  stripping is consistent with how every other diagram kind treats quoted
  titles/labels here, so we keep it. Curves are drawn as a **closed
  cardinal (Catmull-Rom) spline** for the default circle graticule
  (`cardinal_closed_path`, d3's `curveCardinalClosed.tension`, `k=(1−t)/6`) and
  as straight closed segments (`straight_closed_path`) for `graticule polygon`.
  `config.radar.{width,height,marginTop,marginBottom,marginLeft,marginRight,
  axisScaleFactor,curveTension}` flow through `apply_radar_config`
  (`src/parse/mod.rs`, from `meta.config`) onto `RadarDiagram`: width/height
  override the derived SVG size (default 680×520 + title), margins replace the
  symmetric `PAD`, `axisScaleFactor` multiplies the curve plot radius, and
  `curveTension` sets the spline tension (default 0.17).
