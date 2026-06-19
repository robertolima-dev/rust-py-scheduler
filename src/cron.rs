use chrono::{DateTime, Datelike, Duration as ChronoDuration, Local, TimeZone, Timelike};

use crate::errors::SchedulerError;

/// A parsed 5-field Unix cron expression: `minute hour day-of-month month
/// day-of-week`. Unlike `Schedule::Every`, cron is fundamentally a
/// *calendar* schedule ("every day at 9am" is not a fixed duration), so it
/// can't be represented as a single `Duration` added to an `Instant`. The
/// run loop still schedules against monotonic `Instant`s, so the registry
/// asks this type for the next wall-clock occurrence and converts the gap
/// into an `Instant` deadline (see `registry::schedule_next`).
///
/// Day-of-week follows the common Unix convention: 0-6 with Sunday = 0, and
/// 7 is also accepted as Sunday. When *both* day-of-month and day-of-week
/// are restricted (neither is `*`), a timestamp matches if it satisfies
/// *either* field — the same behavior as Vixie cron.
#[derive(Debug, Clone)]
pub struct CronSchedule {
    /// Kept verbatim so `list_jobs()` can show the original expression
    /// instead of a reconstructed (and possibly normalized) form.
    expression: String,
    minutes: Vec<u32>,
    hours: Vec<u32>,
    days_of_month: Vec<u32>,
    months: Vec<u32>,
    days_of_week: Vec<u32>,
    /// Whether the day-of-month / day-of-week fields were literally `*`.
    /// Needed to apply the "OR when both are restricted" rule correctly.
    dom_restricted: bool,
    dow_restricted: bool,
}

impl CronSchedule {
    pub fn parse(expression: &str) -> Result<Self, SchedulerError> {
        let fields: Vec<&str> = expression.split_whitespace().collect();
        if fields.len() != 5 {
            return Err(SchedulerError::InvalidCron(expression.to_string()));
        }

        let invalid = || SchedulerError::InvalidCron(expression.to_string());

        let minutes = parse_field(fields[0], 0, 59).ok_or_else(invalid)?;
        let hours = parse_field(fields[1], 0, 23).ok_or_else(invalid)?;
        let days_of_month = parse_field(fields[2], 1, 31).ok_or_else(invalid)?;
        let months = parse_field(fields[3], 1, 12).ok_or_else(invalid)?;
        // Day-of-week is parsed over 0..=7, then 7 is folded onto 0 (Sunday).
        let mut days_of_week = parse_field(fields[4], 0, 7).ok_or_else(invalid)?;
        if days_of_week.contains(&7) {
            days_of_week.retain(|&d| d != 7);
            if !days_of_week.contains(&0) {
                days_of_week.push(0);
            }
            days_of_week.sort_unstable();
        }

        Ok(CronSchedule {
            expression: expression.trim().to_string(),
            minutes,
            hours,
            days_of_month,
            months,
            days_of_week,
            dom_restricted: fields[2] != "*",
            dow_restricted: fields[4] != "*",
        })
    }

    pub fn expression(&self) -> &str {
        &self.expression
    }

    fn matches(&self, when: &DateTime<Local>) -> bool {
        if !self.minutes.contains(&when.minute())
            || !self.hours.contains(&when.hour())
            || !self.months.contains(&when.month())
        {
            return false;
        }

        let dom_ok = self.days_of_month.contains(&when.day());
        // chrono: Sun=6 in num_days_from_monday terms; use weekday() mapping.
        // Sunday should be 0 to match the Unix convention.
        let dow = when.weekday().num_days_from_sunday();
        let dow_ok = self.days_of_week.contains(&dow);

        match (self.dom_restricted, self.dow_restricted) {
            // Vixie cron: when both day fields are restricted, match either.
            (true, true) => dom_ok || dow_ok,
            (true, false) => dom_ok,
            (false, true) => dow_ok,
            (false, false) => true,
        }
    }

    /// The first matching wall-clock minute strictly after `after`. Iterates
    /// minute by minute (cron has minute resolution), capped at ~4 years so
    /// an impossible expression like "Feb 31" terminates with `None` instead
    /// of looping forever.
    pub fn next_after(&self, after: DateTime<Local>) -> Option<DateTime<Local>> {
        // Advance to the start of the next whole minute.
        let mut candidate = (after + ChronoDuration::minutes(1))
            .with_second(0)
            .and_then(|dt| dt.with_nanosecond(0))?;

        // 4 years of minutes covers every leap-year edge case.
        const MAX_MINUTES: u64 = 4 * 366 * 24 * 60;
        for _ in 0..MAX_MINUTES {
            if self.matches(&candidate) {
                return Some(candidate);
            }
            candidate += ChronoDuration::minutes(1);
        }
        None
    }
}

/// Parses a single cron field over the inclusive range `[min, max]`.
/// Supports `*`, `*/step`, `a`, `a-b`, `a-b/step`, and comma-separated lists
/// of any of those. Returns `None` on anything malformed or out of range.
fn parse_field(field: &str, min: u32, max: u32) -> Option<Vec<u32>> {
    let mut values: Vec<u32> = Vec::new();

    for part in field.split(',') {
        let (range_part, step) = match part.split_once('/') {
            Some((range_part, step_str)) => {
                let step: u32 = step_str.parse().ok()?;
                if step == 0 {
                    return None;
                }
                (range_part, step)
            }
            None => (part, 1),
        };

        let (start, end) = if range_part == "*" {
            (min, max)
        } else if let Some((a, b)) = range_part.split_once('-') {
            (a.parse().ok()?, b.parse().ok()?)
        } else {
            let value: u32 = range_part.parse().ok()?;
            (value, value)
        };

        if start < min || end > max || start > end {
            return None;
        }

        let mut value = start;
        while value <= end {
            values.push(value);
            value += step;
        }
    }

    if values.is_empty() {
        return None;
    }

    values.sort_unstable();
    values.dedup();
    Some(values)
}

/// Converts a parsed cron schedule into the next monotonic deadline plus a
/// Unix-timestamp string for display, mirroring what `Schedule::Every`
/// produces. `now_instant`/`now_wall` are taken together so the gap between
/// the next wall-clock occurrence and "now" is applied to the same instant.
pub fn next_deadline(
    schedule: &CronSchedule,
    now_instant: std::time::Instant,
    now_wall: DateTime<Local>,
) -> (std::time::Instant, Option<String>) {
    match schedule.next_after(now_wall) {
        Some(next_wall) => {
            let gap = (next_wall - now_wall)
                .to_std()
                .unwrap_or(std::time::Duration::ZERO);
            (now_instant + gap, Some(next_wall.timestamp().to_string()))
        }
        None => {
            // No occurrence within the search horizon: park it far in the
            // future so it never fires, and report no next run.
            (
                now_instant + std::time::Duration::from_secs(60 * 60 * 24 * 365),
                None,
            )
        }
    }
}

#[allow(dead_code)]
fn local_from_ymd_hm(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> DateTime<Local> {
    Local
        .with_ymd_and_hms(year, month, day, hour, minute, 0)
        .single()
        .expect("valid local datetime in tests")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_wrong_field_count() {
        assert!(CronSchedule::parse("* * * *").is_err());
        assert!(CronSchedule::parse("* * * * * *").is_err());
        assert!(CronSchedule::parse("").is_err());
    }

    #[test]
    fn parses_every_minute() {
        let schedule = CronSchedule::parse("* * * * *").unwrap();
        assert_eq!(schedule.minutes.len(), 60);
        assert_eq!(schedule.hours.len(), 24);
    }

    #[test]
    fn parses_specific_values() {
        let schedule = CronSchedule::parse("0 9 * * *").unwrap();
        assert_eq!(schedule.minutes, vec![0]);
        assert_eq!(schedule.hours, vec![9]);
    }

    #[test]
    fn parses_step_values() {
        let schedule = CronSchedule::parse("*/15 * * * *").unwrap();
        assert_eq!(schedule.minutes, vec![0, 15, 30, 45]);
    }

    #[test]
    fn parses_ranges_and_lists() {
        let schedule = CronSchedule::parse("0 9-11,17 * * *").unwrap();
        assert_eq!(schedule.hours, vec![9, 10, 11, 17]);
    }

    #[test]
    fn rejects_out_of_range() {
        assert!(CronSchedule::parse("60 * * * *").is_err());
        assert!(CronSchedule::parse("* 24 * * *").is_err());
        assert!(CronSchedule::parse("* * 0 * *").is_err()); // day-of-month is 1-31
        assert!(CronSchedule::parse("* * * 13 *").is_err());
    }

    #[test]
    fn rejects_non_numeric() {
        assert!(CronSchedule::parse("a * * * *").is_err());
        assert!(CronSchedule::parse("*/0 * * * *").is_err());
    }

    #[test]
    fn sunday_accepts_both_zero_and_seven() {
        let zero = CronSchedule::parse("0 0 * * 0").unwrap();
        let seven = CronSchedule::parse("0 0 * * 7").unwrap();
        assert_eq!(zero.days_of_week, seven.days_of_week);
        assert_eq!(zero.days_of_week, vec![0]);
    }

    #[test]
    fn next_after_finds_the_next_top_of_hour() {
        let schedule = CronSchedule::parse("0 * * * *").unwrap();
        let now = local_from_ymd_hm(2026, 6, 19, 10, 30);
        let next = schedule.next_after(now).unwrap();
        assert_eq!(next.hour(), 11);
        assert_eq!(next.minute(), 0);
    }

    #[test]
    fn next_after_is_strictly_after_now() {
        // At exactly 11:00, "0 * * * *" must return 12:00, never 11:00 again.
        let schedule = CronSchedule::parse("0 * * * *").unwrap();
        let now = local_from_ymd_hm(2026, 6, 19, 11, 0);
        let next = schedule.next_after(now).unwrap();
        assert_eq!(next.hour(), 12);
        assert_eq!(next.minute(), 0);
    }

    #[test]
    fn next_after_handles_daily_time() {
        let schedule = CronSchedule::parse("30 9 * * *").unwrap();
        let now = local_from_ymd_hm(2026, 6, 19, 10, 0);
        let next = schedule.next_after(now).unwrap();
        assert_eq!(next.day(), 20);
        assert_eq!(next.hour(), 9);
        assert_eq!(next.minute(), 30);
    }

    #[test]
    fn impossible_expression_returns_none() {
        // February 30th never happens.
        let schedule = CronSchedule::parse("0 0 30 2 *").unwrap();
        let now = local_from_ymd_hm(2026, 1, 1, 0, 0);
        assert!(schedule.next_after(now).is_none());
    }
}
