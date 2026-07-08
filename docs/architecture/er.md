# ER — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/er.rs` · Renderer: `src/svg/er.rs`.

- ER `EntityAttribute.comment` is populated from a quoted string after the
  attribute (`string name "the customer name"`) and rendered as a fourth
  attribute column (type · name · key · comment). `EntityAttribute.key` holds
  all comma-separated key constraints joined as `PK, FK`.
- ER attribute rows render as a true bordered table (`draw_entity`), matching
  upstream: one `<rect>` per cell with the entity's stroke as the border, row
  fills alternating between `theme.bg` and `theme.flow_node_fill` (upstream's
  white/lavender striping), `ROW_H`-tall rows, and per-entity column widths
  sized to the widest cell in each column (`entity_columns`), with the last
  column stretched to absorb any width the header forces (`resolved_columns`).
  Key markers (PK/FK) render plain, like any other cell — not the old red/bold
  (issue #255).
- ER relations accept both the glyph cardinality form (`||--o{`) and the
  **verbal/numeric** form `LEFT <card> to|optionally to <card> RIGHT : label`
  (`src/parse/er.rs`, `find_reltype` + `split_card_end`/`split_card_start`):
  `to` is identifying, `optionally to` non-identifying; cardinality words
  (`only one`, `zero or one`, `zero or more`, `one or many`, `many(0)`, `0+`,
  `1+`, `1`) map onto the existing `Cardinality` enum.
- ER entity alias `id[Alias] { … }` (and bare `id[Alias]`) sets `Entity.label`
  (display) while `Entity.name` (the id relations reference) stays clean —
  `split_id_label`, mirroring the flowchart/kanban split. `ensure_entity`
  upgrades a placeholder label when the aliased block/decl appears after a
  relation already materialized the entity.
- ER `direction TB/BT/LR/RL` fills `ErDiagram.direction`; the renderer drives
  the same size-swap/transpose the flowchart and class renderers use.
- ER Crow's-Foot markers are drawn as explicit paths (`draw_cardinality`),
  positioned along the edge from the entity boundary. The crow's foot is **wide
  at the entity border and converges to a single point** one marker length out
  along the edge (`draw_crowfoot`), matching upstream — not the inverse
  double-arrowhead that pointed at the entity (issue #313). Shared geometry
  lives in the `FOOT_DEPTH` / `CARD_CIRCLE_R` / `ZERO_MORE_CIRCLE_D` constants:
  `FOOT_DEPTH` is the convergence-point distance, and the zero-or-more circle
  sits tangent to that point so it reads as a complete, separate glyph rather
  than merging into the foot (issues #256, #313).
- ER styling: `classDef <name> <props>` fills `ErDiagram.class_defs`, `class
  <ids> <name>` fills `Entity.classes`, and `style <id> <props>` fills
  `Entity.style` (`entity_index` materializes a placeholder for a
  forward-referenced id, like the flowchart). The renderer resolves them
  through the shared `resolve_style` — the entity box fill/stroke, header text
  color, and separator/attribute stroke follow the class; unstyled entities
  stay byte-identical to the theme defaults.
