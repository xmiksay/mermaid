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
│   ├── preamble/    strips frontmatter/%%{init}%%/accTitle/accDescr → DiagramMeta
│   │                (mod + frontmatter/init/config submodules)
│   ├── style.rs     `classDef`/`class`/`:::className`/`style`/`linkStyle` parsing;
│   │                parse_style_props + parse_multi_id_stmt (id-list/payload split)
│   ├── token.rs     quote-aware tokenizing: unquote/unquote_any/find_unquoted/split_unquoted/
│   │                split_top_level + extract_inline_class/split_id_label/parse_attr_pairs
│   ├── {sequence,flowchart,state,class,c4,block,zenuml}/  multi-file per-diagram parsers (mod + submodules)
│   └── {pie,er,gantt,journey,timeline,sankey,quadrant,xychart,radar,packet,
│        mindmap,gitgraph,requirement,architecture,kanban,treemap}.rs
├── svg/             Diagram AST → SVG string
│   ├── mod.rs       render*/render_diagram* dispatchers, RenderError, pub Theme
│   ├── builder/    string-based SVG writer (escape, fnum, SvgBuilder, def_arrow_marker, curves, text)
│   ├── geometry.rs  shared edge-clip (clip_rect/circle/rhombus, clip_polyline, clip_rect_ends), polyline_midpoint, fit_edges (viewBox)
│   ├── color.rs     readable_text_color: contrast-aware label color for filled shapes
│   ├── label.rs     decode_label: `#…;` entity codes (markdown emphasis → markup.rs)
│   ├── markup.rs    inline-HTML labels → styled tspans (b/i/u/span/a); strip_tags
│   ├── metrics.rs   shared text_width/font_scale (per-glyph widths track font_size)
│   ├── decorate.rs  post-render role/aria + <title>/<desc> injection from DiagramMeta
│   ├── theme/       Theme struct + default_theme/dark/forest/neutral + with_font* (mod.rs);
│   │                apply_theme_variables `themeVariables` overrides (variables.rs)
│   ├── style.rs     resolves classDef/style/linkStyle into inline fill/stroke
│   ├── gantt_date.rs civil day-count date math (days_from_civil/format_date/Excludes)
│   ├── interact.rs  shared click/link wrappers (open_click/close_click)
│   ├── sankey_layout.rs  d3-sankey column assignment + ordering relaxation
│   ├── {sequence,flowchart,state,class,c4,block,timeline}/  multi-file per-diagram renderers (mod + submodules)
│   └── {pie,er,gantt,journey,sankey,quadrant,xychart,radar,packet,
│        mindmap,gitgraph,requirement,architecture,kanban,treemap}.rs
├── sugiyama/        layered graph layout (private)
│   ├── mod.rs       Graph/Layout/LayoutConfig/LayoutError + layout_with()
│   │                + layout_pinned(LayerPins: nodes forced onto the first/last layer);
│   │                LayerPins::order (fixed order of a pinned layer),
│   │                Layout::edge_polylines (per input edge), LayoutConfig::{pull_sources,transpose}
│   ├── tests.rs
│   └── {cycle,layer,order,coord,route,work}.rs
examples/render_user.rs        small one-shot example
examples/gen-doc-diagrams.rs   regenerates assets/gallery/**/*.md (gallery + index)
tests/integration.rs           end-to-end tests; writes samples to target/test-samples/
samples/                       primary `<kind>.mmd` per kind + secondary `<kind>/NN-slug.mmd`,
                               shared by benches + gallery
assets/gallery/<stem>.md       one rendered gallery section per SAMPLES entry; primaries
                               embed into rustdoc via src/lib.rs, secondaries don't
assets/gallery/README.md       browsable index of every sample, grouped by kind
gallery_build.rs               shared `SAMPLES` list + section/index helpers, `include!`'d
                               into examples/gen-doc-diagrams.rs and tests/integration.rs
```

Cargo manifest: single `[package]`. Crate is published to crates.io as
`mermaid-svg`.

## Gallery pipeline

`gallery_build.rs` is not a module — it is `include!`'d verbatim into both
`examples/gen-doc-diagrams.rs` and `tests/integration.rs`, so its `SAMPLES`
list (one `(title, stem, source)` 3-tuple per sample) and `gallery_section()`
helper are shared. `cargo run --example gen-doc-diagrams` regenerates one
`assets/gallery/<stem>.md` per `SAMPLES` entry, rewriting only the files whose
content changed and printing each rewrite — so `git status` after a regen
shows exactly which diagrams a change affected.

**Hybrid primary/secondary scheme.** Every diagram kind has exactly one
**primary** sample: a flat `samples/<kind>.mmd` → `assets/gallery/<kind>.md`
pair (stem has no `/`, `is_primary(stem)`). A kind may additionally have any
number of **secondary** samples living under a per-kind subdirectory,
numbered from `02`: `samples/<kind>/02-some-slug.mmd` →
`assets/gallery/<kind>/02-some-slug.md` (stem is `<kind>/NN-slug`,
`kind_of(stem)` strips back to `<kind>`). Both kinds of entries are plain
`SAMPLES` 3-tuples — no structural difference, just a stem convention plus
the `kind_of`/`is_primary` helpers that read it.

**Only the primaries reach rustdoc.** `src/lib.rs` embeds exactly the 23
primary stems into the crate rustdoc with one
`#![doc = include_str!("../assets/gallery/<stem>.md")]` per stem, in
`SAMPLES` order (`#![doc]` attributes concatenate in order). This is
deliberate and does not grow as secondaries are added: both `ci.yml` and
`docs.yml` build that page with `cargo doc --no-deps --lib -D warnings` on
every push, and every extra embedded sample would slow that build. Secondary
samples exist only as render-fixtures (covered by `every_sample_renders`) and
as entries in the browsable index below — never as new `#![doc]` lines.

**Browsable index.** `gallery_index()` builds `assets/gallery/README.md`: one
`## <kind title>` heading per kind (first-appearance order, reusing that
kind's primary title string), followed by a link-only list — primary first
(labeled "Basic"), then secondaries in stem order, each linking to its own
`{stem}.md` relative to `assets/gallery/`. No images are re-embedded there,
to avoid recreating the same doc-bloat problem one level up. It's generated
by the same `gen-doc-diagrams` run and checked by the
`gallery_index_up_to_date` integration test, mirroring
`doc_gallery_up_to_date`'s pattern. The top-level `README.md` links to it.

The split (one file per sample, `assets/gallery/**/*.md`) keeps parallel
renderer PRs from conflicting on a shared base64 blob: a PR touching one
diagram regenerates exactly that diagram's gallery file(s). `.gitattributes`
marks `assets/gallery/**/*.md linguist-generated=true` so the blobs stay
collapsed in GitHub diffs, primaries and secondaries alike. Changing
`SAMPLES`' primary entries (add/remove/reorder a primary stem) fans out to
the `lib.rs` include lines, so treat that as a serial-window change; adding a
secondary entry never touches `lib.rs`.

## Benches & integration samples

Bench layout: `benches/render.rs` drives criterion; it `include_str!`s the same
top-level `samples/` `.mmd` files (one per diagram kind) used by the gallery.
Two groups: `parse/<kind>` (parse only)
and `render/<kind>` (parse + render to SVG). Sized inputs use realistic
non-trivial examples (typically 10-30 lines).

Integration tests write one sample SVG to `target/test-samples/<stem>.svg`
per `SAMPLES` entry in `gallery_build.rs` — one per primary diagram kind
(`pie.svg`, `sequence.svg`, `flowchart.svg`, `state.svg`, `class.svg`,
`er.svg`, `gantt.svg`, `journey.svg`, `timeline.svg`, `sankey.svg`,
`quadrant.svg`, `xychart.svg`, `radar.svg`, `packet.svg`, `mindmap.svg`,
`gitgraph.svg`, `requirement.svg`, `c4.svg`, `block.svg`,
`architecture.svg`, `kanban.svg`, `treemap.svg`, `zenuml.svg`), plus one
nested `<kind>/NN-slug.svg` per secondary sample (e.g.
`flowchart/02-nested-subgraphs.svg`).

Edge-geometry sweeps (#402) live in `tests/integration/edge_ends.rs`,
`edge_canvas.rs`, `edge_heads.rs` and `edge_cross.rs` (same directory),
reading SVGs back through `svg_shapes.rs` (shape boxes, cluster frames) and
`tests/integration/svg_read.rs`. They run over the repro fixtures in
`tests/fixtures/edges/*.mmd` plus the graph-shaped gallery samples: flowchart
ends meet faces by rank relation (routed ends too) and run straight under
every marker, aligned edges stay straight, every marker tip stays on its
node, small round targets keep edges in their band, no edge leaves the viewBox
(flowchart/state/class/ER/requirement), output is deterministic, and
baselines captured before #402 guard against regressions:
`baseline-paths.txt` (class/ER/requirement/state edge points, compared up to
one translation), `baseline-crossings.txt` (per flowchart case, each
`edge:node` pair where an edge runs through a rect, circle or rhombus it does
not end on — compared per edge, so a swap still fails; accepted exceptions
are listed with their reason in `crossing-waivers.txt`), `baseline-counts.txt`
(flowchart sample label collisions) and `baseline-heads.txt` (flowchart
different-kind heads closer than 12px, and bare ends within 3px of an
arrowhead tip); counts may only drop. `EDGE_BASELINE_WRITE=1 make
test-integration` rewrites the baselines after an intended change.

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
`src/svg/theme/mod.rs` (`default_theme`/`dark`/`forest`/`neutral`; `base` uses
`..Self::default_theme()` struct-update so it inherits new fields for free).
Its `themeVariables`-override wiring lives beside it in `src/svg/theme/variables.rs`
(`apply_theme_variables` and the per-diagram helpers).
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
`let fg = &theme.fg;` keeps the `format!("{fg}")` idiom working. Upstream Mermaid
derives *distinct* categorical scales from a theme's
primary/secondary/tertiary colors, and this crate mirrors that with four
`Cow<'static, [Cow<'static, str>]>` (owned-on-write) fields: `cscale_palette`
(upstream `cScale0..11`, read by journey/timeline/sankey/radar/packet/kanban/
quadrant/treemap via `cscale_color(i)`), `pie_palette` (upstream
`pie1..12`, pie only, `pie_color(i)`), `git_palette` (upstream `git0..7`,
gitGraph only, `git_color(i)`), and `xychart_palette` (upstream
`xyChart.plotColorPalette`, xychart only, `xychart_color(i)` — pale-lavender
bars / dark gray-blue line, hardcoded per theme upstream, #319). The default theme carries the exact khroma-
computed pastel values (pale-lavender `#ECECFF`/`#B9B9FF` family, pale yellows);
dark/forest/neutral share one hand-tuned array across all three scales, since the
upstream darken/hue-rotate derivation would drive their dark/achromatic primaries
to black or monochrome. Per-slot `themeVariables` recolor individual entries in
the matching scale (`pie{N}`→`set_pie_palette`, `git{N}`→`set_git_palette`,
`cScale{N}`→`set_cscale_palette`, all via the shared `set_slot` helper); every
`*_color(i)` accessor returns `&str` and wraps modulo the (possibly grown)
length. `theme: base` **without** overrides is now visibly distinct from
`default`.
`Theme::apply_theme_variables(&mut self, vars)` recolors a base theme from the
upstream `themeVariables` names — beyond the generic ones (`primaryColor`,
`lineColor`, …) it now honors the documented per-diagram variables: sequence
(`actorBkg`/`actorBorder`/`actorTextColor`/`actorLineColor`/`signalColor`/
`signalTextColor`/`labelBoxBkgColor`/`activationBkgColor`), pie
(`pie{1..12}`/`pieStrokeColor`/`pieOpacity`/`pieTitleTextColor`), git
(`git{0..7}`/`commitLabelColor`/`tagLabelColor`), gantt `todayLineColor`, the
generic `cScale{0..11}` categorical scale, plus `titleColor`/`edgeLabelBackground`. Text-color
variables land on `Option<Str>` fields with `theme.actor_text()`/`signal_text()`/
`title()`/`commit_label()`/`tag_label()`/`pie_stroke()`/`today_line()` accessors that fall back
to `fg`/`fg_muted`/`#fff`, so an unset variable keeps the render byte-identical.
`theme_from_meta` in `src/svg/mod.rs` wires
theme name → `themeVariables` → `fontFamily`/`fontSize` → `useMaxWidth` onto the
effective theme. `Theme::responsive` (default `true`) is cleared by
`config.useMaxWidth: false`, making `SvgBuilder::finish` emit a fixed pixel
`width`/`height`; every renderer adopts font + responsiveness via
`SvgBuilder::new(w, h).theme(theme)`.

## Cross-cutting behavior

- **Source preamble** (`src/parse/preamble/`) is stripped by
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
  yet honored), `ticket_base_url`, `value_format`, `git_graph.*`,
  `sequence_message_label_background` (`config.sequence.messageLabelBackground`
  — see [sequence.md](architecture/sequence.md)). Closing a per-diagram config
  gap is a `meta.config` lookup, not new scanning.
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
- **The canvas includes edges** in flowchart, state, class, ER and
  requirement: the drawn curves (the clipped polyline through
  `curve_basis_points`) are measured — not the control points, which can poke
  out where the curve does not. Flowchart folds them into its
  measure-then-shift pass; the other four call `geometry::fit_edges` after
  their own sizing, which shifts the drawing / grows the canvas only on a side
  where an edge would leave it, padded to the renderer's canvas padding.
  Class, ER and requirement clip through the shared
  `geometry::clip_rect_ends` and state through `state::edges::transition_line`,
  so the measured line is the drawn one.
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
- **Opposite edge pairs** — when both `(u, v)` and `(v, u)` are routed, they
  otherwise share one reversed polyline, so the two curves and their labels
  collapse onto one segment (the second label sits hidden under the first,
  #241). `bow_opposite_pairs` in `route.rs` bows each such edge sideways by
  `BOW` layout units, using the edge's own direction for the perpendicular so
  the pair bows to opposite sides; endpoints stay on the node centers (only the
  midpoint / interior waypoints move) so arrowheads leave each node at an angle
  and the labels no longer overlap. All graph-shaped kinds benefit for free.
  The `BOW` only parts the curves sideways, though; the two labels still land at
  the shared midpoint *along* the axis, so with #260's opaque backgrounds one
  still occludes the other (#312) — the general pairwise label-overlap pass
  below (`label_layout::layout_edge_labels`) is what actually separates them.
- **General edge-label overlap avoidance** (`src/svg/label_layout.rs`,
  `pub(super)`, shared by every graph-shaped renderer — Flowchart, State,
  Class, ER, Block, Requirement, Architecture — plus Sequence): each renderer
  draws its edges in a two-pass shape — draw every path first, collecting a
  `LabelSlot { polyline, size }` per labeled edge in the same order they were
  routed, then call `layout_edge_labels(&slots)` once for the whole diagram and
  draw every label at its resolved position. The algorithm starts each label at
  its polyline's arc-length midpoint (`geometry::polyline_midpoint`) and, for up
  to 4 passes, scans every pair `(i, j)` in that same order: if their current
  label rects overlap (`rect_overlap_area`, the same `(x, y, w, h)` overlap test
  C4's `place_label` uses), it nudges `j`'s label back toward its own
  source by one `LABEL_STAGGER_STEP` (`polyline_midpoint_offset`), clamped to
  `min(4 steps, 40% of that edge's arc length)` so a short edge gives up
  gracefully instead of sliding a label off its end; if `j` has no room left it
  tries nudging `i` forward under the same clamp; if neither can move, both are
  left as-is and the scan continues (one stuck pair never blocks another). This
  generalizes the state renderer's original opposite-pair-only stagger (#312) —
  which it has since replaced entirely — to any two colliding labels, including
  unrelated edges that merely happen to land near each other; it is a heuristic,
  not a globally optimal placement for a 3-or-more-way cluster, and is a strict
  no-op (every label at its natural midpoint) when nothing collides. Requirement
  keeps its own custom label background (a light-gray patch, not the shared
  `edge_label_bg`) and only takes the resolved *position* from the shared pass;
  Architecture has no visible label background at all (upstream has none there
  either) but still builds a `LabelSlot` with an *estimated* rect purely for the
  collision math. `layout_edge_labels`'s output only ever feeds a label's draw
  position — never a renderer's canvas bounding-box computation. C4's own
  `rect_overlap_area` was deduplicated into this module; C4's heavier
  `place_label` search (`src/svg/c4/rel_place.rs`, shape/label/line
  avoidance along a routed polyline) is unrelated and untouched.
- Label line breaks: `split_label_lines()` in `src/svg/builder/text.rs` splits any
  label on `<br>`/`<br/>`/`<br />` (case-insensitive) and `\n` (real newline or
  the two-char literal escape). `SvgBuilder::text()` auto-emits stacked
  `<tspan>`s for multi-line labels, so every renderer honors `<br>` for free;
  flowchart also sizes nodes from the resulting line count / widest line.
- Edge labels on the graph-shaped renderers (flowchart, state, class, ER,
  block) share `label::draw_edge_label` — it measures the text, paints an opaque
  `edge_label_bg` rect (theme `flow_label_bg`, wired from the
  `edgeLabelBackground` theme variable) and centers the label on top, matching
  upstream so a label stays legible where its edge crosses a node or another
  label (#260). C4 keeps its own wrapped label/`[tech]` block layout but calls
  `edge_label_bg` (one rect behind the whole block) for the same background.
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
