# mermaid-svg — architecture reference

Deep reference for the crate: module map, gallery pipeline, theme internals,
and cross-cutting parse/render behavior. Per-diagram behavior notes live in
one file per diagram kind under [docs/architecture/](architecture/) (index at
the bottom). The always-loaded project brief lives in
[.claude/CLAUDE.md](../.claude/CLAUDE.md) — keep all of these in sync with the
code in the same change.

## Layout

```
src/
├── lib.rs           public API: render*/parse/Diagram/ast::*/Theme/errors
├── bin/
│   └── mermaid-svg.rs   CLI (stdin/file → stdout/file, --theme/-f|--font/--font-size flags)
├── parse/           Mermaid source → Diagram AST (line-oriented scanners)
│   ├── mod.rs       parse()/parse_with_meta() dispatcher, ParseError + SyntaxKind, ast re-export
│   ├── ast/         all AST types (pub via lib.rs as `ast::*`) incl. DiagramMeta —
│   │                mod + block/c4/charts/class/er/flowchart/gantt/sequence/state/structure
│   ├── preamble.rs  strips frontmatter/%%{init}%%/accTitle/accDescr → DiagramMeta
│   ├── style.rs     `classDef`/`class`/`:::className`/`style`/`linkStyle` parsing
│   ├── token.rs     quote-aware tokenizing: unquote/unquote_any/find_unquoted/split_unquoted
│   ├── {sequence,flowchart,state,class,c4,block,zenuml}/  multi-file per-diagram parsers (mod + submodules)
│   └── {pie,er,gantt,journey,timeline,sankey,quadrant,xychart,radar,packet,
│        mindmap,gitgraph,requirement,architecture,kanban,treemap}.rs
├── svg/             Diagram AST → SVG string
│   ├── mod.rs       render*/render_diagram* dispatchers, RenderError, pub Theme
│   ├── builder.rs   string-based SVG writer (escape, fnum, SvgBuilder)
│   ├── geometry.rs  shared edge-clip (clip_rect/circle/rhombus) + polyline_midpoint
│   ├── label.rs     decode_label: `#…;` entity codes (markdown emphasis → markup.rs)
│   ├── markup.rs    inline-HTML labels → styled tspans (b/i/u/span/a); strip_tags
│   ├── metrics.rs   shared text_width/font_scale (per-glyph widths track font_size)
│   ├── decorate.rs  post-render role/aria + <title>/<desc> injection from DiagramMeta
│   ├── theme.rs     Theme struct + default_theme/dark/forest/neutral + with_font*
│   ├── style.rs     resolves classDef/style/linkStyle into inline fill/stroke
│   ├── gantt_date.rs civil day-count date math (days_from_civil/format_date/Excludes)
│   ├── interact.rs  shared click/link wrappers (open_click/close_click)
│   ├── {sequence,flowchart,state,class,c4,block}/  multi-file per-diagram renderers (mod + submodules)
│   └── {pie,er,gantt,journey,timeline,sankey,quadrant,xychart,radar,packet,
│        mindmap,gitgraph,requirement,architecture,kanban,treemap}.rs
├── sugiyama/        layered graph layout (private)
│   ├── mod.rs       Graph/Layout/LayoutConfig/LayoutError + layout_with()
│   ├── tests.rs
│   └── {cycle,layer,order,coord,route,work}.rs
examples/render_user.rs        small one-shot example
examples/gen-doc-diagrams.rs   regenerates assets/gallery.md (the rustdoc gallery)
tests/integration.rs           end-to-end tests; writes samples to target/test-samples/
samples/                       one `.mmd` per diagram kind, shared by benches + gallery
assets/gallery/<stem>.md       one rendered gallery section per SAMPLES entry,
                               embedded into rustdoc via src/lib.rs
gallery_build.rs               shared `SAMPLES` list + section helper, `include!`'d into
                               examples/gen-doc-diagrams.rs and tests/integration.rs
```

Cargo manifest: single `[package]`. Crate is published to crates.io as
`mermaid-svg`.

## Gallery pipeline

`gallery_build.rs` is not a module — it is `include!`'d verbatim into both
`examples/gen-doc-diagrams.rs` and `tests/integration.rs`, so its `SAMPLES`
list (one `(stem, source)` per diagram kind) and `gallery_section()` helper are
shared. `cargo run --example gen-doc-diagrams` regenerates one
`assets/gallery/<stem>.md` per `SAMPLES` entry (23 files), rewriting only the
files whose content changed and printing each rewrite — so `git status` after a
regen shows exactly which diagrams a change affected. `src/lib.rs` embeds them
into the crate rustdoc with one `#![doc = include_str!("../assets/gallery/<stem>.md")]`
per stem in `SAMPLES` order (`#![doc]` attributes concatenate in order). The
`doc_gallery_up_to_date` integration test names the stale stem if any committed
file drifts from the samples.

The split (one file per diagram, `assets/gallery/*.md`) keeps parallel
renderer PRs from conflicting on a shared base64 blob: a PR touching one
diagram regenerates exactly one gallery file. `.gitattributes` marks
`assets/gallery/*.md linguist-generated=true` so the blobs stay collapsed in
GitHub diffs. Changing `SAMPLES` itself (add/remove/reorder a stem) fans out to
the `lib.rs` include lines, so treat it as a serial-window change.

## Benches & integration samples

Bench layout: `benches/render.rs` drives criterion; it `include_str!`s the same
top-level `samples/` `.mmd` files (one per diagram kind) used by the gallery.
Two groups: `parse/<kind>` (parse only)
and `render/<kind>` (parse + render to SVG). Sized inputs use realistic
non-trivial examples (typically 10-30 lines).

Integration tests write one sample SVG per diagram kind to
`target/test-samples/<stem>.svg`, one stem per `SAMPLES` entry in
`gallery_build.rs`:
- `pie.svg`, `sequence.svg`
- `flowchart.svg`, `state.svg`
- `class.svg`, `er.svg`
- `gantt.svg`, `journey.svg`
- `timeline.svg`, `sankey.svg`
- `quadrant.svg`, `xychart.svg`
- `radar.svg`, `packet.svg`
- `mindmap.svg`, `gitgraph.svg`
- `requirement.svg`, `c4.svg`
- `block.svg`, `architecture.svg`
- `kanban.svg`, `treemap.svg`, `zenuml.svg`

## Themes — internal contract

Each per-diagram `render(d, theme: &Theme)` and any helper that touches a
theme color receives `theme: &Theme` and starts with local bindings:

```rust
fn draw_thing(svg: &mut SvgBuilder, …, theme: &Theme) {
    let fg = theme.fg;
    let flow_node_fill = theme.flow_node_fill;
    …
}
```

`format!` strings then use plain identifiers (`{fg}`), since Rust's named
format args don't support field access.

When adding a new color to `Theme`, also add it to the built-in constructors in
`src/svg/theme.rs` (`default_theme`/`dark`/`forest`/`neutral`; `base` uses
`..Self::default_theme()` struct-update so it inherits new fields for free).
Custom themes use struct-update syntax from one of the built-ins, so adding a
field is non-breaking. `by_name` maps `default`→`default_theme`,
`base`→`base` (upstream's customization palette — warm `#fff4dd` primary,
visibly distinct from `default`'s lavender, **not** an alias), and the three
named themes.

Color/font fields are `Cow<'static, str>` (not `&'static str`): built-in
constructors stay `const` (`Cow::Borrowed(...)`), but `themeVariables`/
`fontFamily` config and downstream overrides supply owned runtime strings
(`fg: "#000".into()`). `Theme` is thus `Clone`, **not** `Copy`. Renderers read a
color as `&theme.fg` (a `&Cow<str>` that deref-coerces to `&str`), so
`let fg = &theme.fg;` keeps the `format!("{fg}")` idiom working. The categorical
`pie_palette` is a `Cow<'static, [Cow<'static, str>]>` (owned-on-write) so
per-slot `themeVariables` (`pie{N}`/`git{N}`/`cScale{N}`) can recolor
individual entries via `Theme::set_palette` — `pie_color(i)` returns `&str` and
still wraps modulo the (possibly grown) length. `theme: base` **without**
overrides is now visibly distinct from `default`.
`Theme::apply_theme_variables(&mut self, vars)` recolors a base theme from the
upstream `themeVariables` names — beyond the generic ones (`primaryColor`,
`lineColor`, …) it now honors the documented per-diagram variables: sequence
(`actorBkg`/`actorBorder`/`actorTextColor`/`actorLineColor`/`signalColor`/
`signalTextColor`/`labelBoxBkgColor`/`activationBkgColor`), pie
(`pie{1..12}`/`pieStrokeColor`/`pieOpacity`/`pieTitleTextColor`), git
(`git{0..7}`/`commitLabelColor`/`tagLabelColor`), the generic `cScale{0..11}`
categorical scale, plus `titleColor`/`edgeLabelBackground`. Text-color
variables land on `Option<Str>` fields with `theme.actor_text()`/`signal_text()`/
`title()`/`commit_label()`/`tag_label()`/`pie_stroke()` accessors that fall back
to `fg`/`fg_muted`/`#fff`, so an unset variable keeps the render byte-identical.
`theme_from_meta` in `src/svg/mod.rs` wires
theme name → `themeVariables` → `fontFamily`/`fontSize` → `useMaxWidth` onto the
effective theme. `Theme::responsive` (default `true`) is cleared by
`config.useMaxWidth: false`, making `SvgBuilder::finish` emit a fixed pixel
`width`/`height`; every renderer adopts font + responsiveness via
`SvgBuilder::new(w, h).theme(theme)`.

## Cross-cutting behavior

- **Source preamble** (`src/parse/preamble.rs`) is stripped by
  `parse_with_meta` *before* per-diagram dispatch, yielding a `DiagramMeta`
  (title, `acc_title`, `acc_descr`, and the config-derived fields): YAML
  frontmatter (`--- title: … / config: { … } ---`), `%%{init: {…}}%%`
  directives, and `accTitle:`/`accDescr:` (line + `accDescr { … }` block).
  `parse()` still returns just the `Diagram`; a frontmatter `title` is copied
  onto the diagram's own `title` field when it has one (flowchart gained a
  `title`).
- **The whole `config:` tree is flattened** (frontmatter YAML via `flatten_yaml`
  indentation, `%%{init}%%` via `parse_init_object`'s JSON-ish recursion) into
  `DiagramMeta.config`, a dotted `key → value` map (`themeVariables.primaryColor`,
  `gitGraph.mainBranchName`, `kanban.ticketBaseUrl`, `flowchart.htmlLabels`, …).
  **Precedence matches upstream**: a directive overrides frontmatter and the
  *last* `%%{init}%%` wins (`meta.config.insert`, last write wins — upstream's
  `cleanAndMerge`/`assignWithDepth`); frontmatter is folded in first, before the
  init loop. An `%%{init}%%` directive **may span multiple lines** (upstream's
  directiveRegex matches newlines): `collect_init` gathers lines from the `%%{`
  opener until the `}%%` closer before parsing, so a pretty-printed init object
  no longer leaks continuation lines into dispatch. `derive_typed_fields` reads
  the honored subset out of it: `theme`, `theme_variables`, `font_family`,
  `font_size`, `use_max_width` (top-level *or* the per-diagram
  `<diagram>.useMaxWidth` key — upstream's schema only defines it per diagram,
  via `per_diagram_use_max_width`), `look`/`layout`/`security_level` (parsed, not
  yet honored), `ticket_base_url`, `value_format`, `git_graph.*`. Closing a
  per-diagram config gap is a `meta.config` lookup, not new scanning.
- **Rendering is `parse_with_meta` → `render_body` (per-diagram match) →
  `decorate::apply`.** `theme_from_meta` builds the effective theme: a preamble
  `theme` overrides the caller's, then `themeVariables`/`fontFamily`/`fontSize`/
  `useMaxWidth` layer on top.
  `decorate` (string surgery on the finished doc) always adds
  `role="graphics-document document"` + `aria-roledescription="<kind>"`, and
  when meta carries accTitle/accDescr injects `<title>`/`<desc>` + the matching
  `aria-labelledby`/`aria-describedby`. `render_diagram_with` (no meta) still
  gets role/aria but no title/desc.
- **Output is responsive**: `SvgBuilder::finish()` emits `width="100%"` +
  `style="max-width: {w}px;"` + `viewBox` and **no fixed height** (upstream
  shape). Tests must not assert a root `height="…"`.
- **Label text is decoded** in `SvgBuilder::text()` via `decode_label`
  (`src/svg/label.rs`), which strips KaTeX `$$…$$` math fences (`strip_math`:
  `$$x^2$$` → `x^2` — full KaTeX layout is out of scope, so the raw delimiters
  are dropped rather than leaked; only matched `$$` pairs are unwrapped) then
  resolves `#…;` entity codes
  (`#quot;`→`"`, `#35;`→`#`, `#9829;`/`#x2665;`→`♥`, named set). Backtick-fenced
  markdown *strings* and their `**bold**`/`*italic*`/`__`/`_` emphasis are
  handled one layer up by `parse_spans` (`src/svg/markup.rs`): a fenced line is
  routed to `parse_markdown_spans`, which toggles bold/italic into styled
  `<tspan>`s instead of flattening the markers to plain text. A marker-free
  fenced label still collapses to one plain run (bare `<text>` fast path). Bare
  labels with `_`/`*` (e.g. `snake_case`) are never touched.
- **Inline HTML labels** (`htmlLabels`, `src/svg/markup.rs`): `SvgBuilder::text`
  first line-splits (`split_label_lines` on `<br>`/`\n`), then `parse_lines`
  walks **all** lines together into styled runs mapped onto `<tspan>`s —
  `<b>`/`<strong>`→`font-weight="bold"`, `<i>`/`<em>`→italic,
  `<u>`→underline, `<span style="color:…">`→`fill`, `<a href>`→wraps the run in
  an SVG `<a>`. `parse_lines` threads one open-tag style stack across the line
  breaks so a tag opened before a `<br>` still styles the following line
  (`<b>a<br>b</b>` keeps both lines bold, #187); the single-line `parse_spans`
  is now a test-only helper. Tag scanning runs on the raw source *before* entity
  decoding, so
  `#lt;`-encoded brackets never masquerade as tags; the per-run text is then
  `decode_label`-ed. Unknown tags are **stripped** (not escaped), so unsupported
  markup degrades to plain text; a bare `<` that doesn't open a well-formed tag
  stays literal. A tag-free single-line label keeps the bare `<text>` fast path,
  so the whole gallery stays byte-identical. `strip_tags` gives the visible text
  for width estimation (`node_size`). Renderers that emit *literal* angle
  brackets (class generics `List<int>`, C4 `<<stereotype>>`) entity-encode them
  (`#lt;`/`#gt;`) at the source so the markup pass keeps them intact.
- **Parser unknown-line policy is hard-error everywhere.** Every diagram
  parser (flowchart included) returns `ParseError::Syntax { line }` on an
  unparseable statement — the honest library equivalent of upstream rendering
  its error diagram — rather than silently dropping it, so a typo can't vanish.
  Flowchart also errors on a recognized keyword with an incomplete body (bare
  `style`/`classDef`/`class`/`linkStyle`/`click`, unknown `direction` token).
  Two deliberate tolerances remain (documented in
  `src/parse/flowchart/mod.rs`): a top-level `direction` is a validated no-op,
  and unknown keys / `shape:` names inside a v11 `id@{ … }` block fall back to
  `Rect` for forward compatibility.
- Sugiyama waypoints include **endpoints** (center of src, center of dst).
  The SVG renderer clips them to the node boundary itself.
- Label line breaks: `split_label_lines()` in `src/svg/builder.rs` splits any
  label on `<br>`/`<br/>`/`<br />` (case-insensitive) and `\n` (real newline or
  the two-char literal escape). `SvgBuilder::text()` auto-emits stacked
  `<tspan>`s for multi-line labels, so every renderer honors `<br>` for free;
  flowchart also sizes nodes from the resulting line count / widest line.
- Text width scales with the font size: `src/svg/metrics.rs` owns the shared
  `text_width(s, base_char_w, font_size)` / `font_scale(font_size)` helpers
  (`= font_size / BASE_FONT_SIZE`, `BASE_FONT_SIZE = 14`). Every renderer keeps
  its own per-glyph `base_char_w` (flowchart/er/class/state `7.5`, sequence
  actor `8.0`, ER bold PK/FK `8.0`, mindmap `7.0`, requirement label `5.5`,
  edge labels `7.0`) but routes the estimate through `text_width` so node/label
  boxes grow with `--font-size` instead of overflowing (#122). `SvgBuilder::text`
  and timeline scale the `LABEL_LINE_H` line spacing by `font_scale` the same
  way. Because `font_scale(14) == 1`, every default-theme render — and the whole
  gallery — is byte-identical; only a non-default font size changes.
  `text_width` counts East-Asian-wide / full-width glyphs (`char_width_units`:
  CJK ideographs, kana, Hangul, full-width forms) as **two** half-width units,
  so a CJK label no longer overflows its shape by ~50% (#187); pure-ASCII text
  is unchanged, keeping the gallery byte-identical.
- **A leading UTF-8 BOM (U+FEFF)** is stripped at the top of `parse_with_meta`
  (`src/parse/mod.rs`) before preamble handling — Rust's `trim` leaves it, so a
  Windows-editor file would otherwise report `unknown diagram type: ﻿flowchart`
  (#187).

## Per-diagram notes

One file per diagram kind under `docs/architecture/`:

- [Flowchart](architecture/flowchart.md)
- [Sequence](architecture/sequence.md)
- [State](architecture/state.md)
- [Class](architecture/class.md)
- [ER](architecture/er.md)
- [Gantt](architecture/gantt.md)
- [Journey](architecture/journey.md)
- [Pie](architecture/pie.md)
- [Packet](architecture/packet.md)
- [Quadrant chart](architecture/quadrant.md)
- [Sankey](architecture/sankey.md)
- [XY chart](architecture/xychart.md)
- [Treemap](architecture/treemap.md)
- [C4](architecture/c4.md)
- [Requirement](architecture/requirement.md)
- [GitGraph](architecture/gitgraph.md)
- [Radar](architecture/radar.md)
- [Kanban](architecture/kanban.md)
- [Block (block-beta)](architecture/block.md)
- [Mindmap](architecture/mindmap.md)
- [ZenUML](architecture/zenuml.md)
- [Architecture (architecture-beta)](architecture/architecture.md)
- [Timeline](architecture/timeline.md)
