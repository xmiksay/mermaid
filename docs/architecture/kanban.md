# Kanban — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/kanban.rs` · Renderer: `src/svg/kanban.rs`.

- Kanban columns and tasks accept the documented `id[Label]` bracket form
  (`split_id_label` in `src/parse/kanban.rs`): the text before `[` is the id,
  the bracketed text the display label (a bare `[Label]` reuses the label as
  id). Task `@{…}` metadata parses `assigned`/`priority`/`ticket`. The renderer
  (`src/svg/kanban.rs`) color-codes the card border by priority
  (`priority_color`: Very High/High/Low/Very Low; others use the default
  stroke) and draws the `ticket` id on the card — hyperlinked when
  `config.kanban.ticketBaseUrl` is set (captured in `preamble.rs` →
  `DiagramMeta.ticket_base_url`, copied onto `KanbanDiagram` in
  `parse_with_meta`; `#TICKET#` in the URL is replaced by the id).
