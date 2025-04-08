use chrono::{DateTime, Utc};
use num_format::{Locale, ToFormattedStr, ToFormattedString};
use std::str::FromStr;
use std::sync::LazyLock;
use sys_locale::get_locale;

static LOCALE: LazyLock<Locale> = LazyLock::new(|| {
    let locale_str = get_locale().unwrap_or(String::from("en-US"));
    Locale::from_str(&locale_str).unwrap_or(Locale::en)
});

pub struct Util;

impl Util {
    /// Gets the elapsed time between two times as a human-readable string.
    pub fn get_relative_time(date_time: DateTime<Utc>, since: DateTime<Utc>) -> String {
        let delta = since.signed_duration_since(date_time);

        if delta.num_days() > 1 {
            format!("{} days ago", delta.num_days())
        } else if delta.num_hours() > 1 {
            format!("{} hours ago", delta.num_hours())
        } else if delta.num_seconds() > 1 {
            format!("{} minutes ago", delta.num_minutes())
        } else {
            format!("{} seconds ago", delta.num_seconds())
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
}
