# Class — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/class/` · Renderer: `src/svg/class/`.

- Class `namespace X { class A; class B }` is stored in `namespaces`; the
  renderer draws a dashed rect around the members. `namespace Name["label"]`
  splits into a clean id + display `Namespace.label` (via `extract_class_label`,
  like `class Name["label"]`), the renderer showing the label. Nested
  namespaces work: each class is registered with **every** namespace on the
  stack, so an outer frame's bounds enclose the inner one's classes;
  `Namespace.depth` (0 = outermost) makes the renderer draw shallower frames
  with more padding so the outer visibly wraps the inner.
- Class **two-way relations** (`relationType lineType relationType`, e.g.
  `<|--|>`, `*--*`, `o--o`, `<-->`, `<..>`) glue a mirror marker onto the base
  token; `detect_two_way` (`src/parse/class/relation.rs`) consumes that trailing
  `|>`/`>`/`*`/`o` (only when the base is left-decorated/reversed) so it can't
  leak into the right class name, and fills `ClassRelation.to_kind`. `kind`
  marks the `from` end (reversed), `to_kind` the `to` end; the renderer draws
  its marker as `marker-end`.
- Class **one-line body** `class Duck { +swim() }` opens and closes on the same
  line: `handle_class_decl` parses the inline members (shared `add_member_line`
  helper) and keeps the block **closed** instead of leaving `in_block` set — so
  the block no longer swallows every following statement. An empty `{}` closes
  with no members.
- Class `direction` (TD/BT/LR/RL) drives the transpose the same way the
  flowchart does.
- Class relation multiplicities (`A "1" --> "*" B`) parse into
  `ClassRelation.from_card`/`to_card`; the renderer draws them as small labels
  near each edge end. Token scanning is quote-aware so cards like `"1..*"`
  (which embed the `..` token) don't split the line.
- Class relation marker orientation: `ClassRelation.reversed` records whether
  the token's decorated end (triangle/diamond/circle/arrow) is on the left, at
  the `from` class — set by `is_reversed_token` for tokens opening with `<`,
  `*`, or `o` (`<|--`, `*--`, `o--`, `<--`, `<..`). `from`→`to` order (hence
  layout) is preserved; only the marker end moves. `style_for(kind, reversed)`
  emits the single decorated marker as `marker-start` (reversed) or `marker-end`
  (forward); `orient="auto-start-reverse"` points it into its node at either
  end. Composition/aggregation draw *only* the diamond — no far-end arrowhead.
- Class lollipop-interface `()` (`bar ()-- foo` / `foo --() bar`) is stripped
  off the token-adjacent side in `src/parse/class/relation.rs`
  (`split_trailing_lollipop`/`split_leading_lollipop`, before the multiplicity),
  keeping the class names clean and setting `ClassRelation.lollipop_from`/
  `lollipop_to`. The renderer overrides that end's marker with a hollow socket
  circle (`cls-lollipop`), so `()--|>` still draws the inheritance triangle at
  the far end plus the socket at the interface end.
- Class generics `~T~` are converted to angle brackets at render time
  (`convert_generics` in `src/svg/class/members.rs`) for class names and member/return
  types — `List~int~` → `List<int>`, nested `List~List~int~~` →
  `List<List<int>>`, `Map~string, int~` → `Map<string, int>` (innermost pair
  first; a lone unmatched `~` is left alone). The same `member_display` pass
  strips the trailing UML classifier (`*` abstract → `font-style="italic"`,
  `$` static → `text-decoration="underline"`).
- Class notes/annotations/labels/interactivity (`src/parse/class/`):
  `note "text"` (free) and `note for <Class> "text"` (attached) fill
  `ClassDiagram.notes` (`ClassNote { target, text }`); the renderer draws them
  as yellow sticky boxes in a row below the diagram, with a dashed connector to
  the target class. Standalone annotations parse in **either** order —
  `<<interface>> Shape` and `Shape <<interface>>` — via
  `parse_standalone_annotation`. A `class Name["label"]` sets `UmlClass.label`
  (the display text), keeping `name` clean — no phantom duplicate box.
  `click`/`link`/`callback` lines bind a `UmlClass.click` (reusing the flowchart
  `ClickAction`), parsed before the `:`-shorthand split so a URL's `https://`
  colon can't misroute the line. The **keyword drives the shape** (`split_interaction`
  keeps it): `callback` **always** binds a JS callback (a quoted first arg is the
  function name, never a URL), `link` always a hyperlink, `click` decides by the
  argument (`href`/`call`/quoted-URL/bare-name). The shared `open_click`/
  `close_click` wrappers live in `src/svg/interact.rs` (used by both the
  flowchart and class renderers).
