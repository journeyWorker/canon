//! The two `<TIMESTAMP>` grammars of the options (0.37 `canon-0-37-option-forms`): RFC 3339 only, written as the RFC writes it.
//!
//! - a time option (`--expiry`, `--as-of`, `--before`, `--after`, `--since`) takes a full-date, `YYYY-MM-DD`, read as that day's start in
//!   UTC ([`parse`]), or a date-time;
//! - `--approval-at` takes a date-time only ([`parse_date_time`]): it is signed into the approval payload, and the caller fixes it once,
//!   for the signature and for the record.
//!
//! A date-time is `YYYY-MM-DDTHH:MM:SS`, an optional fraction, and `Z` or `+HH:MM` / `-HH:MM`. The year has four digits, the month, the day
//! and every field of the time two; the `T` and the `Z` are read in either case (RFC 3339 5.6); there is no sign on the year, no space or
//! other separator, no white space around the value and no other character for the minus of an offset. That the date and the time exist
//! (`2026-02-30`, 25:00, `+24:00`) is the calendar's: [`parse`] and [`parse_date_time`] refuse both after the shape, and say what does not
//! exist ([`TimestampError::Missing`]). A second of 60 is refused in every time place (`canon-0-38-date-refusals`): canon's time is UTC without
//! a table of leap seconds, and the instant one second after the preceding second has one spelling, the one the refusal names.

use chrono::{DateTime, Datelike as _, Duration, SecondsFormat, Utc};

/// A `<TIMESTAMP>` option value that is not the RFC 3339 spelling the option takes.
#[derive(Debug, thiserror::Error)]
pub enum TimestampError {
    #[error("invalid timestamp `{input}`: want {wanted}")]
    Invalid {
        input: String,
        /// What the option takes, as the sentence of the refusal says it.
        wanted: &'static str,
        /// The calendar's refusal, when the spelling is right and chrono still refuses it.
        #[source]
        source: Option<chrono::ParseError>,
    },
    /// The spelling is right and the date, the time or the offset does not exist, or the second is a leap second.
    #[error("invalid timestamp `{input}`: {what}")]
    Missing { input: String, what: String },
}

/// What a time option takes.
const WANTED_TIME: &str = "RFC 3339: a date `YYYY-MM-DD` or a date-time `YYYY-MM-DDTHH:MM:SSZ`";
/// What `--approval-at` takes.
const WANTED_DATE_TIME: &str = "an RFC 3339 date-time `YYYY-MM-DDTHH:MM:SSZ`";

/// `ASCII digits` at each of `at` in `bytes`.
fn digits(bytes: &[u8], at: &[usize]) -> bool {
    at.iter().all(|index| bytes.get(*index).is_some_and(u8::is_ascii_digit))
}

/// Whether `bytes` spell a full-date: `YYYY-MM-DD`, four digits, two, two.
fn is_full_date(bytes: &[u8]) -> bool {
    bytes.len() == 10 && digits(bytes, &[0, 1, 2, 3, 5, 6, 8, 9]) && bytes[4] == b'-' && bytes[7] == b'-'
}

/// Whether `text` is spelled as a date-time: a full-date, `T` or `t`, `HH:MM:SS`, an optional `.` and one or more digits, then `Z` or `z`, or
/// `+` or `-` and `HH:MM`.
fn is_date_time(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() < 20 || !is_full_date(&bytes[..10]) || !matches!(bytes[10], b'T' | b't') || !digits(bytes, &[11, 12, 14, 15, 17, 18]) || bytes[13] != b':' || bytes[16] != b':' {
        return false;
    }
    let mut at = 19;
    if bytes[at] == b'.' {
        let fraction = bytes[at + 1..].iter().take_while(|byte| byte.is_ascii_digit()).count();
        if fraction == 0 {
            return false;
        }
        at += 1 + fraction;
    }
    match &bytes[at..] {
        [b'Z' | b'z'] => true,
        [b'+' | b'-', ..] => bytes.len() - at == 6 && digits(bytes, &[at + 1, at + 2, at + 4, at + 5]) && bytes[at + 3] == b':',
        _ => false,
    }
}

fn missing(text: &str, what: String) -> TimestampError {
    TimestampError::Missing { input: text.to_string(), what }
}

/// The number written by the two digits at `at` of `bytes` (the shape holds them digits).
fn two(bytes: &[u8], at: usize) -> u32 {
    u32::from(bytes[at] - b'0') * 10 + u32::from(bytes[at + 1] - b'0')
}

/// Whether the full-date at the start of `bytes` (spelled right) names a day of the calendar.
fn check_date(text: &str, bytes: &[u8]) -> Result<(), TimestampError> {
    let year = bytes[..4].iter().fold(0_u32, |year, digit| year * 10 + u32::from(digit - b'0'));
    let (month, day) = (two(bytes, 5), two(bytes, 8));
    if !(1..=12).contains(&month) {
        return Err(missing(text, format!("month {month:02} does not exist (01-12)")));
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if !(1..=days).contains(&day) {
        return Err(missing(text, format!("day {day:02} does not exist in {}-{month:02} (it has {days} days)", &text[..4])));
    }
    Ok(())
}

/// The date-time of `text`, which [`is_date_time`] holds spelled right: the calendar's judgement of it, and the refusal of a leap second.
fn date_time(text: &str, wanted: &'static str) -> Result<DateTime<Utc>, TimestampError> {
    let bytes = text.as_bytes();
    check_date(text, bytes)?;
    let (hour, minute, second) = (two(bytes, 11), two(bytes, 14), two(bytes, 17));
    if hour > 23 {
        return Err(missing(text, format!("hour {hour:02} does not exist (00-23)")));
    }
    if minute > 59 {
        return Err(missing(text, format!("minute {minute:02} does not exist (00-59)")));
    }
    if second > 60 {
        return Err(missing(text, format!("second {second:02} does not exist (00-59)")));
    }
    if let [sign @ (b'+' | b'-'), ..] = &bytes[bytes.len() - 6.min(bytes.len())..] {
        let at = bytes.len() - 5;
        let (offset_hour, offset_minute) = (two(bytes, at), two(bytes, at + 3));
        if offset_hour > 23 || offset_minute > 59 {
            return Err(missing(text, format!("offset {}{} does not exist (00:00-23:59)", char::from(*sign), &text[at..])));
        }
    }
    let parse = |text: &str| DateTime::parse_from_rfc3339(text).map(|time| time.with_timezone(&Utc)).map_err(|source| TimestampError::Invalid { input: text.to_string(), wanted, source: Some(source) });
    if second == 60 {
        let same = parse(&format!("{}59{}", &text[..17], &text[19..])).ok().and_then(|time| time.checked_add_signed(Duration::seconds(1)));
        // A following second outside the years 0000-9999 has no RFC 3339 spelling in UTC: the refusal names no instant then.
        return Err(match same.filter(|time| (0..=9999).contains(&time.year())) {
            Some(time) => missing(text, format!("a leap second (second 60) is not accepted; the same instant is written `{}`", time.to_rfc3339_opts(SecondsFormat::AutoSi, true))),
            None => missing(text, "a leap second (second 60) is not accepted; the second after it is outside the years 0000-9999 and cannot be written".to_string()),
        });
    }
    parse(text)
}

/// The value parser of `--approval-at`: an RFC 3339 date-time (its `T` and `Z` in either case), as an instant in UTC.
pub fn parse_date_time(text: &str) -> Result<DateTime<Utc>, TimestampError> {
    if !is_date_time(text) {
        return Err(TimestampError::Invalid { input: text.to_string(), wanted: WANTED_DATE_TIME, source: None });
    }
    date_time(text, WANTED_DATE_TIME)
}

/// The value parser of a time option: an RFC 3339 full-date, which is that day's start in UTC, or an RFC 3339 date-time.
pub fn parse(text: &str) -> Result<DateTime<Utc>, TimestampError> {
    if is_full_date(text.as_bytes()) {
        check_date(text, text.as_bytes())?;
        return chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d")
            .map(|date| date.and_hms_opt(0, 0, 0).expect("midnight exists").and_utc())
            .map_err(|source| TimestampError::Invalid { input: text.to_string(), wanted: WANTED_TIME, source: Some(source) });
    }
    if !is_date_time(text) {
        return Err(TimestampError::Invalid { input: text.to_string(), wanted: WANTED_TIME, source: None });
    }
    date_time(text, WANTED_TIME)
}

/// The clap value parser of a time option that is read as a `DateTime<Utc>`: it takes the text and leaves every judgement to the stateless
/// stage (the declared domain says the spelling, `arg_rules::semantic` the calendar, in that order, ATIP 11.7 step 6). A text [`parse`]
/// refuses reads as the earliest instant here, and is never seen: the stage refuses the call first.
pub fn shape_only(text: &str) -> Result<DateTime<Utc>, std::convert::Infallible> {
    Ok(parse(text).unwrap_or(DateTime::<Utc>::MIN_UTC))
}

/// [`shape_only`] for `--approval-at` kept as an instant.
pub fn shape_only_date_time(text: &str) -> Result<DateTime<Utc>, std::convert::Infallible> {
    Ok(parse_date_time(text).unwrap_or(DateTime::<Utc>::MIN_UTC))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_date_is_utc_midnight_and_a_date_time_is_the_instant_it_names() {
        assert_eq!(parse("2030-01-01").unwrap().to_rfc3339(), "2030-01-01T00:00:00+00:00");
        assert_eq!(parse("2030-01-01T10:00:00+02:00").unwrap().to_rfc3339(), "2030-01-01T08:00:00+00:00");
        assert_eq!(parse_date_time("2030-01-01T10:00:00Z").unwrap(), parse_date_time("2030-01-01T19:00:00+09:00").unwrap());
        assert_eq!(parse_date_time("2030-01-01t10:00:00z").unwrap(), parse_date_time("2030-01-01T10:00:00Z").unwrap());
        assert_eq!(parse_date_time("2030-01-01T10:00:00.5Z").unwrap().to_rfc3339(), "2030-01-01T10:00:00.500+00:00");
    }

    #[test]
    fn only_rfc_3339_is_read() {
        for text in ["tomorrow", "2030-1-1", "2030-01-1", "2030-1-01", " 2030-01-01", "2030-01-01 ", "+2030-01-01", "-0001-01-01", "20300-01-01", "230-01-01", "2030- 01-01", "2030-01-01 10:00:00Z", "2030-01-01T10:00", "2030-01-01T10:00:00", "2030-01-01T10:00:00+0200", "2030-01-01T10:00:00+02", "2030-01-01T10:00:00\u{2212}02:00", "2030-01-01T10:00:00.Z", "2030-01-01T10:00:00Z ", " 2030-01-01T10:00:00Z"] {
            parse(text).unwrap_err();
        }
        for text in ["2030-01-01", "2026-02-30T10:00:00Z", "2030-01-01T25:00:00Z", "2030-01-01T10:00:00+24:00"] {
            parse_date_time(text).unwrap_err();
        }
        parse("2026-02-30").unwrap_err();
        assert!(parse("2030-01-01T10:00:00+09:00").is_ok() && parse_date_time("2030-01-01T10:00:59Z").is_ok());
        for text in ["2030-01-01T23:59:60Z", "1998-12-31T23:59:60Z", "2030-01-01T10:00:60Z", "2030-01-01T23:59:60.5Z", "2030-01-01T23:59:60+01:00"] {
            parse(text).unwrap_err();
            parse_date_time(text).unwrap_err();
        }
    }

    #[test]
    fn a_leap_second_says_it_is_not_accepted_and_the_same_instant() {
        assert_eq!(parse("2030-01-01T23:59:60Z").unwrap_err().to_string(), "invalid timestamp `2030-01-01T23:59:60Z`: a leap second (second 60) is not accepted; the same instant is written `2030-01-02T00:00:00Z`");
        assert!(parse_date_time("2030-01-01T23:59:60.5Z").unwrap_err().to_string().ends_with("`2030-01-02T00:00:00.500Z`"));
        assert!(parse_date_time("2030-01-01T23:59:60+01:00").unwrap_err().to_string().ends_with("`2030-01-01T23:00:00Z`"));
        // The following second past 9999-12-31 has no spelling: nothing is offered.
        for text in ["9999-12-31T23:59:60Z", "9999-12-31T23:59:60-01:00"] {
            let message = parse(text).unwrap_err().to_string();
            assert!(message.contains("cannot be written") && !message.contains("+10000") && !message.contains("same instant is written"), "{message}");
        }
        assert!(parse("9999-12-31T22:59:60Z").unwrap_err().to_string().ends_with("`9999-12-31T23:00:00Z`"));
        assert_eq!(parse_date_time("2030-01-01T23:59:59Z").unwrap() + Duration::seconds(1), parse_date_time("2030-01-02T00:00:00Z").unwrap());
        assert_eq!(parse("2030-01-02T00:00:00Z").unwrap().to_rfc3339(), "2030-01-02T00:00:00+00:00");
    }

    #[test]
    fn a_date_a_time_or_an_offset_that_does_not_exist_says_what() {
        let say = |text: &str| parse(text).unwrap_err().to_string();
        assert_eq!(say("2026-02-30"), "invalid timestamp `2026-02-30`: day 30 does not exist in 2026-02 (it has 28 days)");
        assert!(say("2026-02-29").contains("day 29 does not exist"));
        assert!(say("2026-04-31").contains("it has 30 days"));
        assert!(say("2026-13-01").contains("month 13 does not exist"));
        assert!(say("2026-01-01T25:00:00Z").contains("hour 25 does not exist"));
        assert!(say("2026-01-01T10:61:00Z").contains("minute 61 does not exist"));
        assert!(say("2026-01-01T10:00:61Z").contains("second 61 does not exist"));
        assert!(say("2026-01-01T10:00:00+24:00").contains("offset +24:00 does not exist"));
        assert!(say("2026-01-01T10:00:00-01:60").contains("offset -01:60 does not exist"));
        assert!(parse("2028-02-29").is_ok() && parse("2000-02-29").is_ok() && parse("1900-02-29").is_err());
        // A spelling error keeps its sentence.
        assert!(say("2026-2-30").contains("want RFC 3339"));
    }

    #[test]
    fn the_refusal_names_what_the_option_takes_and_keeps_its_cause() {
        assert_eq!(parse("tomorrow").unwrap_err().to_string(), "invalid timestamp `tomorrow`: want RFC 3339: a date `YYYY-MM-DD` or a date-time `YYYY-MM-DDTHH:MM:SSZ`");
        assert_eq!(parse_date_time("2030-01-01").unwrap_err().to_string(), "invalid timestamp `2030-01-01`: want an RFC 3339 date-time `YYYY-MM-DDTHH:MM:SSZ`");
        assert!(matches!(parse("2026-02-30").unwrap_err(), TimestampError::Missing { .. }));
        assert!(std::error::Error::source(&parse("tomorrow").unwrap_err()).is_none());
    }
}
