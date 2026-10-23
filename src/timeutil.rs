use chrono::{Datelike, Duration, Local, NaiveDateTime, TimeZone, Utc};

pub fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

pub fn to_local(ms: i64) -> chrono::DateTime<Local> {
    Utc.timestamp_millis_opt(ms)
        .single()
        .unwrap_or_else(Utc::now)
        .with_timezone(&Local)
}

fn from_local_naive(naive: NaiveDateTime, fallback_ms: i64) -> i64 {
    match Local.from_local_datetime(&naive) {
        chrono::LocalResult::Single(dt) => dt.timestamp_millis(),
        chrono::LocalResult::Ambiguous(earlier, _) => earlier.timestamp_millis(),
        chrono::LocalResult::None => {
            let shifted = naive + Duration::hours(1);
            match Local.from_local_datetime(&shifted) {
                chrono::LocalResult::Single(dt) | chrono::LocalResult::Ambiguous(dt, _) => {
                    dt.timestamp_millis()
                }
                chrono::LocalResult::None => fallback_ms,
            }
        }
    }
}

pub fn start_of_day(ms: i64) -> i64 {
    let local = to_local(ms);
    let naive = local.date_naive().and_hms_opt(0, 0, 0).expect("midnight");
    from_local_naive(naive, ms)
}

pub fn add_local_days(ms: i64, days: i64) -> i64 {
    let local = to_local(ms);
    let date = local.date_naive() + Duration::days(days);
    let naive = date.and_time(local.time());
    from_local_naive(naive, ms.saturating_add(days.saturating_mul(86_400_000)))
}

pub fn end_of_day(ms: i64) -> i64 {
    add_local_days(start_of_day(ms), 1) - 1
}

pub fn start_of_week(ms: i64) -> i64 {
    let local = to_local(ms);
    let date = local.date_naive();
    let monday = date - Duration::days(date.weekday().num_days_from_monday() as i64);
    let naive = monday.and_hms_opt(0, 0, 0).expect("midnight");
    from_local_naive(naive, ms)
}

pub fn end_of_week(ms: i64) -> i64 {
    add_local_days(start_of_week(ms), 7) - 1
}

pub fn local_days_between(from_ms: i64, to_ms: i64) -> i64 {
    let a = to_local(from_ms).date_naive();
    let b = to_local(to_ms).date_naive();
    (b - a).num_days()
}

/// Weekday of `ms` inside the week that contains `now`. Monday is 0.
pub fn weekday_index(ms: i64, now: i64) -> Option<usize> {
    let start = start_of_week(now);
    let end = end_of_week(now);
    if ms < start || ms > end {
        return None;
    }
    let index = local_days_between(start, ms);
    if (0..7).contains(&index) {
        Some(index as usize)
    } else {
        None
    }
}

pub fn weekday_short(index: usize) -> &'static str {
    ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"][index % 7]
}

pub fn format_long_date(ms: i64) -> String {
    let local = to_local(ms);
    let weekdays = [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ];
    let months = [
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
    let weekday = weekdays[local.weekday().num_days_from_monday() as usize];
    let month = months[local.month() as usize - 1];
    format!("{weekday}, {} {month}", local.day())
}

pub fn format_span(from_ms: i64, to_ms: i64) -> String {
    let delta = to_ms.saturating_sub(from_ms);
    if delta < 45_000 {
        return "now".into();
    }
    if delta < 3_600_000 {
        let minutes = (delta + 30_000) / 60_000;
        return format!("{minutes}m");
    }
    let days = local_days_between(from_ms, to_ms);
    if days <= 0 {
        let hours = (delta + 1_800_000) / 3_600_000;
        return format!("{hours}h");
    }
    if days == 1 {
        "1d".into()
    } else if days < 30 {
        format!("{days}d")
    } else if days < 365 {
        format!("{}mo", ((days as f32) / 30.0).round() as i64)
    } else {
        format!("{:.1}y", days as f32 / 365.25)
    }
}

pub fn relative_day(due_ms: i64, now_ms: i64) -> String {
    let days = local_days_between(now_ms, due_ms);
    if days <= 0 {
        "today".into()
    } else if days == 1 {
        "tomorrow".into()
    } else {
        format!("in {days} days")
    }
}

pub fn format_when(due_ms: i64, now_ms: i64) -> String {
    let days = local_days_between(now_ms, due_ms);
    if days < 0 {
        let n = -days;
        if n == 1 {
            "Due yesterday".into()
        } else {
            format!("Overdue {n}d")
        }
    } else if due_ms <= now_ms {
        "Due now".into()
    } else if days == 0 {
        "Later today".into()
    } else if days == 1 {
        "Tomorrow".into()
    } else {
        format!("In {days} days")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Weekday;

    #[test]
    fn week_starts_monday() {
        let now = now_ms();
        assert_eq!(to_local(start_of_week(now)).weekday(), Weekday::Mon);
    }

    #[test]
    fn add_days_follows_the_calendar() {
        let now = now_ms();
        let later = add_local_days(now, 3);
        assert_eq!(local_days_between(now, later), 3);
        assert_eq!(local_days_between(later, now), -3);
    }

    #[test]
    fn end_of_week_is_seven_days_after_monday() {
        let now = now_ms();
        let start = start_of_week(now);
        let end = end_of_week(now);
        assert!(end > start);
        assert_eq!(local_days_between(start, end), 6);
    }
}
