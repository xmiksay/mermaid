# Timeline — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/timeline.rs` · Renderer: `src/svg/timeline/` (`mod.rs`
horizontal + shared helpers, `vertical.rs` the `direction TB` variant).

- **Block-and-arrow visual model** (matching JS Mermaid, #262/#321): a saturated
  section band tops each named section spanning its columns, filled **period
  boxes** sit above a thick **arrow axis** (`marker-end="url(#tl-arrow)"`),
  filled **event boxes** hang below, and a dark dashed connector runs from each
  period through the axis down past its events to one **tail arrow** aligned
  across all periods (`CONNECTOR_TAIL` below the tallest event stack, also
  `marker-end="url(#tl-arrow)"`). Connectors use the theme foreground ink, not
  the section color (upstream tints them dark gray, so a yellow section stays
  legible). Period/section/event fills are the section color **darkened 10% in
  lightness** (`color::darken10`) to match upstream's timeline `cScale` — the
  shared palette is the pale scale journey/kanban render, timeline uses the
  darker variant. Box labels are regular weight (never bold) and take a
  luminance-contrasting ink (`text_color_for` picks `#fff` below a 168 luma
  threshold, `#333` above — white on the dark purples/blues, dark elsewhere,
  reproducing upstream's per-section label colors). Boxes carry
  `class="tl-section"`/`"tl-period"`/`"tl-event"` for styling and test
  anchoring. The old dots-on-a-line look is gone.
- timeline header accepts a v11.14+ direction token — `timeline LR`/`timeline TD`
  (also `TB`/`BT`/`RL`) parse into `TimelineDiagram.direction` (`parse_header` in
  `src/parse/timeline.rs`), validated against the known set (unknown tokens still
  hard-error). `TB`/`TD`/`BT` render a **vertical** timeline (`vertical::render`
  in `src/svg/timeline/vertical.rs`): the same block-and-arrow model rotated a
  quarter turn — the arrow axis runs down the middle, period boxes sit to its
  left, events flow rightward per period, a dark dashed connector links each
  period across the axis and past its events to a tail arrow aligned right of
  the widest event row, and sections become bands (rotated labels) down the left
  margin. `LR`/`RL`/unset keep the horizontal layout (axis reversal for
  `BT`/`RL` is not modelled).
- A **sectionless** timeline advances its color per time-period (upstream
  `isWithoutSections`) instead of one flat fill: `period_color` picks
  `cscale_color(period idx)` when no section is named, `cscale_color(section idx)`
  otherwise. `config.timeline.disableMulticolor` (frontmatter/`%%{init}%%` →
  `DiagramMeta.timeline_disable_multicolor` → `TimelineDiagram.disable_multicolor`)
  forces the flat single color back on.
