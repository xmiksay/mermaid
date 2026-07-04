# Timeline — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/timeline.rs` · Renderer: `src/svg/timeline.rs`.

- timeline header accepts a v11.14+ direction token — `timeline LR`/`timeline TD`
  (also `TB`/`BT`/`RL`) parse into `TimelineDiagram.direction` (`parse_header` in
  `src/parse/timeline.rs`), validated against the known set (unknown tokens still
  hard-error). The horizontal renderer treats it as a no-op. A **sectionless**
  timeline advances its color per time-period (upstream `isWithoutSections`)
  instead of one flat fill: `src/svg/timeline.rs` picks `pie_color(period idx)`
  when no section is named, `pie_color(section idx)` otherwise.
  `config.timeline.disableMulticolor` (frontmatter/`%%{init}%%` →
  `DiagramMeta.timeline_disable_multicolor` → `TimelineDiagram.disable_multicolor`)
  forces the old flat single color back on.
