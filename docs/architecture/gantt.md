# Gantt — architecture notes

Part of the [mermaid-svg architecture reference](../architecture.md).
Parser: `src/parse/gantt.rs` · Renderer: `src/svg/gantt.rs (+ src/svg/gantt_date.rs)`.

- Gantt dates are **exact civil day-counts from the Unix epoch**
  (`src/svg/gantt_date.rs`: `days_from_civil`/`civil_from_days`/`weekday`, the
  Hinnant algorithms) — no more `365.25`-day drift. `parse_date` honors the
  `dateFormat` field order (`DD-MM-YYYY` etc.), `format_date` renders axis
  ticks with the `axisFormat` d3/`strftime` subset (`%Y %m %d %b %a …`, default
  ISO `%Y-%m-%d`). `parse_datetime` extends `parse_date` to **fractional** day
  counts by reading sub-day tokens (`H` hour, lowercase `m` minute, lowercase
  `s` second, `S` ms) out of the `dateFormat`, so `dateFormat YYYY-MM-DD HH:mm`
  with a `2026-01-03 09:00` value no longer collapses to midnight; `parse_date`
  is just `parse_datetime(..).floor()`. Task start/end dates resolve through
  `parse_datetime`; `Excludes` still uses whole-day `parse_date`.
- **Time-only formats** (`dateFormat HH:mm`) are fully supported: `field_order`
  contributes just the present time tokens when no full date is found, and
  `parse_datetime` only requires as many numbers as the format has fields (2 for
  `HH:mm`), so `17:49` parses to a sub-day fraction off a fixed base day. The
  gantt parser's `looks_like_date` also treats a `:`-joined digit token as a
  date, so a leading `09:00` is read as the start rather than the task id, and an
  explicit `18:14` end becomes a `TaskEnd::Date`. When the whole chart spans less
  than a day the renderer drops the `0.5`-day minimum-bar floor (so a 2h and a
  90m task no longer render identically), lets the axis span the true sub-day
  range instead of stretching to one day, and `pick_tick_step` picks clean
  minute/hour tick intervals. `format_date` takes a **fractional** day and reads
  the fraction for `%H`/`%M`/`%S`, so a `HH:mm` axis shows real times, not `00`.
- Gantt `excludes` (weekends / weekday names / specific dates) is honored by
  the renderer via `Excludes` (`src/svg/gantt_date.rs`): each non-working day
  gets a light shading band behind the bars, and duration-based tasks are
  **stretched** over excluded days (`Excludes::stretched_end`, matching
  upstream's `getMaxEndTime`). Explicit end-date / `until` tasks are not
  stretched. `todayMarker` is a **CSS style string** upstream, not a date: the
  marker is always positioned at the *current* date (`today_days()` reads the
  system clock) and drawn only when it lands inside the chart's range — so a
  chart of past/future dates stays deterministic. `todayMarker off` suppresses
  it; any other value is passed through `css_style` (commas → `;`) onto the
  line's `style`; unset draws the default red dashed line.
- Gantt task tags are consumed as a leading run in `parse_task`: `active`/
  `done` set `TaskStatus`, while `crit`, `milestone` and `vert` are **orthogonal
  flags** (`GanttTask.crit`/`.milestone`/`.vert`) — upstream combines them, so
  `done, crit` keeps the done fill with a crit (red) border instead of the last
  tag winning. `colors_for(status, crit)` picks the fill from the status and a
  red border for `crit` (crit-only also takes the red fill). A `milestone`
  renders as a diamond (rotated square `<path>`) centered on the start date; a
  `vert` renders a dashed vertical marker line spanning the whole chart at the
  start date (label beside the line) — both ignore the duration. Adding a tag to
  the tag-match loop is what keeps it from being mis-consumed as the task **id**.
- Gantt task end is a `TaskEnd` enum (not a bare `duration_days`): `Duration`
  (units `ms`/`s`/`m`/`h`/`d`/`w`/`M`/`y`, decimals allowed — `parse_duration`
  matches `ms` before the single-char `m`/`s`; `M`/`y` approximated as 30/365
  days), `Date` (an explicit end date — the renderer computes the length from
  the resolved start), or `UntilId` (`until <taskId>` — ends where the named
  task *starts*, resolved against `id_to_start_end`). `parse_end` in
  `src/parse/gantt.rs` classifies the trailing time token; a task with a single
  time token (`X : 24d` / `X : until id`) implies `TaskStart::AfterPrevious`.
  `TaskStart::AfterId` holds a **`Vec<String>`**: `after a b c` starts at the
  *latest* end of the listed predecessors (unknown ids ignored, empty falls back
  to the previous task's end). `until`/end-date resolution happens in
  `resolve_tasks` (`src/svg/gantt.rs`), so forward/unknown refs fall back to a
  1-day length like `after` does. Config keywords `includes …`,
  `inclusiveEndDates`, `topAxis` are consumed in `parse()` (informational only)
  so they don't fall through to the task path.
- Gantt `click <taskId> href "url"` / `click <taskId> call fn()` binds a
  `GanttTask.click` (`ClickAction`, shared with the flowchart). Parsing reuses
  the shared `parse_click` in `src/parse/flowchart/click.rs` (also consumed by
  the state parser); clicks are collected into a map during `parse()` and bound
  to tasks by id
  afterward (a directive may precede or follow its task). The renderer wraps the
  bar/milestone/vert in the shared `open_click`/`close_click` (`svg/interact.rs`).
- Gantt `weekend friday|saturday`, `weekday <day>`, `tickInterval Nday|Nweek|Nmonth`
  and `displayMode[:] compact` are parsed via `strip_kw` (space- or colon-separated)
  into `GanttDiagram.{weekend,weekday,tick_interval,display_mode}` — previously
  `weekend`/`displayMode` hard-errored and `weekday`/`tickInterval` were dropped.
  Honored in `src/svg/gantt.rs`: `weekend` shifts the `excludes weekends` day pair
  (`weekend_days_for` in `gantt_date.rs`: `friday` → Fri+Sat, else Sat+Sun),
  `tickInterval` overrides `pick_tick_step` (`parse_tick_interval` → days), and
  `weekday` offsets the first axis tick onto that weekday (`weekday_tick_offset`).
  `display_mode` is stored but the compact row-packing layout is a follow-up.
