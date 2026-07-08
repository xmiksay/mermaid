# Timeline — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/timeline.rs` · Renderer: `src/svg/timeline.rs`.

- timeline header accepts a v11.14+ direction token — `timeline LR`/`timeline TD`
  (also `TB`/`BT`/`RL`) parse into `TimelineDiagram.direction` (`parse_header` in
  `src/parse/timeline.rs`), validated against the known set (unknown tokens still
  hard-error). `TB`/`TD`/`BT` render a **vertical** timeline (`render_vertical` in
  `src/svg/timeline.rs`): the axis runs down the left, periods stack top→bottom
  with labels to the axis's right, events flow rightward per period, and sections
  become vertical bands (rotated labels) down the left margin — the horizontal
  layout rotated a quarter turn. `LR`/`RL`/unset keep the horizontal layout (axis
  reversal for `BT`/`RL` is not modelled). A **sectionless**
  timeline advances its color per time-period (upstream `isWithoutSections`)
  instead of one flat fill: `src/svg/timeline.rs` picks
  `cscale_color(period idx)` when no section is named,
  `cscale_color(section idx)` otherwise.
  `config.timeline.disableMulticolor` (frontmatter/`%%{init}%%` →
  `DiagramMeta.timeline_disable_multicolor` → `TimelineDiagram.disable_multicolor`)
  forces the old flat single color back on.
