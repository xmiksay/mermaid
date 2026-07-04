# Mindmap — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/mindmap.rs` · Renderer: `src/svg/mindmap.rs`.

- mindmap `:::class1 class2` and `::icon(fa fa-book)` are **attachment lines**,
  not child nodes (`src/parse/mindmap.rs`): both attach to the most-recent node
  (`stack.last_mut()` / `root`), `:::` filling `MindmapNode.classes` and `::icon`
  filling `MindmapNode.icon`. The renderer (`src/svg/mindmap.rs`) never prints the
  raw Font Awesome class string — `draw_mindmap_icon` maps `icon_name()` (the last
  `fa-`-prefixed token) onto a small builtin glyph set (book/star/clock/user/cog/
  cloud/database/check/heart), unknown names falling back to a generic tag glyph.
  The layout (`src/svg/mindmap.rs`) is **two-sided**: first-level branches are
  dealt alternately onto the right and left of a centred root (`layout` builds a
  canonical right-growing subtree, `mirror` reflects the left ones about the root
  centre and flags them `dir = -1`), so the map fans out on both sides instead of
  only rightward. `draw_edges` picks the parent's right or left edge by the
  child's `dir`; `bounds` frames both halves (plus icon glyphs) into positive
  space. A top-level `classDef <name> <props>` line fills
  `MindmapDiagram.class_defs`; `draw_nodes` resolves each node's `:::` classes
  through the shared `resolve_style`, overriding the node fill/stroke and label
  color (unstyled nodes stay byte-identical).
- **Multi-line labels**: the grammar is line-oriented, but a `"…"` label —
  including a `` "`**bold**\nmore`" `` markdown string — may span source lines.
  Before parsing a node the loop reassembles them: if a line opens a `"` that
  does not close, following lines are appended (joined with `\n`) until the
  quote count balances, then the joined label flows through the normal
  `unquote_any` + markdown-fence path. Without this the closing `"`/backticks
  and `]` leaked into the label and the trailing line became a bogus sibling.
