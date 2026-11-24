//! ISO 8601 to [`Timestamp`], the way `ClaudeUsageScanner.parseISO8601` reads
//! the CLI's timestamps: `2026-09-12T03:35:41.545Z` and the plain second form.
//!
//! The core never reads a clock; this only turns a string the transcript
//! already carries into the instant it names.

use crate::reduce::Timestamp;

/// `ISO8601DateFormatter` with `.withInternetDateTime` and, when present,
/// `.withFractionalSeconds`: `YYYY-MM-DDTHH:MM:SS[.sss](Z|±HH:MM)`.
pub fn parse_iso8601(raw: &str) -> Option<Timestamp> {
    let bytes = raw.as_bytes();
    if bytes.len() < 20 || bytes[4] != b'-' || bytes[7] != b'-' || bytes[10] != b'T' {
        return None;
    }
    let number = |range: std::ops::Range<usize>| -> Option<i64> {
        let digits = raw.get(range)?;
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        digits.parse().ok()
    };
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }

    let mut index = 19;
    let mut fraction = 0.0;
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        let start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index == start {
            return None;
        }
        let digits = raw.get(start..index)?;
        let scale = 10f64.powi(digits.len() as i32);
        fraction = digits.parse::<f64>().ok()? / scale;
    }

    let offset_seconds = match bytes.get(index) {
        Some(b'Z') => {
            index += 1;
            0
        }
        Some(b'+' | b'-') => {
            let sign = if bytes[index] == b'-' { -1 } else { 1 };
            let hours = number(index + 1..index + 3)?;
            if bytes.get(index + 3) != Some(&b':') {
                return None;
            }
            let minutes = number(index + 4..index + 6)?;
            index += 6;
            sign * (hours * 3600 + minutes * 60)
        }
        _ => return None,
    };
    if index != bytes.len() {
        return None;
    }

    let days = days_from_civil(year, month, day);
    let seconds = days * 86_400 + hour * 3600 + minute * 60 + second - offset_seconds;
    Some(Timestamp(seconds as f64 + fraction))
}

/// The string `ISO8601DateFormatter` with `.withInternetDateTime` writes:
/// `YYYY-MM-DDTHH:MM:SSZ`. Whole seconds, as `JSONEncoder.dateEncodingStrategy =
/// .iso8601` stores them — a session's start does not need its milliseconds back.
pub fn format_iso8601(instant: Timestamp) -> String {
    let seconds = instant.0.floor();
    let fraction = instant.0 - seconds;
    // Rounding, not truncating: `.floor()` already moved the fraction aside.
    let seconds = seconds as i64 + i64::from(fraction >= 0.999_5);
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let (hour, minute, second) = (rest / 3600, (rest % 3600) / 60, rest % 60);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// Howard Hinnant's inverse: the civil date of a day count since 1970-01-01.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_ish = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_ish + 2) / 5 + 1;
    let month = if month_ish < 10 {
        month_ish + 3
    } else {
        month_ish - 9
    };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// Howard Hinnant's civil-date algorithm: days since 1970-01-01.
pub(crate) fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatting_round_trips_through_the_parser() {
        for raw in [
            "2026-09-12T03:35:41Z",
            "1970-01-01T00:00:00Z",
            "2024-02-29T23:59:59Z",
            "2000-03-01T12:00:00Z",
            "1969-12-31T23:59:59Z",
        ] {
            let instant = parse_iso8601(raw).expect(raw);
            assert_eq!(format_iso8601(instant), raw);
        }
    }

    #[test]
    fn a_fraction_is_dropped_to_the_second() {
        let instant = parse_iso8601("2026-09-12T03:35:41.545Z").expect("a timestamp");
        assert_eq!(format_iso8601(instant), "2026-09-12T03:35:41Z");
    }

    #[test]
    fn an_offset_is_written_as_utc() {
        let instant = parse_iso8601("2026-09-12T03:35:41+07:00").expect("a timestamp");
        assert_eq!(format_iso8601(instant), "2026-09-11T20:35:41Z");
    }

    #[test]
    fn internet_date_times_with_and_without_fractions() {
        assert_eq!(parse_iso8601("1970-01-01T00:00:00Z"), Some(Timestamp(0.0)));
        assert_eq!(
            parse_iso8601("2026-09-12T03:35:41.545Z"),
            Some(Timestamp(1_789_184_141.545))
        );
        assert_eq!(
            parse_iso8601("2026-09-12T03:35:41Z"),
            Some(Timestamp(1_789_184_141.0))
        );
    }

    #[test]
    fn offsets_are_applied() {
        assert_eq!(
            parse_iso8601("2026-09-12T11:35:41+08:00"),
            Some(Timestamp(1_789_184_141.0))
        );
    }

    #[test]
    fn garbage_is_rejected() {
        for raw in [
            "",
            "t",
            "2026-09-12",
            "not a timestamp",
            "2026-13-12T00:00:00Z",
            "2026-09-12T00:00:00",
            "2026-09-12T00:00:00Zjunk",
        ] {
            assert_eq!(parse_iso8601(raw), None, "{raw}");
        }
    }
}
