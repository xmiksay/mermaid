# XY chart — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/xychart.rs` · Renderer: `src/svg/xychart.rs`.

- xychart axis titles (`parse_axis`) accept upstream's `text: alphaNum | STR |
  MD_STR` — a **quoted or bare-word** title before the optional `[..]` band or
  `min --> max` range (`x-axis time 0 --> 10`, `y-axis revenue`, `x-axis "Label"
  [a, b]`). For an unquoted range the word just before `-->` is the min; earlier
  words are the title. A bare title with no band/range yields empty
  `Categories`.
- xychart data points accept an optional **per-point label** — upstream
  `dataPoint: NUMBER_WITH_DECIMAL STR` (`line [1.5 "label", 2.3]`), parsed
  (quote-aware) into `XySeries.labels` (aligned with `values`, `None` when
  absent) and drawn beside each point. Category/value lists split **quote-aware**
  (`split_unquoted`) so a `"a, b"` cell survives the comma.
- xychart value-axis ticks are **"nice" round values**, not raw 1/5-range
  divisions: `nice_ticks`/`tick_step` pick a step of 1/2/5 × 10^k so ~10 ticks
  fit the span and extend the value domain out to the nearest step multiples
  (d3's `ticks()`/`nice()`), so a 4000–11000 range reads 4000, 4500, … 11000.
  The niced bounds also become `vmin`/`vmax`, so bars/points map through the
  rounded domain.
- xychart series accept an optional **quoted title** — `bar "Revenue" [..]` /
  `line "Trend" [..]` parses into `XySeries.title` and is now drawn in a
  **legend** row above the plot (`draw_legend`, upstream `showLegend` default on;
  `config.xyChart.showLegend: false` hides it). `config.xyChart.width`/`height`
  (→ `XyChartDiagram.width`/`height`) override the default plot size, and
  `themeVariables.xyChart.plotColorPalette` (comma-separated →
  `plot_color_palette`) replaces the default palette for series colors — all
  wired through `apply_xychart_config` in `parse_with_meta`.
- xychart series colors come from a **dedicated `Theme::xychart_palette`**
  (upstream `xyChart.plotColorPalette`, hardcoded per theme), not the generic
  `cScale`: the default theme opens with pale-lavender bars (`#ECECFF`) and a
  dark gray-blue line (`#8493A6`). `theme.xychart_color(i)` wraps the scale;
  `plot_color_palette` (set via config/themeVariables) overrides it (#319).
- xychart draws **no gridlines and no point markers** — only the axes, short
  value/category tick marks and labels, bars, and the line path. Upstream draws
  neither the dotted horizontal gridlines across the plot nor circular markers
  at each data point (#319).
