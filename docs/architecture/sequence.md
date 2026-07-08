# Sequence — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/sequence/` · Renderer: `src/svg/sequence/`.

- Sequence parser has **nested items** (`Vec<SequenceItem>`) — `Alt`/`Par`/
  `Critical` blocks have branches; `Loop`/`Opt`/`Break` have label + items;
  `Rect` has a color + items. Renderer draws labeled frames with **pentagon/flag
  tab labels** and a **dotted, theme-colored border** (`draw_block_frame`, #268:
  `stroke-dasharray="2 2"` in `theme.actor_stroke`, tab drawn as a beveled
  `<path>`, guard/condition text centered in black; `else`/`and` dividers reuse
  the same dotted stroke); the operator title (`alt`/`loop`/…) in the tab is
  **regular weight** (upstream's `.labelText`), not bold (#329); `break` reuses
  the frame with a `break` title; `rect <color>` draws a
  colored background band behind its items via a separate `draw_rect_bands`
  pass (paired `RectOpen`/`RectClose` events, LIFO stack, default fill
  `rgba(0,0,0,0.05)` when no color given). Block frames and rect bands are
  **sized to the participants they enclose**, not the whole diagram (#123):
  `draw_block_frames`/`draw_rect_bands` compute `min_x`/`max_x` from the
  participant ids referenced by the events between each open/close pair
  (`collect_ids`/`extents`, falling back to `all_extents` when the block
  encloses no message). The frame-label tab fill is theme-driven
  (`theme.frame_label_fill`), which defaults to the theme's actor background
  (`#ECECFF` lavender for `default`, matching upstream's `labelBoxBkgColor =
  actorBkg`) rather than the old gray `#EEE` (#329).
- Sequence `autonumber` is **positional**: it parses to
  `SequenceItem::AutoNumber(Option<AutoNumberConfig>)` interleaved in `items`.
  `autonumber [start [step]]` → `Some{start,step}` (defaults 1/1) turns numbering
  on and resets the counter to `start`; `autonumber off` → `None` turns it off
  for subsequent messages. `start`/`step` are **`f64`** (upstream v11.15+ accepts
  decimals — `autonumber 1.5 0.5`); the renderer threads a `&mut Numbering { on,
  step }` plus an `f64` counter through `layout_items`, tagging each numbered
  message with its number. A numbered message draws a **filled circle badge** on
  the arrow origin (`draw_seq_number` in `src/svg/sequence/messages.rs`, radius
  `SEQ_BADGE_R`, **near-black `theme.arrow_stroke` fill** matching upstream's
  `#sequencenumber` `signalColor` — the #268 badge used the purple
  `actor_stroke` (#329) — white number) instead of a
  `"1. "` text prefix; `fmt_seq_number` drops the decimal point for integral
  values, so `2.0` shows as `2`. A non-positive step falls back to `1.0`.
  `SequenceDiagram.autonumber` stays a bool flag ("was ever on").
- Sequence **lifelines** are **solid** (no dash) in the `theme.lifeline` color
  (default `#9370DB`, a purple-gray matching the actor border) rather than dashed
  gray (#268).
- Sequence **half arrows** (v11.12.3+) use upstream `sequenceDiagram.jison`'s
  spellings (#223): the barb is a *doubled* char — `\\` (upper) or `//` (lower)
  — or a single barb behind a `|` shaft (`-|\`, `-|/`). Dashed forms add a dash
  on the shaft side (`--\\`, `--//`, `--|\`, `--|/`), and the eight **reverse**
  forms put the barb at the tail (`\\-`, `//-`, `\|-`, `/|-` plus their dashed
  `--` variants). The `ARROWS` table (`src/parse/sequence/message.rs`) is
  ordered longest-first so a dashed/pipe token wins over its bare prefix at the
  same position. Each spelling maps to one of eight `ArrowKind` variants split
  by upper/lower barb × solid/dashed × forward/reverse; the barb picks the
  marker (`arrow-half-top` / `arrow-half-bottom`) and reverse forms hang it off
  the tail (`marker-start`) instead of the head (`marker-end`)
  (`define_markers`, `stroke_for` in `src/svg/sequence/messages.rs`). The
  pre-#223 single-char barbs (`-\`, `-/`) are not upstream tokens and are now a
  hard error.
- Sequence `par_over <label>` (upstream's overlapping-par frame) reuses the
  `par`/`and` `BlockFrame::Par` structure, so it accepts `and` branches and
  renders as a normal par block (`handle_block_keyword` in
  `src/parse/sequence/frames.rs`).
- Sequence `properties <id>: {…}` / `details <id>: {…}` attach actor metadata;
  like `link`/`links` they are consumed (not rendered) by `is_actor_menu` so they
  don't hard-error.
- Sequence `activate`/`deactivate` is paired and drawn as an activation band
  on the lifeline. `draw_activations` keeps a **stack** of open start-ys per
  participant (`HashMap<String, Vec<f64>>`) so nested/stacked activations (the
  `->>+` shorthand) draw one band per level, each offset `level * 3px` to the
  right instead of overwriting. Activations still open at the end of the event
  loop are flushed down to `lifeline_bottom`. The band fill/stroke are
  theme-driven (`theme.activation_fill`/`activation_stroke`). An `activate`/
  `deactivate` that directly follows a message (the `->>+`/`-->>-` shorthand, or
  a bare `activate` on the next line) starts its band **at that message's arrow**,
  not half a row below it: `layout_items` remembers the previous message's arrow
  y (`prev_msg_arrow_y`) and the activation event borrows it, matching upstream.
  - The `->>+`/`-->>-` **activation shorthand** is handled in the parser
    (`parse_message` in `src/parse/sequence/message.rs`): a leading `+`/`-` on the
    target id is stripped (not part of the participant name) and
    `parse_line_to_items` synthesizes the paired event **after** the message
    (upstream jison `actor signaltype +/- actor text`) — `+` appends
    `Activate(receiver)` (`msg.to`), `-` appends `Deactivate(sender)`
    (`msg.from`, the participant activated when it earlier received a message).
    Deactivating the *sender* (not the receiver) is what closes John's band in
    the canonical `Alice->>+John` / `John-->>-Alice` example.
- Sequence `title` accepts both the space form (`title Demo`) and the legacy
  colon form (`title: Demo`, upstream lexer `"title:"\s…`); both set
  `SequenceDiagram.title`.
- Sequence participant **type metadata** `id@{ "type": "database" }` (v11.12+)
  is split off in `parse_participant` (`split_participant_meta`/`meta_type_kind`):
  the `@{…}` block is stripped from the id and its `type`
  (`boundary`/`control`/`entity`/`database`/`actor`/`participant`) maps onto
  `ParticipantKind` — reusing the ZenUML stereotype glyphs in
  `src/svg/sequence/glyphs.rs`. Unknown/absent types keep the declared
  `participant`/`actor` kind, and a trailing `as <alias>` still binds the
  display name.
- Sequence `actor X` (vs `participant X`) renders as a **stick figure** (circle
  head + body/arms/legs, name below) instead of the rounded rect —
  `draw_actor`/`draw_actor_figure` in `src/svg/sequence/glyphs.rs` branch on
  `Participant.kind`. The figure is drawn at **full upstream size**
  (`drawActorTypeActor` proportions — head r=13, 19px torso, splayed arms/legs;
  the #268 figure was ~half scale, #329); `actor_figure_height` gives an actor
  column a taller header row than a compact participant box so the figure isn't
  squeezed, and the shared `actor_h` (max over columns) grows to fit.
- Sequence `note` boxes are theme-driven (`theme.note_fill`/`note_stroke`, no
  longer a hardcoded `#FFF5AD`) and **word-wrap** to their box width (#123):
  `note_geometry` (`src/svg/sequence/messages.rs`) computes the box (an `over`
  note spans its participants with a `NOTE_MIN_W` floor; `left/right of` keep
  `NOTE_SIDE_W`), wraps the text to the interior via `wrap_note_text` (honoring
  existing `<br>`/`\n` breaks first), and grows the box height with the line
  count. The layout pass reserves that computed height, so a multi-line note
  pushes later events down.
- Sequence `box <color> <label>` groups participants: `SequenceBox` carries an
  optional `color` (parsed in `split_box_color` — hex, `rgb()/rgba()`, or a
  named CSS color; else the whole string is the label) plus the member
  `participant_ids` (any participant declared while the box frame is open). The
  renderer (`draw_boxes`) draws a colored background rect spanning the members
  from above the actor row to below the footer, label centered on top; a
  missing color renders transparent. Reserves `BOX_LABEL_H` above the actor row.
- Sequence `create [participant|actor] X [as Y]` / `destroy X` are **positional**
  lifecycle items (`SequenceItem::Create(id)`/`Destroy(id)`, same shape as
  `AutoNumber`). `create` also registers the participant (so it gets a column);
  the renderer draws its actor box **inline** at the create point (not the top
  row) and starts the lifeline there. `destroy` ends the lifeline with an `×`
  cross (`draw_destroy_cross`) and draws no footer box. `parse()` runs
  `reorder_destroys` so each `destroy X` is moved just past the next message
  involving `X` (the `destroy Carl` / `Alice-xCarl` idiom terminates *after*
  that message). Actor menus (`link X: … @ url`, `links X: {json}`) are consumed
  by `is_actor_menu` (not rendered) so they don't hard-error.
