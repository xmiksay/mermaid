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
  section band per section, rounded task boxes beneath it, a score-driven
  face glyph above each task (smiley for score ≥ 4, neutral for 3, sad for
  ≤ 2), actor dots straddling the task's top edge, and a vertical actor
  legend in the left gutter (`LEFT_MARGIN` = 160). Section bands, actor dots
  and the legend all draw from `theme.pie_color(i)` (sections and actors are
  indexed independently).
- Audit round 5 (#190) verified journey fully clean against upstream — no
  parity issues filed.
