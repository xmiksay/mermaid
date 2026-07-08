# Timeline — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/timeline.rs` · Renderer: `src/svg/timeline/` (`mod.rs`
horizontal + shared helpers, `vertical.rs` the `direction TB` variant).

- **Block-and-arrow visual model** (matching JS Mermaid, #262): a saturated
  section band tops each named section spanning its columns, filled **period
  boxes** sit above a thick **arrow axis** (`marker-end="url(#tl-arrow)"`),
  filled **event boxes** hang below, and a dashed connector runs from each period
  through the axis down to its events. Period and event boxes reuse the section
  color; box labels take a luminance-contrasting ink (`text_color_for` picks
  `#fff` on a dark fill, `#333` otherwise). Boxes carry `class="tl-section"`/
  `"tl-period"`/`"tl-event"` for styling and test anchoring. The old dots-on-a-line
  look is gone.
- timeline header accepts a v11.14+ direction token — `timeline LR`/`timeline TD`
  (also `TB`/`BT`/`RL`) parse into `TimelineDiagram.direction` (`parse_header` in
  `src/parse/timeline.rs`), validated against the known set (unknown tokens still
  hard-error). `TB`/`TD`/`BT` render a **vertical** timeline (`vertical::render`
  in `src/svg/timeline/vertical.rs`): the same block-and-arrow model rotated a
  quarter turn — the arrow axis runs down the middle, period boxes sit to its
  left, events flow rightward per period, a dashed connector links each period
  across the axis to its events, and sections become bands (rotated labels) down
  the left margin. `LR`/`RL`/unset keep the horizontal layout (axis reversal for
  `BT`/`RL` is not modelled).
- A **sectionless** timeline advances its color per time-period (upstream
  `isWithoutSections`) instead of one flat fill: `period_color` picks
  `cscale_color(period idx)` when no section is named, `cscale_color(section idx)`
  otherwise. `config.timeline.disableMulticolor` (frontmatter/`%%{init}%%` →
  `DiagramMeta.timeline_disable_multicolor` → `TimelineDiagram.disable_multicolor`)
  forces the flat single color back on.
