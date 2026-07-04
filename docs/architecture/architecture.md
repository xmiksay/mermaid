# Architecture (architecture-beta) — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/architecture.rs` · Renderer: `src/svg/architecture.rs`.

- architecture-beta icons: `draw_arch_icon` (`src/svg/architecture.rs`) draws
  five built-in glyphs (`cloud`, `database`/`db`/`disk`, `server`,
  `internet`/`globe`, `queue`/`kafka`) and returns `false` for anything else. A
  static renderer can't fetch Iconify packs (`logos:aws-lambda`, `mdi:…`), so an
  unrecognized name falls back to the generic box **plus** the name as a caption
  (`truncate_icon_name`: segment after the last `:`, capped at 16 chars) — the
  icon identity is shown, not silently lost. A quoted icon name
  (`("logos:aws-lambda")`) is unquoted in `parse_id_icon_label`, so the caption
  never keeps a stray `"`. The titled edge form `id:S -[title]- S:id` (upstream
  langium Arrow `'--' | '-' title=ARCH_TITLE '-'`) fills `ArchEdge.label`
  (`split_titled_edge`), rendered at the edge midpoint. `align row|column id id…`
  (v11.16+) is consumed by `is_align_stmt` (honoring it in layout is a follow-up).
