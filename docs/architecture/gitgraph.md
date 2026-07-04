# GitGraph — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/gitgraph.rs` · Renderer: `src/svg/gitgraph.rs`.

- gitGraph header (`src/parse/gitgraph.rs`) tolerates a trailing colon on both
  the keyword and the direction token — `gitGraph:`, `gitGraph TB:`,
  `gitGraph BT:` all parse (the dispatcher in `src/parse/mod.rs` also trims a
  trailing `:` off the head token). `branch <name> order: <n>` consumes the
  `order:`/`tag:` attributes instead of swallowing them into the branch name
  (`parse_branch`); `order` reaches `GitEvent::Branch.order`. The renderer
  (`src/svg/gitgraph.rs`) sorts lanes by explicit `order` (falling back to
  insertion order) and, for `BT`, flips the commit axis (`cols - 1 - col`) so
  newer commits sit higher. **Statement keywords match on a word boundary**
  (`keyword()`: the keyword must end the line or be followed by whitespace), so
  `commitxyz`/`branches foo` hard-error instead of masquerading as
  `commit`/`branch`. Branch names are **unquoted everywhere** a `(REFERENCE |
  STRING)` is allowed — `branch`/`checkout`/`switch`/`merge` all route the name
  through `take_value`, so `branch "feat x"` + `checkout "feat x"` +
  `merge "feat x"` reference one lane.
- gitGraph **config directives** (`config.gitGraph.*`, from `%%{init}%%` or
  frontmatter `config:`) flow through the preamble: `preamble.rs` fills a
  `DiagramMeta.git_graph` (`GitGraphMeta`, all-`Option`), and
  `parse_with_meta` overlays them onto `GitGraphDiagram.config`
  (`GitGraphConfig`, whose `Default` keeps upstream's own defaults). Honored:
  `mainBranchName` (initial/default branch — the renderer no longer hardcodes
  `"main"`), `showBranches` (branch labels + lane lines), `showCommitLabel`
  (per-commit id label), `rotateCommitLabel` (rotates the id label -45° in the
  horizontal layout), `parallelCommits` (`assign_col`: a commit's column is one
  past its deepest parent so independent branches can share a column, instead of
  a strictly advancing global counter), and `mainBranchOrder`
  (`GitGraphConfig.main_branch_order`, seeded into `branch_orders[0]` so main
  sorts among the `order:`-ed lanes instead of being pinned to lane 0).
- gitGraph `merge` and `cherry-pick` render with **dedicated glyphs** (no longer
  reusing `Highlight`/`Reverse`): `CommitKind::Merge` → double concentric circle
  (`draw_merge_glyph`), `CommitKind::CherryPick` → the two-cherry glyph
  (`draw_cherry_pick_glyph`). `cherry-pick id:"x" parent:"y" tag:"t"` keeps its
  `parent`/`tag` on `GitEvent::CherryPick` (`parse_cherry_pick_attrs`); the tag
  renders as the node label. `commit`/`merge` share `parse_commit_attrs(s,
  default_kind)`: `tag:` **accumulates into a `Vec`** (`GitEvent::{Commit,
  Merge}.tags`, upstream `tags+=STRING`; the renderer stacks them upward), and a
  `merge <branch> type: NORMAL|REVERSE|HIGHLIGHT` overrides the merge glyph via
  `GitEvent::Merge.kind` (default `CommitKind::Merge`).
