# Mindmap — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/mindmap.rs` · Renderer: `src/svg/mindmap/`.

**Upstream-compat gotcha:** a node whose label is the bare word `Mindmap`
parses here but upstream 11.x's lexer treats it as the diagram keyword and
errors (`got 'MINDMAP'`). The shipped `samples/mindmap.mmd` avoids the reserved
word (`Mind maps`) so it stays dual-renderable.

- mindmap `:::class1 class2` and `::icon(fa fa-book)` are **attachment lines**,
  not child nodes (`src/parse/mindmap.rs`): both attach to the most-recent node
  (`stack.last_mut()` / `root`), `:::` filling `MindmapNode.classes` and `::icon`
  filling `MindmapNode.icon`. The renderer (`src/svg/mindmap/draw.rs`) never prints the
  raw Font Awesome class string — `draw_mindmap_icon` maps `icon_name()` (the last
  `fa-`-prefixed token) onto a small builtin glyph set (book/star/clock/user/cog/
  cloud/database/check/heart), unknown names falling back to a generic tag glyph.
  The glyph is drawn **inside** the annotated node, to the left of the label
  (which is re-centred in the remaining width), so an icon never floats onto a
  sibling node — the earlier bug where a glyph rendered below its node landed
  visually on the node stacked beneath it.
- The layout (`src/svg/mindmap/layout.rs`) is a **deterministic radial tree** matching
  upstream's radial silhouette. `build` recurses over the tree: the root sits at
  the origin and its children are dealt around the full circle by *angular
  sector*, each sector sized in proportion to the subtree's leaf count
  (`leaves`) at radius `depth * RING_GAP`. To avoid the earlier **sprawl** (leaves
  flung far from their parent with long diagonal edges), a *non-root* node packs
  its children into a `CHILD_SPREAD`-narrowed cone centred on the parent's own
  radial line rather than across the full inherited sector, so each subtree stays
  compact around its branch node. `bounds` frames the whole disc (root circle +
  all rings) into positive space.
- **Branch coloring.** Each first-level branch owns a `section` index that all
  its descendants inherit; `branch_color` reads the categorical theme scale
  **one slot past** the section (`section 0 → cScale1`, section 1 → cScale2, …) —
  upstream's mindmap section palette is offset one past the generic scale, giving
  the yellow/green/purple/magenta branch rotation. `draw_nodes` fills every node
  as a borderless (stroke `none`), drop-shadowed (`SHADOW_FILTER`) rounded rect in
  its branch color, picking white or `theme.fg` label text by the fill's luminance
  (`is_dark`); a bare `Default` node renders as a filled rounded rect too (no more
  thin-underline text). Label font size scales down with depth (`depth_font`) so
  the root/first ring read largest. The root is a solid **bright-blue** disc
  (`Theme::git_color(0)`, the theme's saturated primary lane color) with white
  text. `draw_edges` draws each parent→child spoke as a thick line in the child's
  branch color, tapering with depth. A top-level `classDef <name> <props>` line
  fills `MindmapDiagram.class_defs`; `draw_nodes` resolves each node's `:::`
  classes through the shared `resolve_style`, overriding the fill/stroke and
  label color.
- **Multi-line labels**: the grammar is line-oriented, but a `"…"` label —
  including a `` "`**bold**\nmore`" `` markdown string — may span source lines.
  Before parsing a node the loop reassembles them: if a line opens a `"` that
  does not close, following lines are appended (joined with `\n`) until the
  quote count balances, then the joined label flows through the normal
  `unquote_any` + markdown-fence path. Without this the closing `"`/backticks
  and `]` leaked into the label and the trailing line became a bogus sibling.
