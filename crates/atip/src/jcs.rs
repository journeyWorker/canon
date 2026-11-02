//! RFC 8785 (JSON Canonicalization Scheme) for the values a tool digests: cursor bindings and plan digests.
//!
//! Two tools that digest the same value must get the same bytes, so the text is the one RFC 8785 gives: no whitespace,
//! object members sorted by the UTF-16 code units of their names (so `😀` sorts before `דּ`, as it does in JavaScript),
//! strings escaped with `\"`, `\\`, `\b`, `\t`, `\n`, `\f`, `\r` and `\u00xx` (lower-case hex) for the other control
//! characters and with nothing else, and `null`, `true` and `false` as they are. `null` members and empty arrays are kept.
//!
//! Numbers are written as RFC 8785 section 3.2.2 says: the ECMAScript `Number` to string form of the IEEE 754 double (shortest
//! digits that read back, no exponent from `1e-6` up to below `1e21`, `e+21` and `e-7` forms outside it, `-0` as `0`), so `1`,
//! `1.0`, `1e0` and `100.000` are `1` and `1`, `1` and `100`. A JSON number written with a fraction or an exponent is a double
//! here, as it is to every other implementation. An integer too large to be one is the exception, and an [`Error`]: a number
//! that serde reads as a 64-bit integer outside ±(2^53-1) is not one every JSON reader holds exactly (spec 10.7.5), and its digest
//! would depend on the implementation. A plan or an argument holds such an integer as a string (examples/README.md, "Plan
//! digests"). An integer written with more digits than 64 bits hold is read as a double before it gets here and is written as
//! one, which is the rounding the README warns of: a tool puts it in as a string. A string holds only valid Unicode, which a
//! Rust `str` always is.

use std::fmt;

use serde_json::{Number, Value};
use sha2::{Digest, Sha256};

/// Largest integer a JSON reader holds exactly (spec 10.7.5).
const SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// A value that cannot be written canonically here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// An integer outside ±(2^53-1) that a digest cannot hold exactly, with the number as it was written: a tool writes it as a string.
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

/// A number as RFC 8785 writes it: an integer within ±(2^53-1) as its digits, any other as the ECMAScript form of the double.
fn integer(number: &Number) -> Result<String, Error> {
    if let Some(signed) = number.as_i64() {
        return if signed.unsigned_abs() <= SAFE_INTEGER { Ok(signed.to_string()) } else { Err(Error::NumberNotSafe(number.to_string())) };
    }
    if let Some(unsigned) = number.as_u64() {
        return if unsigned <= SAFE_INTEGER { Ok(unsigned.to_string()) } else { Err(Error::NumberNotSafe(number.to_string())) };
    }
    match number.as_f64() {
        Some(float) if float.is_finite() => Ok(ecmascript(float)),
        _ => Err(Error::NumberNotSafe(number.to_string())),
    }
}

/// `value` (finite) as ECMAScript `Number::toString` (ECMA-262 6.1.6.1.20) writes it, which is what RFC 8785 3.2.2 requires.
fn ecmascript(value: f64) -> String {
    if value == 0.0 {
        return "0".to_string();
    }
    let sign = if value < 0.0 { "-" } else { "" };
    // `{:e}` writes the shortest digits that read back as the same double: `d.ddde<exponent>`. When two numbers of that many digits
    // are equally close to the double, ECMAScript takes the even one (`1424953923781206.25` is `1424953923781206.2`) where the
    // shortest-digits writer rounds up, so the digits are the correctly rounded value of that length when it reads back.
    let shortest = format!("{:e}", value.abs());
    let digit_count = shortest.split_once('e').map_or(1, |(mantissa, _)| mantissa.chars().filter(char::is_ascii_digit).count());
    let rounded = format!("{:.*e}", digit_count.saturating_sub(1), value.abs());
    let scientific = if rounded.parse::<f64>().is_ok_and(|read_back| read_back == value.abs()) { rounded } else { shortest };
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let k = i32::try_from(digits.len()).unwrap_or(i32::MAX);
    // value = 0.DIGITS * 10^n
    let n = exponent.parse::<i32>().unwrap_or_default() + 1;
    let body = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat(usize::try_from(n - k).unwrap_or_default()))
    } else if 0 < n && n <= 21 {
        let (whole, fraction) = digits.split_at(usize::try_from(n).unwrap_or_default());
        format!("{whole}.{fraction}")
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat(usize::try_from(-n).unwrap_or_default()))
    } else {
        let power = n - 1;
        let sign_of_power = if power < 0 { "-" } else { "+" };
        let (first, rest) = digits.split_at(1);
        let fraction = if rest.is_empty() { String::new() } else { format!(".{rest}") };
        format!("{first}{fraction}e{sign_of_power}{}", power.abs())
    };
    format!("{sign}{body}")
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
    fn atip_cratecursor_12_an_integer_beyond_the_safe_range_is_an_error_and_every_other_number_is_the_ecmascript_form_of_its_double() {
        // 64-bit integers outside ±(2^53-1) are refused, whatever their sign, alone or inside a document.
        for text in ["9007199254740992", "-9007199254740992", "9007199254740993", "18446744073709551615", "-9223372036854775808"] {
            let value: Value = serde_json::from_str(text).expect("JSON");
            assert!(matches!(to_string(&value), Err(Error::NumberNotSafe(_))), "{text}");
            assert!(to_string(&json!({ "a": [value] })).is_err(), "{text}");
        }
        assert!(to_string(&json!({ "a": 9_007_199_254_740_993_u64 })).expect_err("refused").to_string().contains("9007199254740993"));
        // A number written with a fraction or an exponent is a double, as it is to every other implementation.
        assert_eq!(canonical("[1.5,0.1,-2.5,1e300,9007199254740993.0,4.50,2e-3,1E-27,5e-324,1e21,1e20,0.000001,1e-7]"), "[1.5,0.1,-2.5,1e+300,9007199254740992,4.5,0.002,1e-27,5e-324,1e+21,100000000000000000000,0.000001,1e-7]");
    }

    /// RFC 8785 Appendix B: IEEE 754 bit patterns and their canonical text.
    #[test]
    fn atip_cratecursor_12_rfc_8785_appendix_b_number_forms() {
        let table = [
            ("0000000000000000", "0"),
            ("8000000000000000", "0"),
            ("0000000000000001", "5e-324"),
            ("8000000000000001", "-5e-324"),
            ("7fefffffffffffff", "1.7976931348623157e+308"),
            ("ffefffffffffffff", "-1.7976931348623157e+308"),
            ("4340000000000000", "9007199254740992"),
            ("c340000000000000", "-9007199254740992"),
            ("4430000000000000", "295147905179352830000"),
            ("44b52d02c7e14af5", "9.999999999999997e+22"),
            ("44b52d02c7e14af6", "1e+23"),
            ("44b52d02c7e14af7", "1.0000000000000001e+23"),
            ("444b1ae4d6e2ef4e", "999999999999999700000"),
            ("444b1ae4d6e2ef4f", "999999999999999900000"),
            ("444b1ae4d6e2ef50", "1e+21"),
            ("3eb0c6f7a0b5ed8c", "9.999999999999997e-7"),
            ("3eb0c6f7a0b5ed8d", "0.000001"),
            ("41b3de4355555553", "333333333.3333332"),
            ("41b3de4355555554", "333333333.33333325"),
            ("41b3de4355555555", "333333333.3333333"),
            ("41b3de4355555556", "333333333.3333334"),
            ("41b3de4355555557", "333333333.33333343"),
            ("becbf647612f3696", "-0.0000033333333333333333"),
            ("43143ff3c1cb0959", "1424953923781206.2"),
        ];
        for (bits, text) in table {
            let value = f64::from_bits(u64::from_str_radix(bits, 16).expect("hex"));
            let number = Value::Number(Number::from_f64(value).expect("a finite double"));
            assert_eq!(to_string(&number).expect("canonical"), text, "{bits}");
        }
    }
}
