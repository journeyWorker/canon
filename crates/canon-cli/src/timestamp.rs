//! The one `<TIMESTAMP>` grammar of every option (0.28 O11): RFC3339, or a
//! calendar date `YYYY-MM-DD` read as that day's start in UTC.

use chrono::{DateTime, NaiveDate, Utc};

/// A `<TIMESTAMP>` option value that is neither RFC3339 nor `YYYY-MM-DD`.
#[derive(Debug, thiserror::Error)]
pub enum TimestampError {
    #[error("invalid timestamp `{input}`: want RFC3339 or YYYY-MM-DD")]
    Invalid {
        input: String,
        #[source]
        source: chrono::ParseError,
    },
}

/// `DateTime<Utc>` value parser of a `<TIMESTAMP>` option.
pub fn parse(s: &str) -> Result<DateTime<Utc>, TimestampError> {
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(date.and_hms_opt(0, 0, 0).expect("midnight exists").and_utc());
    }
    DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc)).map_err(|source| TimestampError::Invalid { input: s.to_string(), source })
}

/// Value parser of a `<TIMESTAMP>` option that a handler keeps as text (a
/// signed payload embeds the exact string): an RFC3339 value is kept as
/// typed, a date becomes its UTC-midnight RFC3339 form.
pub fn normalize(s: &str) -> Result<String, TimestampError> {
    match NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        Ok(date) => Ok(format!("{date}T00:00:00Z")),
        Err(_) => parse(s).map(|_| s.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_date_is_utc_midnight_and_rfc3339_is_kept() {
        assert_eq!(parse("2030-01-01").unwrap().to_rfc3339(), "2030-01-01T00:00:00+00:00");
        parse("2030-01-01T10:00:00+02:00").unwrap();
        parse("tomorrow").unwrap_err();
        assert_eq!(normalize("2030-01-01").unwrap(), "2030-01-01T00:00:00Z");
        assert_eq!(normalize("2030-01-01T10:00:00Z").unwrap(), "2030-01-01T10:00:00Z");
    }

    #[test]
    fn the_refusal_reads_as_before_and_keeps_its_cause() {
        let error = parse("tomorrow").unwrap_err();
        assert_eq!(error.to_string(), "invalid timestamp `tomorrow`: want RFC3339 or YYYY-MM-DD");
        assert!(std::error::Error::source(&error).is_some(), "the chrono parse error stays reachable");
        assert_eq!(normalize("tomorrow").unwrap_err().to_string(), "invalid timestamp `tomorrow`: want RFC3339 or YYYY-MM-DD");
    }
}
