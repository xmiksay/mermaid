//! Gantt-chart AST types.

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GanttDiagram {
    pub title: Option<String>,
    pub date_format: Option<String>,
    pub axis_format: Option<String>,
    pub excludes: Vec<String>,
    pub today_marker: Option<String>,
    /// `weekend friday|saturday` — which two days `excludes weekends` skips.
    /// `friday` → Fri+Sat, anything else (default) → Sat+Sun.
    pub weekend: Option<String>,
    /// `weekday <day>` — the day the axis week starts on; aligns the first
    /// tick when a weekly `tickInterval` is in effect.
    pub weekday: Option<String>,
    /// `tickInterval Nday|Nweek|Nmonth` — axis tick spacing; overrides the
    /// automatic step picked from the total span.
    pub tick_interval: Option<String>,
    /// `displayMode compact` — packs parallel tasks into shared rows. Parsed
    /// and stored; the compact layout itself is a follow-up.
    pub display_mode: Option<String>,
    pub sections: Vec<GanttSection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GanttSection {
    pub name: String,
    pub tasks: Vec<GanttTask>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GanttTask {
    pub name: String,
    pub id: Option<String>,
    pub start: TaskStart,
    pub end: TaskEnd,
    pub status: TaskStatus,
    /// `crit` tag — draws a red border. Orthogonal to `status`: upstream
    /// combines it with `done`/`active` (e.g. `done, crit` = done fill + crit
    /// border) instead of letting the last tag win.
    pub crit: bool,
    /// `milestone` tag — rendered as a diamond at the start date; the end is
    /// ignored. Orthogonal to `status` (combinable with `done`/`active`/`crit`).
    pub milestone: bool,
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum TaskStart {
    Date(String),
    AfterId(String),
    AfterPrevious,
}

/// How a task's end is expressed: an explicit length, an end date, or an
/// `until <taskId>` marker that ends the bar where the named task starts.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum TaskEnd {
    /// Length in days (`Nd`/`Nw`/`Nh`/`Nm`).
    Duration(f64),
    /// Explicit end date (string in the diagram's `dateFormat`).
    Date(String),
    /// Ends when the named task starts (`until <taskId>`).
    UntilId(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum TaskStatus {
    #[default]
    Normal,
    Active,
    Done,
}
