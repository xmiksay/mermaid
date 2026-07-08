# ZenUML — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/zenuml/` · Renderer: `src/svg/sequence/zenuml.rs`
(a ZenUML-chrome pass over the shared sequence layout).

- zenuml (`src/parse/zenuml/`: `mod.rs` header/tokenize/dispatch + declarations,
  `message.rs` calls/returns/assignment, `blocks.rs` if/try chains) is a
  **brace-structured** translation to a
  `SequenceDiagram` with its `zenuml` flag set. After the `zenuml` header the
  body is `tokenize`d into `{`/`}`/statement `Tok`s (braces inside `(…)`/quotes
  stay literal; `\n`/`;` end statements; `//` and `%%` are comments), then a
  recursive `Parser::parse_items(ctx, ret)` walks them. `ctx` is the current
  caller (the enclosing method's *receiver*, or the top-level starter); `ret` is
  who a `return` replies to.
  - Annotators: `@Actor X` declares an actor; `@Boundary`/`@Control`/`@Entity`/
    `@Database X` declare the matching UML stereotype (each drawn with its own
    glyph by `draw_stereotype` in `src/svg/sequence/glyphs.rs` — boundary
    circle-with-bar, control arrow-circle, entity underlined circle, database
    cylinder); any other `@Type X` is a plain participant. `@Starter(X)` sets the
    top-level caller. A bare/`A.method()` call with no explicit `A -> B` source
    originates from the starter — a synthetic `Starter` lane, created lazily,
    when none is declared.
  - Participant declarations (`try_declaration` in `mod.rs`): a bare identifier
    `Bob` declares the participant, and `A as Alice` is an alias (id `A`,
    displayed `Alice` — `split_alias`, quoted display allowed). Declaration
    order is column order. A statement carrying `(` or `->` is never a
    declaration (it stays a call), so these no longer fall through to
    `parse_call` and materialize as phantom Starter self-messages.
  - `new A1` / `new A2(with, parameters)` (`parse_new` in `message.rs`)
    materialize the participant and emit a `SequenceItem::Create` plus a
    `«create»` creation message from the current context — no longer a Starter
    self-call.
  - Method calls carry a context: `Recv.method()` → `ctx -> Recv`, `method()`
    (no dot) is a self-call on `ctx`. Every invocation (a call whose text carries
    `(`, or any call opening a `{ … }` body) brackets the receiver with
    `Activate`/`Deactivate` items, so the renderer draws a **nested activation
    bar** encoding call depth. A `{ … }` body runs in the receiver's context; on
    close the receiver deactivates. **Synthesized returns are suppressed** — the
    activation ending *implies* the reply, so only *labeled* returns are drawn:
    an `x = call()` assignment draws a dashed return labeled `x` (self-calls get
    none). A **typed** assignment `SomeType a = A.m()` (`split_assignment` accepts
    a multi-word identifier LHS) labels the return with the trailing variable
    (`a`), not a participant named `SomeType a = A`.
  - `return <v>` (and the `@return`/`@reply <v>` annotation aliases) emits a
    dashed reply from `ctx` to `ret`; a caller-less bare-value `return` (no
    enclosing method-call body) is a `ParseError::Syntax`, not silently dropped.
    The explicit directed form `return A -> B: result` / `@return A -> B: result`
    (upstream reply form 3) emits a dashed `A`→`B` message and is valid at top
    level (no enclosing caller needed). Control structures map onto existing
    `SequenceItem` frames: `if/else if/else` → `Alt`,
    `while/for/forEach/foreach/loop` → `Loop`, `opt` → `Opt`, `par` → `Par`,
    `try/catch/finally` → `Critical` (catch/finally as option branches). The
    `else`/`catch`/`finally` chain tokens are consumed by their opener's handler.

## Renderer (`src/svg/sequence/zenuml.rs`)

`sequence::render` dispatches to `zenuml::render` when `SequenceDiagram.zenuml`
is set. It reuses the shared layout pass (`layout_items`), activation bands
(`draw_activations`), and message drawing, but swaps in ZenUML chrome:

- **Hierarchical numbering** — `number_calls` walks the event list keeping an
  activation-depth counter stack: each forward call (solid arrow) increments the
  counter at its level and its `Activate` pushes a fresh child level, yielding
  `1`, `1.1`, `1.1.1`, `1.2`…. Dashed returns are not numbered.
- **Top-only participants** — `draw_participant` frames each stereotype/actor
  glyph in a bordered box drawn once at the top; there is no bottom actor row.
- **Fragment chrome** — `draw_block_frames(…, zenuml=true)` draws a shaded header
  band with the operator + condition (instead of a corner tab) and shades the
  `else`/`catch` region.
- **Title frame** — the whole diagram is wrapped in a frame whose top-left tab
  carries the title, rather than a centered heading. Lifelines are solid.
