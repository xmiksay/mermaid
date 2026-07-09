use super::parse;
use super::tokens::parse_duration;
use crate::parse::ast::{TaskEnd, TaskStart, TaskStatus};

#[test]
fn full_example() {
    let s = "gantt\n\
             title My Project\n\
             dateFormat YYYY-MM-DD\n\
             section Planning\n\
             Design : a1, 2026-01-01, 5d\n\
             Review : after a1, 2d\n\
             section Execution\n\
             Build : crit, b1, 2026-01-08, 1w\n";
    let d = parse(s).unwrap();
    assert_eq!(d.title.as_deref(), Some("My Project"));
    assert_eq!(d.date_format.as_deref(), Some("YYYY-MM-DD"));
    assert_eq!(d.sections.len(), 2);
    assert_eq!(d.sections[0].tasks.len(), 2);
    let design = &d.sections[0].tasks[0];
    assert_eq!(design.id.as_deref(), Some("a1"));
    assert_eq!(design.end, TaskEnd::Duration(5.0));
    let review = &d.sections[0].tasks[1];
    match &review.start {
        TaskStart::AfterId(ids) => assert_eq!(ids, &["a1"]),
        _ => panic!("expected after id"),
    }
    let build = &d.sections[1].tasks[0];
    assert!(build.crit);
    assert_eq!(build.status, TaskStatus::Normal);
    assert_eq!(build.end, TaskEnd::Duration(7.0));
}

#[test]
fn end_date_form() {
    // `<start>, <end date>` instead of a duration.
    let d = parse("gantt\nsection S\nTask : a1, 2014-01-06, 2014-01-08\n").unwrap();
    let t = &d.sections[0].tasks[0];
    assert_eq!(t.id.as_deref(), Some("a1"));
    assert_eq!(t.start, TaskStart::Date("2014-01-06".into()));
    assert_eq!(t.end, TaskEnd::Date("2014-01-08".into()));
}

#[test]
fn duration_only_implicit_start() {
    // A single time token starts at the previous task's end.
    let d = parse("gantt\nsection S\nFirst : 2014-01-01, 5d\nanother task : 24d\n").unwrap();
    let t = &d.sections[0].tasks[1];
    assert_eq!(t.start, TaskStart::AfterPrevious);
    assert_eq!(t.end, TaskEnd::Duration(24.0));
}

#[test]
fn until_end_marker() {
    let d = parse("gantt\nsection S\nA : 2014-01-01, 5d\nB : after A, until A\n").unwrap();
    let b = &d.sections[0].tasks[1];
    assert_eq!(b.end, TaskEnd::UntilId("A".into()));
    // Single-token `until` form.
    let d = parse("gantt\nsection S\nA : 2014-01-01, 5d\nB : until A\n").unwrap();
    let b = &d.sections[0].tasks[1];
    assert_eq!(b.start, TaskStart::AfterPrevious);
    assert_eq!(b.end, TaskEnd::UntilId("A".into()));
}

#[test]
fn config_keywords_do_not_error() {
    let d = parse(
        "gantt\ndateFormat YYYY-MM-DD\ntickInterval 1week\ninclusiveEndDates\ntopAxis\nsection S\nA : 2014-01-01, 5d\n",
    )
    .unwrap();
    assert_eq!(d.sections[0].tasks.len(), 1);
    assert_eq!(d.tick_interval.as_deref(), Some("1week"));
    assert!(d.top_axis);
}

#[test]
fn weekend_weekday_display_mode_parse() {
    // Previously `weekend`/`displayMode` hard-errored; all four land on
    // their fields now.
    let d = parse(
        "gantt\ndateFormat YYYY-MM-DD\nweekend friday\nweekday monday\ndisplayMode compact\nsection S\nA : 2014-01-01, 5d\n",
    )
    .unwrap();
    assert_eq!(d.weekend.as_deref(), Some("friday"));
    assert_eq!(d.weekday.as_deref(), Some("monday"));
    assert_eq!(d.display_mode.as_deref(), Some("compact"));
    assert_eq!(d.sections[0].tasks.len(), 1);
}

#[test]
fn display_mode_colon_form() {
    // Upstream writes `displayMode: compact` with a colon.
    let d = parse("gantt\ndisplayMode: compact\nsection S\nA : 2014-01-01, 5d\n").unwrap();
    assert_eq!(d.display_mode.as_deref(), Some("compact"));
}

#[test]
fn milestone_tag_parsed_and_combinable() {
    let s = "gantt\nsection S\n\
             M1 : milestone, 2026-01-06, 0d\n\
             M2 : crit, milestone, m2, 2026-01-08, 0d\n";
    let d = parse(s).unwrap();
    let m1 = &d.sections[0].tasks[0];
    assert!(m1.milestone);
    assert_eq!(m1.status, TaskStatus::Normal);
    let m2 = &d.sections[0].tasks[1];
    assert!(m2.milestone);
    assert!(m2.crit);
    assert_eq!(m2.status, TaskStatus::Normal);
    assert_eq!(m2.id.as_deref(), Some("m2"));
}

#[test]
fn done_and_crit_combine() {
    // Upstream keeps the done status *and* the crit flag rather than
    // letting the last tag win.
    let d = parse("gantt\nsection S\nT : done, crit, 2026-01-01, 2d\n").unwrap();
    let t = &d.sections[0].tasks[0];
    assert_eq!(t.status, TaskStatus::Done);
    assert!(t.crit);
}

#[test]
fn duration_units_ms_s_m_h_d_w_month_year() {
    assert_eq!(parse_duration("2d"), Some(2.0));
    assert_eq!(parse_duration("1w"), Some(7.0));
    assert_eq!(parse_duration("12h"), Some(0.5));
    assert_eq!(parse_duration("720m"), Some(0.5));
    assert_eq!(parse_duration("1M"), Some(30.0));
    assert_eq!(parse_duration("1y"), Some(365.0));
    assert_eq!(parse_duration("86400s"), Some(1.0));
    assert_eq!(parse_duration("86400000ms"), Some(1.0));
    assert_eq!(parse_duration("1.5d"), Some(1.5));
    // `ms` isn't mis-read as minutes/seconds.
    assert_eq!(parse_duration("500ms"), Some(500.0 / 86_400_000.0));
    assert_eq!(parse_duration("nope"), None);
}

#[test]
fn month_year_units_do_not_hard_error() {
    let d = parse("gantt\nsection S\nA : 2026-01-01, 1M\nB : 2026-06-01, 1y\n").unwrap();
    assert_eq!(d.sections[0].tasks[0].end, TaskEnd::Duration(30.0));
    assert_eq!(d.sections[0].tasks[1].end, TaskEnd::Duration(365.0));
}

#[test]
fn vert_tag_is_a_flag_not_the_id() {
    let d = parse("gantt\nsection S\nDeadline : vert, v1, 2026-01-03, 0d\n").unwrap();
    let t = &d.sections[0].tasks[0];
    assert!(t.vert);
    assert_eq!(t.id.as_deref(), Some("v1"));
    assert_eq!(t.start, TaskStart::Date("2026-01-03".into()));
    assert_eq!(t.end, TaskEnd::Duration(0.0));
}

#[test]
fn time_only_start_is_a_date_not_an_id() {
    // `dateFormat HH:mm` values carry a `:`, so a leading time token is the
    // start date rather than being consumed as the task id.
    let d =
        parse("gantt\ndateFormat HH:mm\nsection S\nA : 09:00, 30m\nB : 10:00, 18:14\n").unwrap();
    let a = &d.sections[0].tasks[0];
    assert_eq!(a.start, TaskStart::Date("09:00".into()));
    match a.end {
        TaskEnd::Duration(days) => assert!((days - 30.0 / 1_440.0).abs() < 1e-12),
        _ => panic!("expected a duration end"),
    }
    // An explicit time end resolves to an end date, not an id/duration.
    let b = &d.sections[0].tasks[1];
    assert_eq!(b.start, TaskStart::Date("10:00".into()));
    assert_eq!(b.end, TaskEnd::Date("18:14".into()));
}

#[test]
fn after_accepts_multiple_predecessors() {
    let d = parse(
        "gantt\nsection S\nA : a, 2026-01-01, 5d\nB : b, 2026-01-01, 2d\nC : after a b, 1d\n",
    )
    .unwrap();
    match &d.sections[0].tasks[2].start {
        TaskStart::AfterId(ids) => assert_eq!(ids, &["a", "b"]),
        _ => panic!("expected after ids"),
    }
}

#[test]
fn click_binds_href_and_call_to_tasks() {
    let d = parse(
        "gantt\nsection S\nA : a, 2026-01-01, 5d\nB : b, 2026-01-06, 2d\nclick a href \"https://example.com\"\nclick b call openTask()\n",
    )
    .unwrap();
    use crate::parse::ClickAction;
    assert!(matches!(
        d.sections[0].tasks[0].click,
        Some(ClickAction::Href { .. })
    ));
    assert!(matches!(
        d.sections[0].tasks[1].click,
        Some(ClickAction::Callback { .. })
    ));
}

#[test]
fn auto_id_when_omitted() {
    let s = "gantt\nsection S\nA : 2026-01-01, 3d\nB : 2026-01-05, 2d\n";
    let d = parse(s).unwrap();
    let ids: Vec<_> = d.sections[0].tasks.iter().map(|t| t.id.clone()).collect();
    assert_eq!(ids, vec![Some("task1".into()), Some("task2".into())]);
}
