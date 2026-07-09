use super::axis::{parse_tick_interval, weekday_tick_offset};
use super::*;
use crate::parse::parse;

fn build(s: &str) -> GanttDiagram {
    match parse(s).unwrap() {
        crate::parse::Diagram::Gantt(g) => g,
        _ => panic!("not gantt"),
    }
}

#[test]
fn renders_basic() {
    let d = build(
        "gantt\ntitle My Plan\ndateFormat YYYY-MM-DD\nsection Phase 1\nDesign : a1, 2026-01-01, 5d\nReview : after a1, 2d\n",
    );
    let svg = render(&d, &Theme::default());
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains(">My Plan<"));
    assert!(svg.contains(">Phase 1<"));
    assert!(svg.contains(">Design<"));
    assert!(svg.contains(">Review<"));
}

#[test]
fn draws_section_background_bands() {
    // Two sections → two full-width bands with the first two band styles.
    let d = build(
        "gantt\ndateFormat YYYY-MM-DD\nsection Design\nSpec : 2026-01-01, 5d\nsection Build\nBackend : 2026-01-08, 5d\n",
    );
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("fill=\"#6666ff\" fill-opacity=\"0.098\""));
    assert!(svg.contains(">Design<"));
    assert!(svg.contains(">Build<"));
    // Section labels use upstream regular weight, not bold (#332).
    assert!(!svg.contains("font-weight=\"bold\">Design<"));
    assert!(!svg.contains("font-weight=\"bold\">Build<"));
}

#[test]
fn narrow_gutter_scales_to_section_names_only() {
    // A chart with long task names but short section names keeps a narrow
    // gutter — task names no longer inflate the left column.
    let short = build("gantt\ndateFormat YYYY-MM-DD\nsection S\nA very long descriptive task name here : 2026-01-01, 5d\n");
    // Gutter is sized from the (short) section name, not the long task name.
    assert!(section_gutter(&short) <= LABEL_GUTTER_MIN + 1e-6);
}

#[test]
fn short_task_label_sits_right_of_bar() {
    // A 1-day bar in a 40-day span is too narrow for its name, so the label
    // is drawn to the right (left-anchored), not inside (middle-anchored).
    let d = build("gantt\ndateFormat YYYY-MM-DD\nsection S\nKickoff task : 2026-01-01, 1d\nTail : 2026-02-10, 1d\n");
    let svg = render(&d, &Theme::default());
    assert!(svg.contains(">Kickoff task<"));
    // No left-column duplicate: the name appears exactly once.
    assert_eq!(svg.matches(">Kickoff task<").count(), 1);
}

#[test]
fn task_after_id_starts_right_of_predecessor() {
    let d = build("gantt\nsection S\nA : a, 2026-01-01, 5d\nB : after a, 3d\n");
    let resolved = resolve_tasks(&d);
    assert!(resolved[1].start_day >= resolved[0].start_day + resolved[0].duration - 1e-6);
}

#[test]
fn milestone_renders_diamond_not_bar() {
    let d = build("gantt\nsection S\nKickoff : milestone, 2026-01-01, 0d\n");
    let svg = render(&d, &Theme::default());
    // Diamond is drawn as a <path> with a Z-closed rhombus, no bar <rect>.
    assert!(svg.contains("<path"));
    assert!(svg.contains(">Kickoff<"));
}

#[test]
fn end_date_sets_bar_length() {
    // Two days between 2014-01-06 and 2014-01-08.
    let d = build("gantt\ndateFormat YYYY-MM-DD\nsection S\nT : a1, 2014-01-06, 2014-01-08\n");
    let resolved = resolve_tasks(&d);
    assert!((resolved[0].duration - 2.0).abs() < 1e-6);
}

#[test]
fn until_ends_at_referenced_task_start() {
    // B starts four days before A and runs `until A`, so it ends where A
    // starts — a 4-day bar.
    let d = build("gantt\ndateFormat YYYY-MM-DD\nsection S\nA : a, 2014-01-05, 5d\nB : b, 2014-01-01, until a\n");
    let resolved = resolve_tasks(&d);
    assert!((resolved[1].duration - 4.0).abs() < 1e-6);
    assert!((resolved[1].start_day + resolved[1].duration - resolved[0].start_day).abs() < 1e-6);
}

#[test]
fn crit_uses_red_palette() {
    let d = build("gantt\nsection S\nUrgent : crit, 2026-01-01, 1d\n");
    let svg = render(&d, &Theme::default());
    // Solid-red fill with the light-red crit border (upstream default).
    assert!(svg.contains("fill=\"red\""));
    assert!(svg.contains("#ff8888"));
}

#[test]
fn done_crit_keeps_done_fill_with_red_border() {
    let d = build("gantt\nsection S\nT : done, crit, 2026-01-01, 2d\n");
    let svg = render(&d, &Theme::default());
    // Done (light-grey) fill + crit red border, not the crit red fill.
    assert!(svg.contains("#d3d3d3"));
    assert!(svg.contains("#ff8888"));
}

#[test]
fn normal_bar_uses_purple_palette() {
    let d = build("gantt\ndateFormat YYYY-MM-DD\nsection S\nSpec : 2026-01-01, 5d\n");
    let svg = render(&d, &Theme::default());
    // Upstream default task fill/border are the purple family, not the old
    // light-blue; the fitting label is drawn in white ink over the bar.
    assert!(svg.contains("fill=\"#8a90dd\" stroke=\"#534fbc\""));
    assert!(svg.contains("fill=\"#fff\""));
    assert!(!svg.contains("#A8C5E1"));
}

#[test]
fn active_bar_uses_lavender_palette() {
    let d = build("gantt\ndateFormat YYYY-MM-DD\nsection S\nWork : active, 2026-01-01, 5d\n");
    let svg = render(&d, &Theme::default());
    // Pale lavender-blue fill, dark inside label (not the old orange).
    assert!(svg.contains("fill=\"#bfc7ff\" stroke=\"#534fbc\""));
    assert!(!svg.contains("#FAC858"));
}

#[test]
fn full_height_grid_lines_span_the_body() {
    // Every axis tick draws a light-grey full-height grid line through the
    // rows (#320), so at least one such line is present.
    let d = build("gantt\ndateFormat YYYY-MM-DD\nsection S\nT : 2026-01-01, 10d\n");
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("stroke=\"#d3d3d3\" stroke-width=\"1\" opacity=\"0.8\""));
}

#[test]
fn milestone_label_is_italic() {
    let d = build("gantt\ndateFormat YYYY-MM-DD\nsection S\nKickoff : milestone, 2026-01-01, 0d\n");
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("font-style=\"italic\""));
    // Diamond takes the default purple task fill.
    assert!(svg.contains("fill=\"#8a90dd\""));
}

#[test]
fn excludes_weekends_shade_and_stretch() {
    // 2026-01-01 is a Thursday; a 5-working-day task crosses the weekend,
    // so with `excludes weekends` the bar spans 7 calendar days.
    let d =
        build("gantt\ndateFormat YYYY-MM-DD\nexcludes weekends\nsection S\nT : 2026-01-01, 5d\n");
    let resolved = resolve_tasks(&d);
    assert!((resolved[0].duration - 7.0).abs() < 1e-6);
    let svg = render(&d, &Theme::default());
    // Weekend shading band present.
    assert!(svg.contains("fill-opacity=\"0.04\""));
}

#[test]
fn tick_interval_units() {
    assert_eq!(parse_tick_interval("1day"), Some(1.0));
    assert_eq!(parse_tick_interval("2week"), Some(14.0));
    assert_eq!(parse_tick_interval("1month"), Some(30.0));
    assert_eq!(parse_tick_interval("1w"), Some(7.0));
    assert_eq!(parse_tick_interval("1year"), None);
}

#[test]
fn tick_interval_overrides_auto_step() {
    // A 28-day span auto-picks a 2-day step (14 labels); `tickInterval
    // 1week` forces a 7-day step (fewer labels).
    let span = "gantt\ndateFormat YYYY-MM-DD\nsection S\nT : 2026-01-01, 28d\n";
    let auto = render(&build(span), &Theme::default());
    let weekly = render(
        &build(&span.replace("section S", "tickInterval 1week\nsection S")),
        &Theme::default(),
    );
    let count = |s: &str| s.matches("font-size=\"11\"").count();
    assert!(count(&weekly) < count(&auto));
}

#[test]
fn weekday_offset_is_days_to_next_named_weekday() {
    let thu = crate::svg::gantt_date::days_from_civil(2026, 1, 1) as f64;
    assert_eq!(weekday_tick_offset(Some("monday"), thu), 4.0);
    assert_eq!(weekday_tick_offset(Some("thursday"), thu), 0.0);
    assert_eq!(weekday_tick_offset(None, thu), 0.0);
}

#[test]
fn weekend_friday_shades_friday_not_sunday() {
    // A task spanning the first week of 2026 with `weekend friday`: Friday
    // 2026-01-02 becomes a shaded non-working day.
    let d = build(
        "gantt\ndateFormat YYYY-MM-DD\nexcludes weekends\nweekend friday\nsection S\nT : 2026-01-01, 10d\n",
    );
    let ex = Excludes::parse(&d.excludes, d.date_format.as_deref(), d.weekend.as_deref());
    use crate::svg::gantt_date::days_from_civil;
    assert!(ex.is_excluded(days_from_civil(2026, 1, 2))); // Friday
    assert!(!ex.is_excluded(days_from_civil(2026, 1, 4))); // Sunday now works
}

#[test]
fn vert_task_draws_marker_line_not_bar() {
    let d = build("gantt\ndateFormat YYYY-MM-DD\nsection S\nBase : 2026-01-01, 10d\nFreeze : vert, v1, 2026-01-05, 0d\n");
    let svg = render(&d, &Theme::default());
    // Thick navy full-height marker with a bold navy centered label (#320);
    // not dashed.
    assert!(!svg.contains("stroke-dasharray=\"2 2\""));
    assert!(svg.contains("stroke=\"#000080\" stroke-width=\"4\""));
    assert!(svg
        .contains("text-anchor=\"middle\" fill=\"#000080\" font-size=\"11\" font-weight=\"bold\""));
    assert!(svg.contains(">Freeze<"));
}

#[test]
fn vert_task_allocates_no_row() {
    // A lone `vert` marker in a section must not add a task row: the chart
    // height matches the same chart without the marker.
    let with = build("gantt\ndateFormat YYYY-MM-DD\nsection S\nBase : 2026-01-01, 10d\nFreeze : vert, v1, 2026-01-05, 0d\n");
    let without = build("gantt\ndateFormat YYYY-MM-DD\nsection S\nBase : 2026-01-01, 10d\n");
    let h_with = render(&with, &Theme::default());
    let h_without = render(&without, &Theme::default());
    let vb = |s: &str| {
        s.split("viewBox=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .to_string()
    };
    assert_eq!(vb(&h_with), vb(&h_without));
}

#[test]
fn click_wraps_task_bar_in_anchor() {
    let d = build("gantt\ndateFormat YYYY-MM-DD\nsection S\nA : a, 2026-01-01, 5d\nclick a href \"https://example.com\"\n");
    let svg = render(&d, &Theme::default());
    assert!(svg.contains("<a href=\"https://example.com\""));
}

#[test]
fn today_marker_off_draws_no_marker() {
    // A chart around the present day with `todayMarker off` shows no marker.
    let base = today_days();
    use crate::svg::gantt_date::civil_from_days;
    let (y, m, day) = civil_from_days(base - 1);
    let src = format!(
        "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection S\nT : {y:04}-{m:02}-{day:02}, 5d\n"
    );
    let svg = render(&build(&src), &Theme::default());
    assert!(!svg.contains(">today<"));
}

#[test]
fn today_marker_style_applied_when_in_range() {
    let base = today_days();
    use crate::svg::gantt_date::civil_from_days;
    let (y, m, day) = civil_from_days(base - 1);
    let src = format!(
        "gantt\ndateFormat YYYY-MM-DD\ntodayMarker stroke:cyan,stroke-width:5px\nsection S\nT : {y:04}-{m:02}-{day:02}, 5d\n"
    );
    let svg = render(&build(&src), &Theme::default());
    assert!(svg.contains(">today<"));
    assert!(svg.contains("stroke:cyan; stroke-width:5px"));
}

#[test]
fn after_multiple_ids_uses_latest_end() {
    // C follows the later of A (ends day 5) and B (ends day 10).
    let d = build(
        "gantt\ndateFormat YYYY-MM-DD\nsection S\nA : a, 2026-01-01, 5d\nB : b, 2026-01-01, 10d\nC : after a b, 2d\n",
    );
    let resolved = resolve_tasks(&d);
    let b_end = resolved[1].start_day + resolved[1].duration;
    assert!((resolved[2].start_day - b_end).abs() < 1e-6);
}

#[test]
fn subday_durations_keep_true_length() {
    // With a sub-day span the half-day floor is dropped, so a 2h task and a
    // 90m task keep distinct durations instead of both clamping to 0.5d.
    let d = build("gantt\ndateFormat HH:mm\nsection S\nA : 10:00, 2h\nB : 12:00, 90m\n");
    let resolved = resolve_tasks(&d);
    assert!((resolved[0].duration - 2.0 / 24.0).abs() < 1e-9);
    assert!((resolved[1].duration - 1.5 / 24.0).abs() < 1e-9);
    assert!(resolved[0].duration > resolved[1].duration);
}

#[test]
fn subday_axis_shows_real_times() {
    // A time-only chart draws several minute-spaced ticks labelled with real
    // hours/minutes, not a single `00:00`.
    let d =
        build("gantt\ndateFormat HH:mm\naxisFormat %H:%M\nsection S\nA : 09:00, 30m\nB : 30m\n");
    let svg = render(&d, &Theme::default());
    assert!(svg.contains(">09:00<"));
    assert!(svg.contains(">09:30<"));
}

#[test]
fn tick_labels_do_not_overlap() {
    // #244: the sample's ISO labels are ~59px wide; the chosen step must
    // keep consecutive label spans from intersecting.
    let d = build(
        "gantt\ntitle Release plan\ndateFormat YYYY-MM-DD\nexcludes weekends\nsection Design\nSpec : a1, 2026-01-01, 5d\nReview : after a1, 2d\nsection Build\nBackend : crit, b1, 2026-01-08, 1w\nFrontend : active, 2026-01-08, 1w\nInteg : after b1 a1, 3d\n",
    );
    let resolved = resolve_tasks(&d);
    let (start, total, _) = chart_span(&resolved);
    let ticks = axis_ticks(&d, start, total, TIME_COL_MIN_W);
    assert!(ticks.len() >= 2, "expected multiple ticks");
    for pair in ticks.windows(2) {
        let (x0, label) = &pair[0];
        let (x1, _) = &pair[1];
        let end = x0 + 2.0 + text_width(label, AXIS_LABEL_CHAR_W, AXIS_FONT_SIZE);
        assert!(
            end <= x1 + 2.0 + 1e-6,
            "labels overlap: {label:?} ends at {end}, next tick at {x1}",
        );
    }
}

#[test]
fn tick_step_widens_as_range_grows() {
    let step = |days: &str| {
        let d = build(&format!(
            "gantt\ndateFormat YYYY-MM-DD\nsection S\nT : 2026-01-01, {days}\n"
        ));
        let resolved = resolve_tasks(&d);
        let (start, total, _) = chart_span(&resolved);
        let ticks = axis_ticks(&d, start, total, TIME_COL_MIN_W);
        // Step in days = fractional gap between the first two ticks × total.
        (ticks[1].0 - ticks[0].0) / TIME_COL_MIN_W * total
    };
    assert!(step("5d") < step("30d"));
    assert!(step("30d") < step("200d"));
}

#[test]
fn top_axis_moves_axis_baseline_up() {
    // Default layout draws the axis band below the rows; `topAxis` restores
    // the top placement, so the axis baseline y is smaller.
    let src = "gantt\ndateFormat YYYY-MM-DD\nsection S\nA : 2026-01-01, 5d\nB : 2026-01-03, 4d\n";
    let d = build(src);
    // Two task rows (section names share the gutter, not their own rows).
    let body_h = 2.0 * ROW_H;
    // Default: baseline at HEADER_H + body_h; top: at HEADER_H + AXIS_H - 1.
    let bottom = render(&d, &Theme::default());
    let top = render(
        &build(&src.replace("section S", "topAxis\nsection S")),
        &Theme::default(),
    );
    assert!(bottom.contains(&fnum(HEADER_H + body_h)));
    assert!(top.contains(&fnum(HEADER_H + AXIS_H - 1.0)));
    assert_ne!(bottom, top);
}

#[test]
fn rightmost_outside_label_fits_in_viewbox() {
    // #311: the sample's `Integration` bar is short and near the chart end,
    // so its label is drawn to the right of the bar. The viewBox width must
    // grow to contain that overhanging label (and the last tick label).
    let d = build(
        "gantt\ntitle Release plan\ndateFormat YYYY-MM-DD\nexcludes weekends\nsection Design\nSpec : a1, 2026-01-01, 5d\nReview : after a1, 2d\nsection Build\nBackend : crit, b1, 2026-01-08, 1w\nFrontend : active, 2026-01-08, 1w\nInteger : after b1 a1, 3d\n",
    );
    let resolved = resolve_tasks(&d);
    let (start, total, sub_day) = chart_span(&resolved);
    let min_bar_dur = if sub_day { 0.0 } else { 0.5 };
    let label_gutter = section_gutter(&d);
    let body_x = PAD + label_gutter;
    let body_w = TIME_COL_MIN_W;
    let ticks = axis_ticks(&d, start, total, body_w);
    let content_right = content_right_extent(
        &d,
        &resolved,
        start,
        total,
        body_x,
        body_w,
        min_bar_dur,
        &ticks,
    );

    let svg = render(&d, &Theme::default());
    let width: f64 = svg
        .split("viewBox=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .split_whitespace()
        .nth(2)
        .unwrap()
        .parse()
        .unwrap();
    // The overhanging label extends past the plain chart body; the canvas
    // grew to fit it with padding to spare.
    assert!(content_right > body_x + body_w);
    assert!(width >= content_right + PAD - 1e-6);
}

#[test]
fn axis_format_controls_tick_labels() {
    let d =
        build("gantt\ndateFormat YYYY-MM-DD\naxisFormat %m/%d\nsection S\nT : 2026-01-01, 3d\n");
    let svg = render(&d, &Theme::default());
    // A `%m/%d` axis label like `01/01`, and no ISO `2026-01-01` tick.
    assert!(svg.contains(">01/01<"));
    assert!(!svg.contains(">2026-01-01<"));
}
