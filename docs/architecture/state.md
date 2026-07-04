# State — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/state/` · Renderer: `src/svg/state/`.

- State `state X { ... }` is stored in `composites`; parallel regions are
  separated by `--`. Composites are **clusters, not nodes** (like flowchart
  subgraphs): the composite id is excluded from the sugiyama graph, its members
  (gathered recursively through nested composites in `compute_composite_boxes`)
  lay out inside a dashed rounded frame, and an external transition naming the
  composite id clips to the frame box via `endpoint_clip` (a `StateEndClip` with
  `kind: None` → rect clip). Synthesized `__start_N`/`__end_N`/`__hist_N` ids are
  registered in `existing` by `push_pseudo` so region-tracking counts them as
  members and their circles render inside the frame. Pseudo-state (start/end/
  fork/join) fills use `theme.fg` so they stay visible on the dark theme.
  Parallel regions (`--`) are disconnected components that the shared sugiyama
  layout would otherwise interleave, so `stack_regions` (`svg/state/composite.rs`)
  post-processes `pos`: it left-aligns each region and translates it into its own
  vertical band below the previous one, returning the y of each dashed divider
  (`stroke-dasharray="3 3"`) drawn between adjacent regions in `draw_composites`.
  Every node in a region shares one stacking offset, so `render` shifts each
  routed edge's polyline by its from-node's `node_offset` to keep edges tracking
  the moved states.
- State aliasing `state "description" as X` binds `X`'s display label to the
  quoted text (`parse_quoted_as` in `parse_state_decl`), so the id stays clean
  and a later transition referencing `X` reuses the same state — no phantom box
  named literally `"…" as X`. The composite header `state "label" as X {` reuses
  the same `parse_quoted_as`, so the cluster id is `X` (labelled with the quoted
  text) instead of the raw text becoming the id.
- A **bare state-id** on its own line (`s1`) declares that state (upstream's
  `statement: idStatement`); `is_bare_id` gates it to a single identifier token
  so a genuinely unknown multi-word statement still hard-errors, and a bare id
  inside a composite body joins the active region.
- State stereotypes: `parse_stereotype` (`parse/state/decl.rs`) maps both the
  `<<choice/fork/join/history>>` form and the `[[fork]]`/`[[join]]`/`[[choice]]`
  bracket alternates (upstream lexes the brackets as exact aliases) onto
  `StateKind`, so `state f [[fork]]` yields a real fork instead of a garbage
  `f [[fork]]` state.
- State `click X href "url"` / `click X call fn()` binds a `State.click`
  (`ClickAction`, reusing the flowchart `parse_click` — now `pub(crate)` in
  `parse/flowchart/click.rs`); the renderer wraps the state in the shared
  `open_click`/`close_click` (`svg/interact.rs`) `<a>`/`<g onclick>`. Parsing the
  `click` line before the `X : text` description branch keeps a URL's `://` colon
  from misrouting into a phantom state. `hide empty description` is consumed as a
  no-op (the static renderer always draws the id).
- State history pseudo-states parse to `StateKind::History { deep }`:
  `<<history>>` and `[H]` are shallow (`deep: false`), `[H*]` is deep. The
  bracket forms are handled in `canonicalize` like `[*]` (unique `__hist_N`
  id per occurrence); the stereotype form in `parse_state_decl`. The renderer
  draws a small circle with `H`/`H*` inside.
- State `note right of X: text` (one-liner) and `note left of X\n…\nend note`
  (multi-line) both land in `notes`.
