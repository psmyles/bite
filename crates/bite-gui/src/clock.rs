//! Wall clock stamps: the UTC ones the log file is written in, and the local ones the log
//! window shows them as.
//!
//! The arithmetic is the civil calendar on a count of days, so no date crate is needed. Only the
//! zone offset comes from the system, which knows the daylight saving rules for any date.

/// Milliseconds since the Unix epoch.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

/// A UTC stamp to the millisecond: `2026-09-20T09:00:00.120Z`.
pub fn iso_utc(ms: i64) -> String {
    let (date, time) = split(ms);
    format!("{date}T{time}Z")
}

/// The same instant for reading, as its date and its time, `2026-09-20` and `14:30:00.120`,
/// in the local zone or in UTC.
pub fn readable(ms: i64, local: bool) -> (String, String) {
    let offset = if local {
        utc_offset_seconds(ms.div_euclid(1000)) * 1000
    } else {
        0
    };
    split(ms + offset)
}

/// Reads `YYYY-MM-DDTHH:MM:SS[.fff]Z` back into milliseconds since the epoch.
///
/// The fraction may be left out or run to any length.
pub fn parse_iso(text: &str) -> Option<i64> {
    let text = text.strip_suffix('Z')?;
    let (date, time) = text.split_once('T')?;
    let days = parse_date(date)?;
    let time = parse_time(time)?;
    Some(days * 86_400_000 + time)
}

/// Reads `HH:MM:SS[.fff]` as milliseconds into the day.
fn parse_time(text: &str) -> Option<i64> {
    let (clock, fraction) = text.split_once('.').unwrap_or((text, ""));
    let mut fields = clock.split(':');
    let mut field = |limit: i64| {
        let value: i64 = fields.next().filter(|f| f.len() == 2)?.parse().ok()?;
        (value < limit).then_some(value)
    };
    let (hours, minutes, seconds) = (field(24)?, field(60)?, field(61)?);
    if fields.next().is_some() || !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    // Only the first three digits are milliseconds; a shorter fraction is padded out.
    let millis = format!("{fraction:0<3}")[..3].parse::<i64>().ok()?;
    Some(((hours * 60 + minutes) * 60 + seconds) * 1000 + millis)
}

fn parse_date(text: &str) -> Option<i64> {
    let mut fields = text.split('-');
    let year: i64 = fields.next().filter(|f| f.len() == 4)?.parse().ok()?;
    let month: u32 = fields.next().filter(|f| f.len() == 2)?.parse().ok()?;
    let day: u32 = fields.next().filter(|f| f.len() == 2)?.parse().ok()?;
    if fields.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(days_from_civil(year, month, day))
}

/// The date and the time of day, as text, for milliseconds since the epoch.
fn split(ms: i64) -> (String, String) {
    let days = ms.div_euclid(86_400_000);
    let of_day = ms.rem_euclid(86_400_000);
    let (year, month, day) = civil_from_days(days);
    let (hours, minutes) = (of_day / 3_600_000, of_day / 60_000 % 60);
    let (seconds, millis) = (of_day / 1000 % 60, of_day % 1000);
    (
        format!("{year:04}-{month:02}-{day:02}"),
        format!("{hours:02}:{minutes:02}:{seconds:02}.{millis:03}"),
    )
}

/// Converts a day count since the epoch into a calendar date.
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * shifted_month + 2) / 5 + 1) as u32;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

/// Converts a calendar date into a day count since the epoch; the inverse of the above.
pub fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day_of_year =
        (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// How far the local zone is ahead of UTC at a given instant, in seconds.
///
/// It is asked per instant rather than once, so a line written before a daylight saving change
/// is shown at the offset that was in force when it was written.
#[cfg(unix)]
fn utc_offset_seconds(seconds: i64) -> i64 {
    let time = seconds as libc::time_t;
    let mut local: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&time, &mut local) }.is_null() {
        return 0;
    }
    local.tm_gmtoff as i64
}

#[cfg(windows)]
fn utc_offset_seconds(seconds: i64) -> i64 {
    use windows::Win32::{Foundation::SYSTEMTIME, System::Time::SystemTimeToTzSpecificLocalTime};
    let days = seconds.div_euclid(86_400);
    let of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let universal = SYSTEMTIME {
        wYear: year as u16,
        wMonth: month as u16,
        wDay: day as u16,
        wHour: (of_day / 3600) as u16,
        wMinute: (of_day / 60 % 60) as u16,
        wSecond: (of_day % 60) as u16,
        ..SYSTEMTIME::default()
    };
    let mut local = SYSTEMTIME::default();
    // No zone given means the one the system is set to, with its rules for that date.
    if unsafe { SystemTimeToTzSpecificLocalTime(None, &universal, &mut local) }.is_err() {
        return 0;
    }
    let local_seconds = days_from_civil(
        i64::from(local.wYear),
        u32::from(local.wMonth),
        u32::from(local.wDay),
    ) * 86_400
        + i64::from(local.wHour) * 3600
        + i64::from(local.wMinute) * 60
        + i64::from(local.wSecond);
    local_seconds - seconds
}

#[cfg(not(any(unix, windows)))]
fn utc_offset_seconds(_seconds: i64) -> i64 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_epoch_converts_to_the_first_of_january_nineteen_seventy() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(31), (1970, 2, 1));
    }

    #[test]
    fn days_and_dates_convert_both_ways() {
        for days in [-800_000, -1, 0, 59, 60, 11_016, 20_716, 2_932_896] {
            let (year, month, day) = civil_from_days(days);
            assert_eq!(days_from_civil(year, month, day), days);
        }
    }

    #[test]
    fn a_utc_stamp_reads_back_to_the_instant_it_was_written_for() {
        let ms = 1_790_000_123_456;
        let stamp = iso_utc(ms);
        assert_eq!(stamp, "2026-09-21T14:15:23.456Z");
        assert_eq!(parse_iso(&stamp), Some(ms));
    }

    #[test]
    fn a_stamp_without_milliseconds_still_reads() {
        assert_eq!(parse_iso("2026-09-21T14:15:23Z"), Some(1_790_000_123_000));
        assert_eq!(parse_time("09:00:01.5"), Some(32_401_500));
    }

    #[test]
    fn text_that_is_not_a_stamp_is_refused() {
        for text in [
            "2026-09-21 14:15:23Z",
            "2026-13-21T14:15:23Z",
            "2026-09-21T24:00:00Z",
            "2026-09-21T14:15:23",
            "[registry] Loaded",
        ] {
            assert_eq!(parse_iso(text), None, "{text}");
        }
    }

    #[test]
    fn a_readable_utc_stamp_is_the_iso_one_in_two_parts() {
        assert_eq!(
            readable(1_790_000_123_456, false),
            ("2026-09-21".to_string(), "14:15:23.456".to_string())
        );
    }

    #[test]
    fn the_local_stamp_differs_from_utc_by_whole_minutes() {
        let ms = 1_790_000_123_456;
        let (date, time) = readable(ms, true);
        let back = parse_iso(&format!("{date}T{time}Z")).unwrap();
        assert_eq!((back - ms) % 60_000, 0);
        assert!((back - ms).abs() <= 14 * 3_600_000);
    }
}
