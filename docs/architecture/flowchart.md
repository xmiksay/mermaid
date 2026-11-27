# Flowchart — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/flowchart/` · Renderer: `src/svg/flowchart/`.

## Pipeline (important)

Direction transform in `src/svg/flowchart/mod.rs`: sugiyama only knows
top-down, so for `LR`/`RL` we **swap input sizes** `(w, h) → (h, w)` and
**output coordinates** `(sx, sy) → (sy, sx)`. For `BT`/`RL` we flip the axis.

Edge clipping (`clip_to_node`, in `src/svg/flowchart/edges.rs`) has per-shape variants:
- rect: `t = min(hw/|dx|, hh/|dy|)`
- circle: normalize to radius
- rhombus: `t = 1 / (|dx|/hw + |dy|/hh)`
- other shapes fall back to rect

## Edge geometry (#402)

`route_edges` (`mod.rs`) gathers every edge into an `EdgeIn` and `route_all`
(`src/svg/flowchart/route.rs`) resolves them once into `EdgePath`s (drawn
control points, label polyline, curve, style); the canvas measurement and the
drawing both use them.

- **Centre space first, clip last.** Control points start from the routed
  polyline (node centres plus dummy waypoints) and each end gets one
  **approach point** on its flow face (`approaches`):
  - a two-point edge gets `[S, A_s, A_t, T]`; the bend axis comes from the
    **rank relation** (flow-axis comparison of the centres, with the flow
    axis of the innermost subgraph holding both ends — `edge_horizontal` in
    `directions.rs` — else the diagram's), never from the geometry, so wide
    boxes cannot flip an inter-rank edge onto a side face. Forward and back
    edges bend mid-way through the gap between the facing faces
    (`pair_depth`); same-rank ends join straight;
  - a routed edge's marked end gets its approach point on the node's own
    column, just past its lead-in (`end_approach`). It stays that shallow,
    not mid-way to the dummy, because the last dummy can sit far to the side
    and a deep square run would swing the curve through neighbouring nodes.
    An unmarked routed end has no head to square up and keeps the ray from
    its dummy, leaving the node on the side it heads for.

  Each end is then clipped from its approach point: a rect is entered
  square-on, so same-kind heads on one face coincide exactly. Ends without an
  approach point keep the plain clip from their neighbour.
- **Round ends** (circle family, rhombus; `src/svg/flowchart/round.rs`) take
  no approach point and no face slot. As upstream, each is clipped on the ray
  from its neighbouring control point toward the centre, so arrows spread
  around the shape by where they come from instead of hooking into the top
  arc; its lead-in lies on that same ray. Afterwards `nudge_round_tips` —
  on every edge except self-loops, master-route edges included — turns an
  end that would land within both half-widths + 4 of an earlier,
  different-kind end on the same node around the centre in 1° steps until
  clear (nearest angle first, up to 90°), its lead-in following onto the
  line to its neighbour. Placed tips are bucketed by kind and angle, so each
  step checks only nearby tips of another kind. Heads are placed before bare
  line ends, so a line turns away from a head, never the reverse. Same-kind
  tips may coincide.
- **Straight under the marker.** Each marked end gets one collinear lead-in
  point on its clip ray, marker reach × stroke width + 4px out from the face
  (`marker_reach` mirrors the marker defs). In a tight gap the bend moves
  toward an unmarked end to make room; a gap too tight for both lead-ins draws
  straight. Unmarked ends get no lead-in. The stroke width comes from
  `linkStyle` only when it is a finite number ≥ 0 (`EdgeIn::stroke`).
- **Slots by kind** (`src/svg/flowchart/slots.rs`, rect-like nodes). Ends on
  one face of one node are grouped by kind: the marker (arrow / circle / cross) or bare.
  Same-kind ends coincide, as upstream (fan-ins meet in one arrowhead).
  Different kinds sit side by side, centred on the face, neighbours half of
  each one's marker width + 4 apart, ordered by the mean cross coordinate
  their edges come from so they do not cross. Different kinds are never
  stacked, and no tip ever leaves its node: on a face too narrow the spacing
  is compressed as far as needed to keep every slot centre 4px inside the
  face's corners, so markers wider than the face overlap rather than detach
  (`row`). For this,
  a marker's width is capped at 1e4px, so an absurd `linkStyle`
  `stroke-width` cannot overflow the arithmetic. The offset moves the
  approach point, lead-in and clip together.
  **Fixed ends** — ends without an approach point (plain route, straight,
  unmarked routed) — keep their place and take no slot: the movable row is
  laid out as if they were absent and only moves aside, just far enough,
  where a slot would land within both half-widths + 4 of where a fixed end
  actually lands (`clear_fixed`), and never so far that a slot leaves the
  face. A lone movable end therefore stays centred, and an aligned edge whose
  end had to move takes its other end along (within the narrower face) so it
  stays straight.
- **Unchanged (clip-first, master's route):** opposite pairs (the
  `bow_opposite_pairs` routes), self-loops, cluster (subgraph-id) ends,
  `linear`/`step` curves, and two-point edges spanning more than two layer
  gaps (`skips_ranks`: a local `direction` replaced a cross-lane route with a
  bare line, and an S through the skipped ranks would run through their
  nodes) keep the clip-first `geometry::clip_polyline` + the dominant-axis
  `basis_control_points` (then detoured like every other edge, below).

## Detours (#403)

`detour_all` (`src/svg/flowchart/detour.rs`) runs in `route_all` after the
routes are built and **before** the round-tip nudge (so a re-aimed round end
is nudged like any other), on every edge but self-loops — every curve kind
and the plain routes included. Modules: `detour/search.rs` (the rounds and
the scoring), `detour/cands.rs` (the ways round), `detour/room.rs` (end
runs, re-aiming), `detour/body.rs` (node outlines, segment tests, the
outline's reach past a line), `detour/scene.rs` (nodes and markers,
indexed), `detour/index.rs` (bands about one box thick, as rows and
columns, each sorted along the other axis; a plain list below 48 boxes),
`detour/lines.rs` (other edges and frame borders, for scoring).

- **Trigger.** An edge whose control points' bounding box (a basis curve
  stays inside it) overlaps no node it does not own is skipped without
  sampling. Otherwise the drawn curve (basis at 8 steps per bezier, step
  curves as drawn) is tested segment by segment against the outline (rect,
  circle, rhombus) of every node it does not end on. A subgraph end owns
  the nodes inside its frame only when the other end lies inside that frame
  too (a member to its own frame, nested frames): an edge from outside to a
  subgraph keeps off its members.
- **Ways round** the first node hit, least sideways movement first:
  one waypoint or a pair beside the stretch the curve crosses it on,
  perpendicular to that crossing (entry–exit chord), moved by how far the
  node's outline reaches past that chord *near the crossing* plus half a
  node gap (`LayoutConfig::node_gap`) — clipping a wide node's corner moves
  the curve by the corner, not by half the node; then a gap out. Only when
  none of those clears the node without coming close to another: 2 gaps
  out, and waypoints out from the node's corners (one corner, or a side
  past two) — what reaches a far end sitting behind a wide node, going
  round its end. Step curves instead get a pair of waypoints on one
  vertical (or horizontal) line, a clearance off the node's box, which makes
  that line the step's leg. Waypoints replace at most one existing control
  point, clamped so the curve never runs backwards and never within 10px of
  a kept point.
- **End runs.** A marked rect end keeps its face point, lead-in and the
  point beyond, plus points on the same ray out to two lead-ins (straight
  under the marker; dropped again when the detour does not start right
  after them). A run that would come within 6px of a foreign node (a thick
  stroke's lead-ins grow with it) is dropped and laid again, full length,
  on the ray toward the detour — the head then meets its face at an angle
  but stays straight. A free end — round, or unmarked — is clipped again on
  the ray from its new neighbour (`reaim`), lead-ins on that ray; a rect
  tip stays 8px off the corners, and a bare rect end keeps its old tip when
  the new way out would come within 6px of a node or land on a head already
  there. Linear and step curves keep only the tips.
- **Rules.** A candidate may not run through a node the curve did not, nor
  (step, linear) come within 6px of one; may not run back against the flow
  axis by more than 10px more than before (round a wide node's end it may
  run back *across* the flow), nor grow past twice the edge's length plus a
  gap. Only the stretch that changed is re-tested. When no candidate is
  clean, one that trades the node for a single node not passed before is
  taken (also over a clean one that comes close to a node) and the next
  round goes round that one; an edge gets at most 6 rounds. The result is
  kept only if it runs through fewer nodes than the input (or as many and
  grazes fewer).
- **Score** = sideways movement + 1000 per node the moved stretch comes
  within 4px of (rhombi grown face by face) + 40 per node newly grazed
  within 6px + 150 per other edge's marker newly run over + 4 per px of
  running along another edge (near the node) or any frame border (within
  6px, nearly parallel, stretches of 30px or more). A way that comes within
  4px is pushed off what it comes close to, 3 to 22px, before it is
  accepted. The shorter side wins unless it traces another line; one clean
  crossing beats a traced stretch. Candidates are built and scored lazily,
  best bound first.
- After a detour the edge's marker discs are replaced in the scene.
  Iteration is in edge and node order, so output is deterministic. The
  label polyline becomes the detoured control points.

## Canvas & subgraph frame sizing

- Canvas sizing is a proper **measure-then-shift** pass: every edge's
  `EdgePath` and label position is resolved before drawing, a true bounding
  box is computed over node, subgraph-frame, label rects and every edge's
  drawn curve (`EdgePath::drawn_points`), then everything is
  shifted so the measured minimum lands at `CANVAS_PAD` — otherwise a
  local-`direction`-shifted subgraph frame or a BOW-routed back-edge label
  could land at a negative/overflowing coordinate and get clipped by the
  viewBox.
- `compute_subgraph_boxes` (`src/svg/flowchart/nodes.rs`) sizes each frame
  **bottom-up** (deepest nested subgraph first): a frame pads around the union
  of its own direct members' bounds and its direct children's *already-padded*
  frames, guaranteeing at least `SUBGRAPH_PAD` between a child's frame and its
  parent's — flattening straight through to every recursive leaf (the old
  approach) let a nested child sharing an extremal node with its ancestor pad
  outward from that same node, leaving zero visible margin between the two
  frames.
- `resolve_subgraph_overlaps` (`src/svg/flowchart/nodes.rs`) nudges apart any
  two **sibling** (non-nested) subgraph frames that still collide after
  local-direction transposition — sugiyama itself has no notion of subgraph
  frame padding, only per-node `node_gap`.
- `apply_local_directions` processes subgraphs innermost-first: direct member
  nodes transpose individually for a `direction` override, but a direct child
  subgraph moves as one rigid block (translated, not re-swapped) so its own
  local direction survives an ancestor's differing transpose instead of being
  overridden by it.
- The `[(text)]` database/cylinder shape (`cylinder` in
  `src/svg/flowchart/shapes.rs`, shared by the plain database shape and the
  lined-disk shape) is one continuous closed path — a native `<ellipse>` top
  cap (avoids a faint rasterization seam where two half-arc endpoints would
  otherwise be exactly `2*rx` apart) plus a deliberately-unclosed body path
  (straight sides + bottom arc) sharing the same fill, so it reads as one
  seamless cylinder instead of a filled rect body with a separate stroked
  seam arc doubling its border. `SvgBuilder::ellipse` (`src/svg/builder/mod.rs`)
  is the shared native-ellipse helper.

## Things to remember

- Flowchart **header aliases**: `parse_direction` also accepts upstream's `<dir>`
  symbol aliases — `>`=LR, `<`=RL, `^`=BT, `v`=TB (`graph >`). The
  `flowchart-elk` header (upstream's ELK-layouter selector) is dispatched to the
  flowchart parser and laid out with sugiyama, matching the `layout: elk`
  config's layout-deviation tolerance.
- Flowchart `;` is a **statement terminator/separator** anywhere a newline is
  accepted (upstream grammar). `parse()` flattens each source line into its
  `;`-separated statements via `split_semicolons` before dispatch, so `graph
  TD;`, `A-->B;`, and `graph LR; A-->B` (header + statements on one line) all
  parse. A `;` inside a quoted string, a shape bracket, or an edge-label `|…|`
  run is left intact (so `["a;b"]` and `#59;` entity codes survive).
- Flowchart **scanner robustness** (upstream NODE_STRING / string-lexer parity):
  `read_ident` (`src/parse/flowchart/scanner.rs`) keeps a `-`/`/` inside a node id
  (`a-node`, `x/y`) — consumed only when the next char continues the id, so an
  arrow opener (`-->`, `-.->`) still breaks it. Shape/label text and pipe labels
  scan **quote-aware** (`read_until_unquoted`/`find_unquoted`), so a quoted label
  may embed its own closer: `A["a ] b"]`, `A("call (x)")`, `-->|"a|b"|`. A `%%`
  inside a `"…"` label is content, not a comment — `strip_comment`
  (`src/parse/mod.rs`) uses `find_unquoted` (fix shared by every diagram).
  A bracket-less `subgraph one two three` keeps all its words as the id (renderer
  shows it) instead of truncating at the first space; `id [Label]` still splits on
  the `[`. `click A call handler(arg one, arg two)` keeps the whole `(…)` argument
  list (`click_tokens` tracks paren depth, upstream CALLBACKARGS `[^)]*`).
- Flowchart `~~~` is the **invisible link** (`EdgeLine::Invisible`): `parse_arrow`
  accepts `~` as an opener, requires ≥3 tildes, and forbids any head/tail. It is
  a real edge (so it shapes the sugiyama layout) but `draw_edge` returns early
  for `Invisible`, drawing nothing. A `~`/`~~` run under 3 is not an edge.
- Flowchart `FlowEdge` has separate `line` (Solid/Dotted/Thick), `head`
  (None/Arrow/Circle/Cross), and `tail` (start-side head, same enum) — covers
  `-->`, `---`, `-.->`, `==>`, `--o`, `--x` plus all no-head variants, and the
  bidirectional forms `<-->`, `o--o`, `x--x` (`tail` set). `parse_arrow` reads
  an optional leading `<`/`o`/`x` before the line dashes; `o`/`x` count as a
  tail marker only when a line char (`-`/`=`/`.`) immediately follows, so a
  bare node id like `o` stays a node. The renderer emits `marker-start` (the
  markers' `orient="auto-start-reverse"` flips them to point outward).
- Flowchart edge labels come in two forms: the pipe form `A -->|text| B` and
  the inline form `A -- text --> B` (also `-. text .->`, `== text ==>`). The
  inline form is recognized in `parse_arrow` via `read_inline_label`: a
  two-char opener (`--`/`-.`/`==`) with no head, followed by text and a
  matching closer, captures the text as the edge label instead of a chain
  node. A head-less solid/thick closer needs ≥3 connectors so a plain
  `A -- B -- C` chain is left untouched.
- `A & B --> C & D` produces 4 edges (cross product) — multi-source/target.
- Flowchart node `(text)` (round) renders as a small `rx="5"` rounded rect;
  only stadium `([text])` is a full pill (`rx = h/2`) — the two shapes are
  visually distinct (`draw_node`).
- Flowchart `subgraph` is tracked in `FlowchartDiagram.subgraphs` including
  nesting. The renderer draws a solid rounded cluster frame with the themed
  `flow_cluster_fill`/`flow_cluster_stroke` and a centered normal-weight label
  set just inside the top border (`draw_subgraphs`) — upstream draws it in
  normal weight below the border, not bold on it (#331).
  - `style <id>`/`class <id> <name>` naming a subgraph id styles the cluster
    frame: the directive lands on the phantom node dropped during subgraph-id
    cleanup, so the parser moves its `style`/`classes` onto `Subgraph.style`/
    `Subgraph.classes` first; the renderer resolves them through the shared
    `resolve_style` (fill/stroke override the theme cluster colors).
  - Mermaid v11 edge ids and attributes: the `e1@` prefix in `A e1@--> B`
    (`consume_edge_id`) names the edge — recorded in an `edge_ids` set *and*
    stored on `FlowEdge.id` — and a standalone `e1@{ animate: …, curve: … }`
    statement (`edge_attr_stmt`) applies those attributes to the matching edge
    (`apply_edge_attrs`) instead of spawning a phantom node. `animate: true`
    sets `FlowEdge.animate` (a SMIL `<animate>` on `stroke-dashoffset` in
    `draw_edge`, needing a dash pattern — falls back to `8 8`); `curve: <name>`
    sets `FlowEdge.curve` (`EdgeCurve::from_name`). `linkStyle N interpolate
    <curve>` / `linkStyle default interpolate <curve>` fill
    `FlowchartDiagram.edge_interpolate`/`default_interpolate`, while
    `config.flowchart.curve` (frontmatter/`%%{init}%%`) fills
    `FlowchartDiagram.config_curve` (the diagram-level default). The renderer
    resolves the effective curve per edge — `@{ curve }` → per-index
    interpolate → default interpolate → `config_curve` → basis — and `curve_basis_path`,
    `curve_linear_path` (straight segments), and `curve_step_path` (orthogonal
    right-angle steps) in `src/svg/builder/curves.rs` build the path. Any other
    upstream curve name (`cardinal`, `natural`, …) falls back to basis.
  - Under the default basis curve, a *bare two-point* edge between offset
    nodes leaves and re-enters its faces as a gentle S, matching upstream
    instead of a straight diagonal (#331); see "Edge geometry (#402)" above
    for how the S is built. Endpoints that share a column/row draw straight.
  - `direction X` inside a subgraph body fills `Subgraph.direction`. The
    renderer works in screen space and, for a cluster whose flow axis differs
    from the diagram's, transposes just that cluster's members (and their
    internal edges) about the cluster centre (`apply_local_directions`) — a TD
    chain inside a `direction LR` subgraph becomes a horizontal row.
  - An edge endpoint naming a subgraph id refers to the cluster, not a node.
    The parser drops any node materialized for a subgraph id (forward ref or
    edge target); the renderer routes such an edge as a straight connector
    clipped to the cluster bounding box (`endpoint_clip` → `EndClip` with a
    `None` shape → rectangle clip).
- Flowchart `click <id>` sets `FlowNode.click` (`ClickAction::Href` for
  `"url"`/`href` forms, `ClickAction::Callback` for a bare name/`call fn()`).
  The renderer wraps hyperlink nodes in `<a href>` and callback nodes in a
  `<g class="clickable" onclick>`; an optional tooltip becomes a `<title>`.
- Asymmetric flowchart shapes are fully supported: parallelogram `[/text/]`,
  parallelogram-alt `[\text\]`, trapezoid `[/text\]`, trapezoid-alt
  `[\text/]`, and the asymmetric flag `>text]` — parsed in
  `src/parse/flowchart/node/` and rendered in `src/svg/flowchart/nodes.rs`.
  The flag mirrors upstream `rect_left_inv_arrow`: a concave notch on the left
  edge and a straight vertical right edge (not a right-pointing arrow).
- Flowchart v11 attribute syntax `id@{ shape: …, label: … }` is handled in
  `parse_at_node` (`src/parse/flowchart/node/spec.rs`): the `@{…}` block right after a
  node id is split into `key: value` pairs (quote-aware comma/colon split), the
  `shape` name mapped onto a `NodeShape` by `shape_from_name` (aliases like
  `rounded`/`diam`/`cyl`/`lean-r`/`trap-b`/`dbl-circ`/`subproc`), and
  `label`/`title` set the node text. `icon`/`img` forms are dropped but their
  `label` is preserved so content is never lost. Beyond the classic geometries,
  ~19 v11 shapes have their own `NodeShape` variant and are drawn in
  `src/svg/flowchart/shapes.rs` (kept out of `nodes.rs`; `draw_node` delegates
  its non-classic arm there): `notch-rect`/`card`, `doc`, `docs`, `tag-doc`,
  `bolt`, `hourglass`, `comment`/`braces`, `delay`, `das` (horizontal cylinder),
  `lin-cyl`/`disk`, `lin-rect`, `div-rect`, `win-pane`, `tri`, `flip-tri`,
  `f-circ`, `cross-circ`, `paper-tape`, `bow-rect`/`stored-data` (+ their
  aliases). The round ones (`f-circ`/`cross-circ`) get a circle edge-clip; every
  other new shape uses the rect-boundary clip. Names still without a variant
  (e.g. `text`, `fork`, `sm-circ`) fall back to Rect.
