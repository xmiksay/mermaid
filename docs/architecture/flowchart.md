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

`route_edges` (`mod.rs`) resolves every edge once into an `EdgePath`
(`src/svg/flowchart/route.rs`: drawn control points, label polyline, curve,
style); the canvas measurement and the drawing both use it.

- **Centre space first, clip last.** Control points start from the routed
  polyline (node centres plus dummy waypoints). A two-point edge gets the S
  points `[S, A_s, A_t, T]` from `s_points`: the bend axis comes from the
  **rank relation** (flow-axis comparison of the centres, with the flow axis
  of the innermost subgraph holding both ends — `edge_horizontal` in
  `directions.rs` — else the diagram's), never from the geometry, so wide
  boxes cannot flip an inter-rank edge onto a side face. Forward and back
  edges bend across the flow axis mid-way through the gap between the facing
  faces; same-rank or aligned ends draw straight. Each end is then clipped
  from its adjacent control point toward the node centre: rects are entered
  square-on, circles and rhombi are aimed at the centre.
- **Straight under the marker.** Each marked end gets one collinear lead-in
  point on its clip ray, marker reach × stroke width + 4px out from the face
  (`marker_reach` mirrors the marker defs). In a tight gap the bend moves
  toward an unmarked end to make room; a gap too tight for both lead-ins draws
  straight. Unmarked ends get no lead-in.
- Same-kind fan-ins meet in one arrowhead, as upstream.
- **Unchanged (clip-first, master's route):** opposite pairs (the
  `bow_opposite_pairs` routes), self-loops, cluster (subgraph-id) ends and
  `linear`/`step` curves keep `clip_edge_polyline` + the dominant-axis
  `basis_control_points`.

## Canvas & subgraph frame sizing

- Canvas sizing is a proper **measure-then-shift** pass: every edge's
  `EdgePath` and label position is resolved before drawing, a true bounding
  box is computed over node, subgraph-frame, label rects and every edge's
  control points (the basis curve stays inside their hull), then everything is
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
