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
//! (`2026-02-30`, 25:00) is the calendar's: [`parse`] and [`parse_date_time`] refuse both, after the shape.

use chrono::{DateTime, NaiveDate, Utc};

/// A `<TIMESTAMP>` option value that is not the RFC 3339 spelling the option takes.
#[derive(Debug, thiserror::Error)]
pub enum TimestampError {
    #[error("invalid timestamp `{input}`: want {wanted}")]
    Invalid {
        input: String,
        /// What the option takes, as the sentence of the refusal says it.
        wanted: &'static str,
        /// The calendar's refusal, when the spelling is right and the date or the time does not exist.
        #[source]
        source: Option<chrono::ParseError>,
    },
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

/// The date-time of `text`, which [`is_date_time`] holds spelled right: the calendar's judgement of it.
fn date_time(text: &str, wanted: &'static str) -> Result<DateTime<Utc>, TimestampError> {
    DateTime::parse_from_rfc3339(text).map(|time| time.with_timezone(&Utc)).map_err(|source| TimestampError::Invalid { input: text.to_string(), wanted, source: Some(source) })
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
        return NaiveDate::parse_from_str(text, "%Y-%m-%d")
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
        assert!(parse("2030-01-01T10:00:00+09:00").is_ok() && parse_date_time("2030-01-01T10:00:60Z").is_ok());
    }

    #[test]
    fn the_refusal_names_what_the_option_takes_and_keeps_its_cause() {
        assert_eq!(parse("tomorrow").unwrap_err().to_string(), "invalid timestamp `tomorrow`: want RFC 3339: a date `YYYY-MM-DD` or a date-time `YYYY-MM-DDTHH:MM:SSZ`");
        assert_eq!(parse_date_time("2030-01-01").unwrap_err().to_string(), "invalid timestamp `2030-01-01`: want an RFC 3339 date-time `YYYY-MM-DDTHH:MM:SSZ`");
        assert!(std::error::Error::source(&parse("2026-02-30").unwrap_err()).is_some(), "the calendar's refusal stays reachable");
        assert!(std::error::Error::source(&parse("tomorrow").unwrap_err()).is_none());
    }
}
