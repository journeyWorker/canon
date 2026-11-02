//! RFC 8785 (JSON Canonicalization Scheme) for the values a tool digests: cursor bindings and plan digests.
//!
//! Two tools that digest the same value must get the same bytes, so the text is the one RFC 8785 gives: no whitespace,
//! object members sorted by the UTF-16 code units of their names (so `😀` sorts before `דּ`, as it does in JavaScript),
//! strings escaped with `\"`, `\\`, `\b`, `\t`, `\n`, `\f`, `\r` and `\u00xx` (lower-case hex) for the other control
//! characters and with nothing else, and `null`, `true` and `false` as they are. `null` members and empty arrays are kept.
//!
//! Numbers are the one place this module is narrower than the RFC. The values a tool digests (an argument normalised by its
//! type, a count, a record's size) are integers, and an integer outside ±(2^53-1) is not one every JSON reader holds exactly
//! (spec 10.7.5), so a number is written only when it is a whole number within that range: `1` and `1.0` are both `1`, `-0`
//! is `0`, and any other number (a fraction, an exponent beyond the range, `9007199254740993`) is an [`Error`] rather
//! than a digest that another implementation would compute differently.

use std::fmt;

use serde_json::{Number, Value};
use sha2::{Digest, Sha256};

/// Largest integer a JSON reader holds exactly (spec 10.7.5).
const SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// A value that cannot be written canonically here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A number that is not a whole number within ±(2^53-1), with the number as it was written.
    NumberNotSafe(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NumberNotSafe(number) => write!(f, "the number {number} is not a whole number within ±(2^53-1), which is all a digest holds (spec 10.7.5)"),
        }
    }
}

impl std::error::Error for Error {}

/// The SHA-256 of `bytes` as 64 lower-case hex digits: what a binding or a digest is made of.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// `value` as its RFC 8785 canonical JSON text.
pub fn to_string(value: &Value) -> Result<String, Error> {
    let mut out = String::new();
    write(value, &mut out)?;
    Ok(out)
}

fn write(value: &Value, out: &mut String) -> Result<(), Error> {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(number) => out.push_str(&integer(number)?),
        Value::String(text) => string(text, out),
        Value::Array(items) => {
            out.push('[');
            for (at, item) in items.iter().enumerate() {
                if at > 0 {
                    out.push(',');
                }
                write(item, out)?;
            }
            out.push(']');
        }
        Value::Object(members) => {
            let mut sorted: Vec<(&String, &Value)> = members.iter().collect();
            sorted.sort_by(|a, b| a.0.encode_utf16().cmp(b.0.encode_utf16()));
            out.push('{');
            for (at, (name, member)) in sorted.into_iter().enumerate() {
                if at > 0 {
                    out.push(',');
                }
                string(name, out);
                out.push(':');
                write(member, out)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

/// A whole number within ±(2^53-1) as its decimal digits.
fn integer(number: &Number) -> Result<String, Error> {
    if let Some(signed) = number.as_i64() {
        return if signed.unsigned_abs() <= SAFE_INTEGER { Ok(signed.to_string()) } else { Err(Error::NumberNotSafe(number.to_string())) };
    }
    if let Some(unsigned) = number.as_u64() {
        return if unsigned <= SAFE_INTEGER { Ok(unsigned.to_string()) } else { Err(Error::NumberNotSafe(number.to_string())) };
    }
    let float = number.as_f64().ok_or_else(|| Error::NumberNotSafe(number.to_string()))?;
    if float == 0.0 {
        return Ok("0".to_string());
    }
    // A whole float prints without a fraction or an exponent (`1e15` is `1000000000000000`).
    if float.fract() == 0.0 && float.abs() <= 9_007_199_254_740_991.0 {
        return Ok(float.to_string());
    }
    Err(Error::NumberNotSafe(number.to_string()))
}

fn string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            control if control < '\u{20}' => out.push_str(&format!("\\u{:04x}", u32::from(control))),
            other => out.push(other),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn canonical(text: &str) -> String {
        to_string(&serde_json::from_str(text).expect("JSON")).expect("canonical")
    }

    /// RFC 8785 section 3.2.3: members sort by UTF-16 code units, so the emoji (a surrogate pair, 0xD83D 0xDE00) is before
    /// U+FB33 although it is after it by code point.
    #[test]
    fn atip_cratecursor_12_rfc_8785_sorts_members_by_utf16_code_units() {
        let input = r#"{
            "\u20ac": "Euro Sign",
            "\r": "Carriage Return",
            "\ufb33": "Hebrew Letter Dalet With Dagesh",
            "1": "One",
            "\ud83d\ude00": "Emoji: Grinning Face",
            "\u0080": "Control",
            "\u00f6": "Latin Small Letter O With Diaeresis"
        }"#;
        let value: Value = serde_json::from_str(input).expect("JSON");
        let expected = "{\"\\r\":\"Carriage Return\",\"1\":\"One\",\"\u{80}\":\"Control\",\"\u{f6}\":\"Latin Small Letter O With Diaeresis\",\"\u{20ac}\":\"Euro Sign\",\"\u{1f600}\":\"Emoji: Grinning Face\",\"\u{fb33}\":\"Hebrew Letter Dalet With Dagesh\"}";
        assert_eq!(to_string(&value).expect("canonical"), expected);
    }

    /// RFC 8785 appendix E, with its numbers left out (a number there is a fraction or an exponent): the string with every
    /// escape, and the literals, which keep their `null`.
    #[test]
    fn atip_cratecursor_12_rfc_8785_writes_strings_and_literals_as_the_appendix_does() {
        let text = canonical(r#"{"string": "\u20ac$\u000F\u000aA'\u0042\u0022\u005c\\\"\/", "literals": [null, true, false]}"#);
        assert_eq!(text, "{\"literals\":[null,true,false],\"string\":\"€$\\u000f\\nA'B\\\"\\\\\\\\\\\"/\"}");
    }

    #[test]
    fn atip_cratecursor_12_a_control_character_is_escaped_short_where_the_rfc_has_a_short_form_and_by_u00xx_otherwise() {
        assert_eq!(to_string(&json!("\u{8}\t\n\u{c}\r\u{1}\u{1f}\u{7f}")).expect("canonical"), "\"\\b\\t\\n\\f\\r\\u0001\\u001f\u{7f}\"");
        assert_eq!(to_string(&json!("é😀\u{2028}")).expect("canonical"), "\"é😀\u{2028}\"", "everything else is written as it is, in UTF-8");
    }

    #[test]
    fn atip_cratecursor_12_whitespace_nulls_and_empty_members_are_kept_exactly() {
        assert_eq!(canonical(" { \"b\" : [ ] , \"a\" : { } , \"c\" : null } "), r#"{"a":{},"b":[],"c":null}"#);
        assert_eq!(canonical(r#"[{"b":2,"a":1},[3,{"z":null,"y":true}]]"#), r#"[{"a":1,"b":2},[3,{"y":true,"z":null}]]"#);
    }

    #[test]
    fn atip_cratecursor_12_integers_within_the_safe_range_are_written_and_a_whole_float_is_the_integer() {
        assert_eq!(canonical("[0,-0,1,1.0,-1.0,9007199254740991,-9007199254740991,1e2,12345678901234]"), "[0,0,1,1,-1,9007199254740991,-9007199254740991,100,12345678901234]");
        assert_eq!(canonical("[-0.0, 0.0]"), "[0,0]");
    }

    #[test]
    fn atip_cratecursor_12_a_number_that_is_not_a_safe_whole_number_is_an_error() {
        for text in ["1.5", "9007199254740992", "-9007199254740992", "18446744073709551615", "1e300", "0.1", "-2.5", "9007199254740993.0"] {
            let value: Value = serde_json::from_str(text).expect("JSON");
            assert!(matches!(to_string(&value), Err(Error::NumberNotSafe(_))), "{text}");
            // Inside a document too.
            assert!(to_string(&json!({ "a": [value] })).is_err(), "{text}");
        }
        assert!(to_string(&json!({ "a": 1.5 })).expect_err("refused").to_string().contains("1.5"));
    }
}
