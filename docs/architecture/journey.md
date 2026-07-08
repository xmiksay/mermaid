# Journey — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/journey.rs` · Renderer: `src/svg/journey.rs`.

- Grammar is the plain upstream trio — `title <text>`, `section <text>`,
  `Task name: <score>: Actor1, Actor2` — parsed line-oriented. A task line
  splits on its first two `:` (`splitn(3, ':')`); the score must parse as an
  integer (`ParseError::InvalidNumber` otherwise); actors are optional,
  comma-split, empties dropped. Tasks appearing before any `section` land in
  an implicit unnamed section (upstream tolerance).
- Renderer mirrors upstream's composition (not a line chart): one colored
  section band per section, rounded task boxes beneath it, actor dots
  straddling each task's top edge, and a vertical actor legend in the left
  gutter (`LEFT_MARGIN` = 160). Section bands, actor dots and the legend all
  draw from `theme.cscale_color(i)` (the generic `cScale` scale; sections and
  actors are indexed
  independently). The legend is sorted **alphabetically** (upstream order),
  not by first appearance — and the sorted index is what colors both legend
  and dots.
- Score is encoded twice, matching upstream: a **face glyph** (smiley for
  score ≥ 4, neutral for 3, sad for ≤ 2) *and* its **vertical position**.
  Below the task row a horizontal arrow-tipped **time axis** (`marker`
  `journey-axis`) runs the full width; each task drops a dashed **stem** from
  the axis to its face. `face_cy_for(score, axis_y)` maps the score (clamped
  to 1..=5) to the face center: score 5 rides nearest the axis, score 1 sinks
  a full `SCORE_SPAN` lower. Height reserves the full band (lowest = score 1)
  so it is stable regardless of the scores present (#263).
- Audit round 5 (#190) verified journey fully clean against upstream — no
  parity issues filed. #263 later added the face-height/axis/stem encoding
  and the alphabetical legend.
