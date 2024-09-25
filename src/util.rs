use std::str::FromStr;

use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use num_format::{Locale, ToFormattedStr, ToFormattedString};
use sys_locale::get_locale;

lazy_static! {
    // TODO maybe remove 1 or both libs?
    static ref LOCALE_STR: String = get_locale().unwrap_or(String::from("en-US"));
    // static ref LOCALE: Locale = Locale::from_str(LOCALE_STR.as_str()).unwrap_or(Locale::en);
}

pub struct Util;

impl Util {
    pub fn get_relative_time(date_time: DateTime<Utc>, since: DateTime<Utc>) -> String {
        let updated_diff = since.signed_duration_since(date_time);

        if updated_diff.num_days() > 1 {
            format!("{} days ago", updated_diff.num_days())
        } else if updated_diff.num_hours() > 1 {
            format!("{} hours ago", updated_diff.num_hours())
        } else if updated_diff.num_seconds() > 1 {
            format!("{} minutes ago", updated_diff.num_minutes())
        } else {
            format!("{} seconds ago", updated_diff.num_seconds())
        }
    }

    pub fn format_number<T>(number: T) -> String
    where
        T: ToFormattedStr,
    {
        let locale = Locale::from_str(LOCALE_STR.as_str()).unwrap_or(Locale::en);
        number.to_formatted_string(&locale)
    }
}
