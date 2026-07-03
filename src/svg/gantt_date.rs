//! Exact civil-date arithmetic for the gantt renderer.
//!
//! Dates are represented as signed day counts from the Unix epoch
//! (1970-01-01 = 0) using Howard Hinnant's `days_from_civil` /
//! `civil_from_days` algorithms — exact for all proleptic-Gregorian dates,
//! unlike the previous `365.25`-day approximation which drifted by ±1 day
//! around month boundaries.

/// Days from 1970-01-01 for a proleptic-Gregorian `(y, m, d)`.
pub(crate) fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400; // [0, 399]
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146097 + doe - 719468
}

/// Inverse of [`days_from_civil`]: a day count back to `(y, m, d)`.
pub(crate) fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Weekday for a day count: `0 = Sunday .. 6 = Saturday`
/// (1970-01-01 was a Thursday).
pub(crate) fn weekday(z: i64) -> i64 {
    (z + 4).rem_euclid(7)
}

/// The current date as a whole day count from the epoch (UTC), read from the
/// system clock. Used only to position the `todayMarker`, which is drawn only
/// when it falls inside the chart's date range — keeping deterministic output
/// for charts that don't span the present day.
pub(crate) fn today_days() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| (d.as_secs() / 86_400) as i64)
        .unwrap_or(0)
}

#[derive(Clone, Copy)]
enum Field {
    Year,
    Month,
    Day,
    Hour,
    Minute,
    Second,
    Milli,
}

/// Field order implied by a `dateFormat` string, honoring both the date tokens
/// (`Y`/`M`/`D`) and any sub-day time tokens (`H` hour, lowercase `m` minute,
/// lowercase `s` second, `S` millisecond). Defaults to year-month-day when the
/// three date tokens can't be located.
fn field_order(fmt: &str) -> Vec<Field> {
    let candidates = [
        (fmt.find(['Y', 'y']), Field::Year),
        (fmt.find('M'), Field::Month),
        (fmt.find(['D', 'd']), Field::Day),
        (fmt.find(['H', 'h']), Field::Hour),
        (fmt.find('m'), Field::Minute),
        (fmt.find('s'), Field::Second),
        (fmt.find('S'), Field::Milli),
    ];
    // A well-formed format has the three date tokens; otherwise fall back.
    if candidates[0].0.is_none() || candidates[1].0.is_none() || candidates[2].0.is_none() {
        return vec![Field::Year, Field::Month, Field::Day];
    }
    let mut present: Vec<(usize, Field)> = candidates
        .into_iter()
        .filter_map(|(pos, f)| pos.map(|p| (p, f)))
        .collect();
    present.sort_by_key(|(i, _)| *i);
    present.into_iter().map(|(_, f)| f).collect()
}

/// Parse a date string into a whole day count, honoring `date_format`'s field
/// order. Any non-digit run separates fields, so `2026-01-05`, `2026/01/05` and
/// `05.01.2026` (with `DD.MM.YYYY`) all parse. Sub-day time components are
/// ignored — use [`parse_datetime`] to keep them.
pub(crate) fn parse_date(s: &str, date_format: Option<&str>) -> Option<i64> {
    parse_datetime(s, date_format).map(|d| d.floor() as i64)
}

/// Parse a date (optionally with a `HH:mm[:ss]` time) into a **fractional** day
/// count from the epoch, honoring `date_format`'s field order. A `dateFormat`
/// with time tokens (`HH:mm`) yields sub-day precision; a plain date yields an
/// integer day count.
pub(crate) fn parse_datetime(s: &str, date_format: Option<&str>) -> Option<f64> {
    let nums: Vec<i64> = s
        .split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty())
        .map(|p| p.parse().ok())
        .collect::<Option<Vec<_>>>()?;
    if nums.len() < 3 {
        return None;
    }
    let (mut y, mut m, mut d) = (0i64, 1i64, 1i64);
    let (mut hh, mut mi, mut ss, mut ms) = (0i64, 0i64, 0i64, 0i64);
    for (field, &v) in field_order(date_format.unwrap_or("YYYY-MM-DD"))
        .iter()
        .zip(&nums)
    {
        match field {
            Field::Year => y = v,
            Field::Month => m = v,
            Field::Day => d = v,
            Field::Hour => hh = v,
            Field::Minute => mi = v,
            Field::Second => ss = v,
            Field::Milli => ms = v,
        }
    }
    let frac =
        hh as f64 / 24.0 + mi as f64 / 1440.0 + ss as f64 / 86_400.0 + ms as f64 / 86_400_000.0;
    Some(days_from_civil(y, m, d) as f64 + frac)
}

const MONTHS_ABBR: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const MONTHS_FULL: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const DAYS_ABBR: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const DAYS_FULL: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

/// Render a day count using a d3-style `axisFormat` (a `strftime` subset);
/// defaults to ISO `%Y-%m-%d`.
pub(crate) fn format_date(day: i64, axis_format: Option<&str>) -> String {
    let fmt = axis_format.unwrap_or("%Y-%m-%d");
    let (y, m, d) = civil_from_days(day);
    let mi = (m - 1).clamp(0, 11) as usize;
    let wd = weekday(day) as usize;
    let mut out = String::new();
    let mut chars = fmt.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('Y') => out.push_str(&format!("{y:04}")),
            Some('y') => out.push_str(&format!("{:02}", y.rem_euclid(100))),
            Some('m') => out.push_str(&format!("{m:02}")),
            Some('d') => out.push_str(&format!("{d:02}")),
            Some('e') => out.push_str(&format!("{d:2}")),
            Some('j') => out.push_str(&format!("{:03}", day - days_from_civil(y, 1, 1) + 1)),
            Some('b') | Some('h') => out.push_str(MONTHS_ABBR[mi]),
            Some('B') => out.push_str(MONTHS_FULL[mi]),
            Some('a') => out.push_str(DAYS_ABBR[wd]),
            Some('A') => out.push_str(DAYS_FULL[wd]),
            Some('H') => out.push_str("00"),
            Some('M') => out.push_str("00"),
            Some('S') => out.push_str("00"),
            Some('%') => out.push('%'),
            Some(other) => {
                out.push('%');
                out.push(other);
            }
            None => out.push('%'),
        }
    }
    out
}

/// The two weekday numbers `excludes weekends` skips for a `weekend` config:
/// `friday` → Fri(5)+Sat(6); anything else (the upstream default) → Sat(6)+Sun(0).
fn weekend_days_for(weekend: Option<&str>) -> [i64; 2] {
    match weekend.map(|w| w.trim().to_ascii_lowercase()).as_deref() {
        Some("friday") => [5, 6],
        _ => [6, 0],
    }
}

/// Excluded (non-working) days: weekends, named weekdays, and specific dates.
pub(crate) struct Excludes {
    /// The weekend pair when `excludes weekends` is set, else empty.
    weekend_days: Vec<i64>,
    weekdays: Vec<i64>,
    dates: Vec<i64>,
}

impl Excludes {
    pub(crate) fn parse(raw: &[String], date_format: Option<&str>, weekend: Option<&str>) -> Self {
        let mut weekend_days = Vec::new();
        let mut weekdays = Vec::new();
        let mut dates = Vec::new();
        for tok in raw {
            match tok.trim().to_ascii_lowercase().as_str() {
                "weekends" => weekend_days = weekend_days_for(weekend).to_vec(),
                "sunday" => weekdays.push(0),
                "monday" => weekdays.push(1),
                "tuesday" => weekdays.push(2),
                "wednesday" => weekdays.push(3),
                "thursday" => weekdays.push(4),
                "friday" => weekdays.push(5),
                "saturday" => weekdays.push(6),
                _ => {
                    if let Some(day) = parse_date(tok, date_format) {
                        dates.push(day);
                    }
                }
            }
        }
        Excludes {
            weekend_days,
            weekdays,
            dates,
        }
    }

    pub(crate) fn active(&self) -> bool {
        !self.weekend_days.is_empty() || !self.weekdays.is_empty() || !self.dates.is_empty()
    }

    pub(crate) fn is_excluded(&self, day: i64) -> bool {
        let wd = weekday(day);
        self.weekend_days.contains(&wd) || self.weekdays.contains(&wd) || self.dates.contains(&day)
    }

    /// End day for a `start`-day task lasting `dur_days` *working* days: each
    /// excluded calendar day the span crosses pushes the end out by one, so the
    /// bar stretches over weekends (matching upstream's `getMaxEndTime`).
    pub(crate) fn stretched_end(&self, start: i64, dur_days: i64) -> i64 {
        let mut end = start + dur_days;
        let mut t = start + 1;
        while t <= end {
            if self.is_excluded(t) {
                end += 1;
            }
            t += 1;
        }
        end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_civil_dates() {
        for &(y, m, d) in &[(1970, 1, 1), (2026, 1, 1), (2026, 12, 31), (2000, 2, 29)] {
            let z = days_from_civil(y, m, d);
            assert_eq!(civil_from_days(z), (y, m, d));
        }
    }

    #[test]
    fn day_math_is_exact_across_month_boundary() {
        // 2026-01-31 → 2026-02-01 is exactly one day (the old 365.25 math drifted here).
        let a = days_from_civil(2026, 1, 31);
        let b = days_from_civil(2026, 2, 1);
        assert_eq!(b - a, 1);
    }

    #[test]
    fn weekday_known_points() {
        // 1970-01-01 Thursday, 2026-01-01 Thursday, 2026-01-03 Saturday.
        assert_eq!(weekday(days_from_civil(1970, 1, 1)), 4);
        assert_eq!(weekday(days_from_civil(2026, 1, 3)), 6);
        assert_eq!(weekday(days_from_civil(2026, 1, 4)), 0);
    }

    #[test]
    fn parses_field_order_from_format() {
        assert_eq!(
            parse_date("05-01-2026", Some("DD-MM-YYYY")),
            Some(days_from_civil(2026, 1, 5))
        );
        assert_eq!(
            parse_date("2026/01/05", Some("YYYY/MM/DD")),
            Some(days_from_civil(2026, 1, 5))
        );
    }

    #[test]
    fn parse_datetime_honors_subday_time_tokens() {
        let noon = parse_datetime("2026-01-03 12:00", Some("YYYY-MM-DD HH:mm")).unwrap();
        let midnight = parse_datetime("2026-01-03 00:00", Some("YYYY-MM-DD HH:mm")).unwrap();
        assert!((noon - midnight - 0.5).abs() < 1e-9);
        assert_eq!(midnight, days_from_civil(2026, 1, 3) as f64);
        // Seconds and the `mm` minute token don't collide with `MM` (month).
        let dt = parse_datetime("2026-01-03 06:30:00", Some("YYYY-MM-DD HH:mm:ss")).unwrap();
        assert!((dt - days_from_civil(2026, 1, 3) as f64 - (6.5 / 24.0)).abs() < 1e-9);
    }

    #[test]
    fn parse_date_floors_time_to_whole_day() {
        assert_eq!(
            parse_date("2026-01-03 18:00", Some("YYYY-MM-DD HH:mm")),
            Some(days_from_civil(2026, 1, 3))
        );
    }

    #[test]
    fn axis_format_specifiers() {
        let day = days_from_civil(2026, 1, 5); // a Monday
        assert_eq!(format_date(day, None), "2026-01-05");
        assert_eq!(format_date(day, Some("%m/%d")), "01/05");
        assert_eq!(format_date(day, Some("%b %d")), "Jan 05");
        assert_eq!(format_date(day, Some("%a")), "Mon");
    }

    #[test]
    fn weekends_excluded_and_stretched() {
        let ex = Excludes::parse(&["weekends".to_string()], None, None);
        assert!(ex.active());
        // 2026-01-03 Sat and 2026-01-04 Sun are excluded.
        assert!(ex.is_excluded(days_from_civil(2026, 1, 3)));
        assert!(ex.is_excluded(days_from_civil(2026, 1, 4)));
        assert!(!ex.is_excluded(days_from_civil(2026, 1, 5)));
        // A 5-working-day task from Thu 2026-01-01 spans the weekend → ends
        // two calendar days later than the naive Thu+5.
        let start = days_from_civil(2026, 1, 1);
        assert_eq!(ex.stretched_end(start, 5), start + 7);
    }

    #[test]
    fn weekend_friday_shifts_the_weekend_pair() {
        // `weekend friday` makes Fri(2026-01-02)+Sat(2026-01-03) the weekend,
        // leaving Sun(2026-01-04) a working day.
        let ex = Excludes::parse(&["weekends".to_string()], None, Some("friday"));
        assert!(ex.is_excluded(days_from_civil(2026, 1, 2)));
        assert!(ex.is_excluded(days_from_civil(2026, 1, 3)));
        assert!(!ex.is_excluded(days_from_civil(2026, 1, 4)));
    }
}
