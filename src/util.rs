//! Small shared formatting helpers.

use crate::errors::AppResult;
use chrono::{DateTime, Utc};
use color_eyre::eyre::Context;
use num_format::{Locale, ToFormattedStr, ToFormattedString};
use reqwest::Url;
use std::str::FromStr;
use std::sync::LazyLock;
use sys_locale::get_locale;

static LOCALE: LazyLock<Locale> = LazyLock::new(|| {
    let locale_str = get_locale().unwrap_or(String::from("en-US"));
    Locale::from_str(&locale_str).unwrap_or(Locale::en)
});

/// Gets the elapsed time between two times as a human-readable string.
pub fn get_relative_time(date_time: DateTime<Utc>, since: DateTime<Utc>) -> String {
    let seconds = since.signed_duration_since(date_time).num_seconds().max(0);

    const MINUTE: i64 = 60;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    const MONTH: i64 = 30 * DAY;
    const YEAR: i64 = 365 * DAY;

    let (count, unit) = if seconds >= YEAR {
        (seconds / YEAR, "year")
    } else if seconds >= MONTH {
        (seconds / MONTH, "month")
    } else if seconds >= DAY {
        (seconds / DAY, "day")
    } else if seconds >= HOUR {
        (seconds / HOUR, "hour")
    } else if seconds >= MINUTE {
        (seconds / MINUTE, "minute")
    } else {
        (seconds, "second")
    };

    let plural = if count == 1 { "" } else { "s" };
    format!("{count} {unit}{plural} ago")
}

/// Formats an age as a compact string (e.g. `26d`, `3mo`, `2y`, `today`) for dense listings where
/// the verbose [`get_relative_time`] would be too wide.
pub fn format_short_age(date_time: DateTime<Utc>, since: DateTime<Utc>) -> String {
    let days = since.signed_duration_since(date_time).num_days().max(0);
    if days >= 365 {
        format!("{}y", days / 365)
    } else if days >= 30 {
        format!("{}mo", days / 30)
    } else if days >= 1 {
        format!("{days}d")
    } else {
        "today".to_string()
    }
}

/// Formats a number, adding separators, using the current locale.
pub fn format_number<T>(number: Option<T>) -> String
where
    T: ToFormattedStr,
{
    if let Some(number) = number {
        number.to_formatted_string(&*LOCALE)
    } else {
        String::default()
    }
}

/// Formats a byte count as a human-readable size (e.g. `1.5 KiB`), or an empty string when unknown.
pub fn format_size(bytes: Option<u64>) -> String {
    let Some(bytes) = bytes else {
        return String::new();
    };

    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

/// Opens a URL in the user's default browser without blocking.
pub fn open_in_browser(url: &Url) -> AppResult<()> {
    open::that_detached(url.to_string())
        .wrap_err_with(|| format!("Couldn't open {url} in a browser"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn format_size_scales_units() {
        assert_eq!(format_size(None), "");
        assert_eq!(format_size(Some(0)), "0 B");
        assert_eq!(format_size(Some(512)), "512 B");
        assert_eq!(format_size(Some(1024)), "1.0 KiB");
        assert_eq!(format_size(Some(1536)), "1.5 KiB");
        assert_eq!(format_size(Some(1_048_576)), "1.0 MiB");
    }

    #[test]
    fn format_short_age_picks_the_coarsest_unit() {
        let now = DateTime::from_timestamp(1_000_000_000, 0).unwrap();
        let ago = |days: i64| now - chrono::Duration::days(days);

        assert_eq!(format_short_age(now, now), "today");
        assert_eq!(format_short_age(ago(5), now), "5d");
        assert_eq!(format_short_age(ago(90), now), "3mo");
        assert_eq!(format_short_age(ago(800), now), "2y");
        // A future timestamp clamps to a non-negative age rather than going negative.
        assert_eq!(
            format_short_age(now + chrono::Duration::days(3), now),
            "today"
        );
    }

    #[test]
    fn get_relative_time_scales_units() {
        let now = DateTime::from_timestamp(1_000_000_000, 0).unwrap();
        let before = |secs: i64| now - chrono::Duration::seconds(secs);

        assert_eq!(get_relative_time(before(0), now), "0 seconds ago");
        assert_eq!(get_relative_time(before(1), now), "1 second ago");
        assert_eq!(get_relative_time(before(59), now), "59 seconds ago");
        assert_eq!(get_relative_time(before(60), now), "1 minute ago");
        assert_eq!(get_relative_time(before(5 * 60), now), "5 minutes ago");
        assert_eq!(get_relative_time(before(3_600), now), "1 hour ago");
        assert_eq!(get_relative_time(before(3 * 3_600), now), "3 hours ago");
        assert_eq!(get_relative_time(before(86_400), now), "1 day ago");
        assert_eq!(get_relative_time(before(5 * 86_400), now), "5 days ago");
        assert_eq!(get_relative_time(before(45 * 86_400), now), "1 month ago");
        assert_eq!(get_relative_time(before(400 * 86_400), now), "1 year ago");
        assert_eq!(
            get_relative_time(before(3650 * 86_400), now),
            "10 years ago"
        );
        // A future timestamp clamps rather than going negative.
        assert_eq!(
            get_relative_time(now + chrono::Duration::days(3), now),
            "0 seconds ago"
        );
    }

    #[test]
    fn format_number_groups_digits_and_handles_none() {
        assert_eq!(format_number::<u64>(None), "");
        assert_eq!(format_number(Some(0_u64)), "0");
        assert_eq!(format_number(Some(42_u64)), "42");

        // The grouping separator is locale-dependent, so assert the digits survive and that at
        // least one separator was inserted rather than pinning an exact separator character.
        let formatted = format_number(Some(1_234_567_u64));
        let digits: String = formatted.chars().filter(|c| c.is_ascii_digit()).collect();
        assert_eq!(digits, "1234567");
        assert!(
            formatted.len() > digits.len(),
            "expected grouping separators in {formatted:?}"
        );
    }
}
