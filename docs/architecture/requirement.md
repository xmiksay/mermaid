# Requirement — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/requirement.rs` · Renderer: `src/svg/requirement.rs`.

- requirementDiagram (`src/parse/requirement.rs`) accepts both relation
  directions — forward `src - kind -> dst` and reverse `dst <- kind - src`
  (endpoints swapped so `from`→`to` order, hence layout, is preserved). Kind
  and requirement keywords are matched case-insensitively. The v11 statements
  `direction TB/BT/LR/RL`, `classDef`, `class`, and `style` are consumed
  instead of hard-erroring: `direction` fills `RequirementDiagram.direction`
  (drives the same size-swap/transpose the flowchart uses), while
  `classDef`/`class`/`style` fill `class_defs`/`node_classes`/`node_styles`
  (reusing `parse/style.rs` + `svg/style.rs::resolve_style`). Beyond the `class`
  statement, a `:::className` shorthand attaches classes to a node
  (`split_name`/`parse_class_shorthand`): trailing on a decl
  (`requirement r:::important { … }`) or standalone on its own line
  (`r:::important`). Requirement/element/relation names are **quote-aware** —
  `find_unquoted` locates the body brace and `token::unquote` strips the
  surrounding quotes (`requirement "My Req" { … }` renders `My Req`, matching
  upstream's `qString`). **Upstream-compat gotcha:** a bare attribute value
  containing dots or dashes (`docref: user-guide.md`) parses here but upstream
  11.x requires quotes (`docref: "user-guide.md"`); the shipped
  `samples/requirement.mmd` uses the quoted form so it stays dual-renderable.
  The `contains` relation draws upstream's crossed-circle
  containment head (`req-contains` marker) instead of the plain arrow, placed at
  the **container** (`from`) end as a `marker-start` (upstream puts ⊕ on the
  container's box edge). Because the parser normalizes both `src - contains ->
  dst` and `dst <- contains - src` to the same `from`→`to`, the container is
  always `from` regardless of the written direction.
- Relation stroke style matches upstream 11.x: `contains` is the only solid
  relation; every other kind (`copies`, `derives`, `satisfies`, `verifies`,
  `refines`, `traces`) is dashed and ends in the thin `req-arrow` head. The
  relation label is drawn in upstream's `<<kind>>` form on a plain edge-label
  background patch (no border/pill), not a lowercase guillemet pill.
- Node header/body match upstream's format (`svg/requirement.rs`): the header
  stereotype is title-cased in `<<…>>` form (`<<Requirement>>`,
  `<<Functional Requirement>>`, …, `<<Element>>`) and the body is prose
  `Label: value` lines (`ID`/`Text`/`Risk`/`Verification` for requirements,
  `Type`/`Doc Ref` for elements) with title-cased enum values (`high` → `High`,
  `test` → `Test`). Angle brackets in the `<<…>>` strings are emitted as the
  `#lt;`/`#gt;` entity codes so the inline-HTML parser doesn't strip
  `<Requirement>` as an unknown tag; they decode back to `<`/`>` after tag
  scanning.
