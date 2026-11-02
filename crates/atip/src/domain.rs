//! Domains: the restrictions of one argument's own value beyond its `valueType` (spec 10.7.4, approaches/rust.md 4.3).
//!
//! A domain is written in the contract's fixed profile of JSON Schema, so that a clap-based tool and an Effect-based
//! tool can each turn it into parser checks, and a checker can generate boundary values from it. This module holds
//! what every tool shares:
//!
//! - [`Domain`], a typed form a tool writes in its table so that only the profile can be written;
//! - [`profile_problems`] and [`pattern_problem`], the structural check of 10.7.4 and 10.13 rule 3: the keywords
//!   allowed for a `valueType`, safe-integer bounds, and a `pattern` that is `^`, a restricted I-Regexp (RFC 9485)
//!   body and `$` and that the Rust `regex` crate compiles with its default limits;
//! - [`admits`], whether a value is inside a domain, so a document's defaults, examples and boundary values can be held
//!   to it;
//! - [`implicit`] and [`merge`], the restriction a Rust integer type carries (`u32` refuses negatives and 2^32) and
//!   the combination of that with the tool's table.
//!
//! The I-Regexp scanner is a port of `iregexpProblem` in the contract's `scripts/check.ts`: the two must give the
//! same answer, and the contract's `scripts/regex-cases.json` is the record both are held to.

#[cfg(feature = "validate")]
use jsonschema::Validator;
use serde_json::{json, Map, Value};

use crate::doc::ValueType;
use crate::judge::{Node, Refused};

/// Largest integer a JSON parser, JavaScript and a Rust `i64` all read the same (spec 10.7.5).
pub const SAFE_INTEGER: i64 = 9_007_199_254_740_991;

/// A domain as a tool's table writes it: only the profile of `integer`, `count`, `string` and `path` arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    /// Bounds of an `integer` or `count`, each within ±(2^53-1).
    Integer { minimum: Option<i64>, maximum: Option<i64> },
    /// Length in characters and a `pattern` of a `string` or `path`.
    Text { min_length: Option<u64>, max_length: Option<u64>, pattern: Option<&'static str> },
}

impl Domain {
    /// An integer of at least `minimum`.
    pub const fn at_least(minimum: i64) -> Domain {
        Domain::Integer { minimum: Some(minimum), maximum: None }
    }

    /// A text of at least `min_length` characters.
    pub const fn longer_than(min_length: u64) -> Domain {
        Domain::Text { min_length: Some(min_length), max_length: None, pattern: None }
    }

    /// A text that is `pattern` (`^...$`).
    pub const fn matching(pattern: &'static str) -> Domain {
        Domain::Text { min_length: None, max_length: None, pattern: Some(pattern) }
    }

    /// The domain as the JSON Schema member of an option or an operand.
    pub fn to_value(&self) -> Value {
        let mut map = Map::new();
        match *self {
            Domain::Integer { minimum, maximum } => {
                if let Some(minimum) = minimum {
                    map.insert("minimum".to_string(), json!(minimum));
                }
                if let Some(maximum) = maximum {
                    map.insert("maximum".to_string(), json!(maximum));
                }
            }
            Domain::Text { min_length, max_length, pattern } => {
                if let Some(min_length) = min_length {
                    map.insert("minLength".to_string(), json!(min_length));
                }
                if let Some(max_length) = max_length {
                    map.insert("maxLength".to_string(), json!(max_length));
                }
                if let Some(pattern) = pattern {
                    map.insert("pattern".to_string(), json!(pattern));
                }
            }
        }
        Value::Object(map)
    }
}

const NUMERIC: [&str; 5] = ["minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum", "multipleOf"];
const STRING: [&str; 3] = ["minLength", "maxLength", "pattern"];
const STRUCTURAL: [&str; 7] = ["type", "enum", "const", "properties", "required", "additionalProperties", "items"];
const ANNOTATIONS: [&str; 4] = ["title", "description", "$comment", "format"];

/// The keywords a domain of `value_type` may use (spec 10.7.4), or `None` when the type has no domain.
fn keywords(value_type: ValueType) -> Option<Vec<&'static str>> {
    match value_type {
        ValueType::Integer | ValueType::Number | ValueType::Count => Some(NUMERIC.to_vec()),
        ValueType::String | ValueType::Path => Some(STRING.to_vec()),
        ValueType::Json => Some(NUMERIC.iter().chain(STRING.iter()).chain(STRUCTURAL.iter()).copied().collect()),
        ValueType::Switch | ValueType::Enum => None,
    }
}

/// Every way `domain` leaves the profile of `value_type` (spec 10.7.4, 10.13 rule 3): a keyword outside it, a
/// bound that is not a finite number (or, for an integer or a count, not a safe integer), a `multipleOf` that is not positive, a
/// `pattern` that is not of the restricted form or that the Rust `regex` crate refuses, a non-scalar `enum` or `const`.
pub fn profile_problems(value_type: ValueType, domain: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    check_node(value_type, domain, "the domain", &mut problems);
    problems
}

fn check_node(value_type: ValueType, node: &Value, at: &str, problems: &mut Vec<String>) {
    let Some(allowed) = keywords(value_type) else {
        problems.push(format!("{at}: a {} argument has no domain (spec 10.7.4)", value_type.as_str()));
        return;
    };
    let Some(map) = node.as_object() else {
        problems.push(format!("{at}: a domain schema is an object"));
        return;
    };
    if map.is_empty() && at == "the domain" {
        problems.push(format!("{at}: is empty: a domain states at least one restriction"));
    }
    let integral = matches!(value_type, ValueType::Integer | ValueType::Count);
    for (keyword, value) in map {
        if ANNOTATIONS.contains(&keyword.as_str()) {
            continue;
        }
        if !allowed.contains(&keyword.as_str()) {
            problems.push(format!("{at}: keyword {keyword} is outside the domain profile of {} (spec 10.7.4)", value_type.as_str()));
            continue;
        }
        if NUMERIC.contains(&keyword.as_str()) {
            match value.as_f64() {
                None => problems.push(format!("{at}: {keyword} is not a finite number")),
                Some(number) if !number.is_finite() => problems.push(format!("{at}: {keyword} is not a finite number")),
                Some(number) if integral && !is_safe_integer(value) => problems.push(format!("{at}: {keyword} {number} is not a safe integer (spec 10.7.5)")),
                Some(number) if keyword == "multipleOf" && number <= 0.0 => problems.push(format!("{at}: multipleOf is not positive")),
                Some(_) => {}
            }
        }
        if keyword == "pattern" {
            match value.as_str() {
                Some(pattern) => {
                    if let Some(problem) = pattern_problem(pattern) {
                        problems.push(format!("{at}: pattern {value}: {problem} (spec 10.7.4)"));
                    }
                }
                None => problems.push(format!("{at}: pattern {value}: is not a string (spec 10.7.4)")),
            }
        }
        if (keyword == "enum" || keyword == "const") && std::slice::from_ref(value).iter().flat_map(|v| v.as_array().map_or_else(|| vec![v], |items| items.iter().collect())).any(|item| item.is_object() || item.is_array()) {
            problems.push(format!("{at}: {keyword} admits only scalar values in a domain (spec 10.7.4)"));
        }
        if keyword == "properties" {
            if let Some(children) = value.as_object() {
                for (name, child) in children {
                    check_node(value_type, child, &format!("{at}/properties/{name}"), problems);
                }
            }
        }
        if keyword == "items" || (keyword == "additionalProperties" && !value.is_boolean()) {
            check_node(value_type, value, &format!("{at}/{keyword}"), problems);
        }
    }
}

fn is_safe_integer(value: &Value) -> bool {
    value.as_i64().is_some_and(|number| number.abs() <= SAFE_INTEGER) || value.as_f64().is_some_and(|number| number.fract() == 0.0 && number.abs() <= SAFE_INTEGER as f64)
}

/// What is wrong with `pattern` as a domain `pattern` (spec 10.7.4): `None` when it is `^`, an I-Regexp body with the
/// contract's restrictions, and `$`, and the Rust `regex` crate compiles it with its default limits (a compiled size of
/// 10 MiB, a nesting depth of 250).
pub fn pattern_problem(pattern: &str) -> Option<String> {
    let body = match pattern.strip_prefix('^').and_then(|rest| rest.strip_suffix('$')) {
        Some(body) if pattern.chars().count() >= 2 => body,
        _ => return Some("is not ^…$".to_string()),
    };
    if let Some(at) = pattern.chars().position(|c| matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')) {
        return Some(format!("carries a raw line terminator at offset {at}: the contract's schema requires `^\\^.*\\$$`, and `.` does not match one (write `\\n`, `\\r`, `\\p{{Zl}}`, `\\p{{Zp}}`)"));
    }
    if let Some(problem) = iregexp_problem(body) {
        return Some(problem);
    }
    regex::Regex::new(pattern).err().map(|error| format!("the Rust regex crate refuses it with its default limits: {error}"))
}

/// The I-Regexp (RFC 9485) syntax of the body of a domain pattern with the restrictions of spec 10.7.4.
/// A port of `iregexpProblem` of the contract's `scripts/check.ts`.
fn iregexp_problem(body: &str) -> Option<String> {
    let mut scan = Scan { cps: body.chars().collect(), i: 0 };
    scan.alternatives(0).and_then(|()| if scan.i < scan.cps.len() { scan.fail(&format!("unmatched {}", scan.cps[scan.i])) } else { Ok(()) }).err()
}

struct Scan {
    cps: Vec<char>,
    i: usize,
}

type Scanned = Result<(), String>;

const SINGLE_ESCAPES: [char; 17] = ['(', ')', '*', '+', '-', '.', '?', '[', '\\', ']', '^', 'n', 'r', 't', '{', '|', '}'];

fn is_category_name(name: &str) -> bool {
    let mut chars = name.chars();
    let (Some(major), rest) = (chars.next(), chars.as_str()) else { return false };
    let minors = match major {
        'L' => "lmotu",
        'M' => "cen",
        'N' => "dlo",
        'P' => "cdefios",
        'Z' => "lps",
        'S' => "ckmo",
        'C' => "cfno",
        _ => return false,
    };
    let mut rest = rest.chars();
    match (rest.next(), rest.next()) {
        (None, _) => true,
        (Some(minor), None) => minors.contains(minor),
        _ => false,
    }
}

impl Scan {
    fn at(&self, offset: usize) -> Option<char> {
        self.cps.get(self.i + offset).copied()
    }

    fn fail<T>(&self, what: &str) -> Result<T, String> {
        Err(format!("{what} at offset {}", self.i))
    }

    fn is_category(&self) -> bool {
        self.at(0) == Some('\\') && matches!(self.at(1), Some('p' | 'P'))
    }

    fn escape(&mut self, in_class: bool) -> Scanned {
        if self.is_category() {
            if self.at(2) != Some('{') {
                return self.fail("\\p without {");
            }
            let Some(end) = self.cps[self.i + 3..].iter().position(|c| *c == '}').map(|at| at + self.i + 3) else {
                return self.fail("unclosed \\p{");
            };
            let name: String = self.cps[self.i + 3..end].iter().collect();
            if !is_category_name(&name) {
                return self.fail(&format!("unknown category {name}"));
            }
            self.i = end + 1;
            return Ok(());
        }
        let c = self.at(1);
        match c {
            Some(c) if SINGLE_ESCAPES.contains(&c) => {
                if c == '-' && !in_class {
                    return self.fail("\\- outside a character class");
                }
                self.i += 2;
                Ok(())
            }
            Some(c) => self.fail(&format!("escape \\{c} is not I-Regexp")),
            None => self.fail("escape \\ is not I-Regexp"),
        }
    }

    fn class_char(&mut self) -> Result<bool, String> {
        let c = self.at(0);
        if c == Some('\\') {
            self.escape(true)?;
            return Ok(true);
        }
        match c {
            None | Some('-' | '[' | ']') => Ok(false),
            Some(c) => {
                if (c == '&' || c == '~') && self.at(1) == Some(c) {
                    return self.fail(&format!("{c}{c} in a character class (a set operation in Rust)"));
                }
                self.i += 1;
                Ok(true)
            }
        }
    }

    fn char_class(&mut self) -> Scanned {
        self.i += 1;
        if self.at(0) == Some('^') {
            self.i += 1;
        }
        let mut items = 0;
        if self.at(0) == Some('-') {
            self.i += 1;
            items += 1;
        }
        while self.at(0) != Some(']') {
            if self.at(0).is_none() {
                return self.fail("unclosed [");
            }
            if self.is_category() {
                self.escape(true)?;
            } else if self.at(0) == Some('-') && self.at(1) == Some(']') {
                self.i += 1;
            } else {
                if !self.class_char()? {
                    return self.fail(&format!("{} in a character class", self.at(0).map_or_else(String::new, |c| c.to_string())));
                }
                if self.at(0) == Some('-') && self.at(1) != Some(']') {
                    self.i += 1;
                    if self.is_category() || !self.class_char()? {
                        return self.fail("bad range");
                    }
                }
            }
            items += 1;
        }
        if items == 0 {
            return self.fail("empty character class");
        }
        self.i += 1;
        Ok(())
    }

    fn digits(&mut self) -> Scanned {
        if !self.at(0).is_some_and(|c| c.is_ascii_digit()) {
            return self.fail("quantifier without digits");
        }
        while self.at(0).is_some_and(|c| c.is_ascii_digit()) {
            self.i += 1;
        }
        Ok(())
    }

    fn quantifier(&mut self) -> Scanned {
        match self.at(0) {
            Some('*' | '+' | '?') => self.i += 1,
            Some('{') => {
                self.i += 1;
                self.digits()?;
                if self.at(0) == Some(',') {
                    self.i += 1;
                    if self.at(0).is_some_and(|c| c.is_ascii_digit()) {
                        self.digits()?;
                    }
                }
                if self.at(0) != Some('}') {
                    return self.fail("unclosed {");
                }
                self.i += 1;
            }
            _ => {}
        }
        Ok(())
    }

    fn alternatives(&mut self, depth: usize) -> Scanned {
        self.branch(depth)?;
        while self.at(0) == Some('|') {
            if depth == 0 {
                return self.fail("| outside parentheses");
            }
            self.i += 1;
            self.branch(depth)?;
        }
        Ok(())
    }

    fn branch(&mut self, depth: usize) -> Scanned {
        while let Some(c) = self.at(0) {
            if c == '|' || c == ')' {
                break;
            }
            match c {
                '(' => {
                    self.i += 1;
                    self.alternatives(depth + 1)?;
                    if self.at(0) != Some(')') {
                        return self.fail("unclosed (");
                    }
                    self.i += 1;
                }
                '[' => self.char_class()?,
                '\\' => self.escape(false)?,
                '.' => return self.fail(". outside a character class"),
                '^' | '$' => return self.fail(&format!("{c} outside a character class")),
                '*' | '+' | '?' | '{' | '}' | ']' => return self.fail(&format!("{c} without an atom")),
                _ => self.i += 1,
            }
            self.quantifier()?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- evaluation

/// Whether `value` is inside `domain`. `value` is the argument's value after normalisation (an integer as a JSON
/// number, a string as it is). A domain that does not compile is an error.
///
/// The domain is judged by the crate itself (the module `judge`): every keyword of the profile (spec 10.7.4) is read
/// there, a `pattern` with the `regex` crate, and the answer is the one `compile` gives. A keyword of JSON Schema outside
/// the profile (`anyOf`, `$ref`, `minItems`, ...) is judged only with the feature `validate`; without it the domain is
/// an error that names the keyword.
pub fn admits(domain: &Value, value: &Value) -> Result<bool, String> {
    match Node::compile(domain) {
        Ok(node) => Ok(node.is_valid(value)),
        Err(Refused::Invalid(why)) => Err(format!("the domain {domain} does not compile: {why}")),
        Err(Refused::Unsupported(keyword)) => beyond_the_profile(domain, value, &keyword),
    }
}

#[cfg(feature = "validate")]
fn beyond_the_profile(domain: &Value, value: &Value, _keyword: &str) -> Result<bool, String> {
    compile(domain).map(|compiled| compiled.admits(value))
}

#[cfg(not(feature = "validate"))]
fn beyond_the_profile(domain: &Value, _value: &Value, keyword: &str) -> Result<bool, String> {
    Err(format!("the domain {domain} uses `{keyword}`, which is outside the domain profile and is judged only with the feature `validate`"))
}

/// A domain compiled by the JSON Schema validator, once, for a caller that asks it of many values. The reference
/// [`admits`] is held to; it needs the feature `validate`.
#[cfg(feature = "validate")]
pub struct Compiled(Validator);

#[cfg(feature = "validate")]
impl std::fmt::Debug for Compiled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Compiled(..)")
    }
}

#[cfg(feature = "validate")]
impl Compiled {
    /// Whether `value` is inside the domain.
    pub fn admits(&self, value: &Value) -> bool {
        self.0.is_valid(value)
    }
}

/// Compiles `domain` with the JSON Schema validator so that many values can be asked of it without compiling it each
/// time. Needs the feature `validate`.
#[cfg(feature = "validate")]
pub fn compile(domain: &Value) -> Result<Compiled, String> {
    jsonschema::validator_for(domain).map(Compiled).map_err(|error| format!("the domain {domain} does not compile: {error}"))
}

/// The restriction the Rust type of an integer argument carries, which no `valueType` says: an unsigned type refuses
/// a negative value and a narrow one its overflow. `None` for a type the safe range already bounds (`i64`).
pub fn implicit(bits: u32, signed: bool) -> Option<Domain> {
    let (minimum, maximum): (i128, i128) = match (bits, signed) {
        (64 | 128, true) => return None,
        (_, true) => (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1),
        (_, false) => (0, (1i128 << bits) - 1),
    };
    let safe = i128::from(SAFE_INTEGER);
    let clamp = |bound: i128| i64::try_from(bound.clamp(-safe, safe)).unwrap_or(0);
    let minimum = (minimum > -safe).then(|| clamp(minimum));
    let maximum = (maximum < safe).then(|| clamp(maximum));
    (minimum.is_some() || maximum.is_some()).then_some(Domain::Integer { minimum, maximum })
}

/// The two domains together: the tighter of each bound, a pattern the one that has it. Two different patterns have no
/// combination and are an error: a tool states one.
pub fn merge(a: Option<Value>, b: Option<Value>) -> Result<Option<Value>, String> {
    let (a, b) = match (a, b) {
        (None, other) | (other, None) => return Ok(other),
        (Some(a), Some(b)) => (a, b),
    };
    let (Some(first), Some(second)) = (a.as_object(), b.as_object()) else { return Err("a domain is an object".to_string()) };
    let mut merged = first.clone();
    for (keyword, value) in second {
        match merged.get(keyword.as_str()).cloned() {
            None => {
                merged.insert(keyword.clone(), value.clone());
            }
            Some(existing) if existing == *value => {}
            Some(existing) => {
                let (Some(old), Some(new)) = (existing.as_f64(), value.as_f64()) else {
                    return Err(format!("the domains give `{keyword}` two values: {existing} and {value}"));
                };
                let take_new = match keyword.as_str() {
                    "minimum" | "exclusiveMinimum" | "minLength" => new > old,
                    "maximum" | "exclusiveMaximum" | "maxLength" => new < old,
                    _ => return Err(format!("the domains give `{keyword}` two values: {existing} and {value}")),
                };
                if take_new {
                    merged.insert(keyword.clone(), value.clone());
                }
            }
        }
    }
    Ok(Some(Value::Object(merged)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pattern_of_the_restricted_form_passes_and_each_removed_spelling_is_named() {
        for good in ["^[a-z][a-z0-9-]*$", "^(core|lead)$", "^\\p{L}{1,10}$", "^[^\\n\\r]+$", "^[a-z]+(-[a-z0-9]+)*$", "^[ \\t]*x$", "^a\\.b$", "^[^\\n\\r\\p{Zl}\\p{Zp}]+$"] {
            assert_eq!(pattern_problem(good), None, "{good}");
        }
        let bad = [
            ("a+", "is not ^…$"),
            ("^a|b$", "| outside parentheses"),
            ("^a.b$", ". outside a character class"),
            ("^\\d+$", "escape \\d is not I-Regexp"),
            ("^a\\-b$", "\\- outside a character class"),
            ("^[a&&b]+$", "&& in a character class"),
            ("^[a~~b]+$", "~~ in a character class"),
            ("^[[:alpha:]]$", "in a character class"),
            ("^[a--b]$", "bad range"),
            ("^\\p{Xx}$", "unknown category"),
            ("^*a$", "* without an atom"),
            ("^(a$", "unclosed ("),
            ("^[]$", "empty character class"),
            ("^[^\n]+$", "raw line terminator"),
            ("^[^\u{2028}]+$", "raw line terminator"),
            ("^a^b$", "^ outside a character class"),
        ];
        for (pattern, expected) in bad {
            let problem = pattern_problem(pattern).unwrap_or_else(|| panic!("{pattern} should be refused"));
            assert!(problem.contains(expected), "{pattern}: {problem}");
        }
    }

    #[test]
    fn a_pattern_the_regex_crate_refuses_with_its_default_limits_is_refused() {
        let problem = pattern_problem("^\\p{L}{1,500}$").expect("too large for the default limits");
        assert!(problem.contains("regex crate"), "{problem}");
    }

    #[test]
    fn the_profile_is_by_value_type_and_bounds_are_safe_integers() {
        assert!(profile_problems(ValueType::Integer, &json!({ "minimum": 1, "maximum": 4_294_967_295_u64 })).is_empty());
        assert!(!profile_problems(ValueType::Integer, &json!({ "minLength": 1 })).is_empty());
        assert!(!profile_problems(ValueType::Integer, &json!({ "minimum": 9_007_199_254_740_992_i64 })).is_empty());
        assert!(profile_problems(ValueType::Number, &json!({ "maximum": 1e20 })).is_empty());
        assert!(profile_problems(ValueType::String, &json!({ "minLength": 1, "pattern": "^[a-z]+$", "format": "uri" })).is_empty());
        assert!(!profile_problems(ValueType::String, &json!({ "anyOf": [] })).is_empty());
        assert!(!profile_problems(ValueType::Switch, &json!({ "minimum": 1 })).is_empty());
        assert!(!profile_problems(ValueType::Enum, &json!({ "minimum": 1 })).is_empty());
        assert!(!profile_problems(ValueType::Integer, &json!({})).is_empty());
    }

    #[test]
    fn admits_follows_the_keywords() {
        let domain = json!({ "minimum": 1, "maximum": 4_294_967_295_u64 });
        assert_eq!(admits(&domain, &json!(1)), Ok(true));
        assert_eq!(admits(&domain, &json!(0)), Ok(false));
        assert_eq!(admits(&domain, &json!(4_294_967_295_u64)), Ok(true));
        assert_eq!(admits(&domain, &json!(4_294_967_296_u64)), Ok(false));
        let text = json!({ "minLength": 1, "pattern": "^[^\\n\\r]*$" });
        assert_eq!(admits(&text, &json!("a b")), Ok(true));
        assert_eq!(admits(&text, &json!("")), Ok(false));
        assert_eq!(admits(&text, &json!("a\nb")), Ok(false));
    }

    #[test]
    fn a_rust_integer_type_carries_its_own_bounds_and_the_safe_range_bounds_the_wide_ones() {
        assert_eq!(implicit(32, false), Some(Domain::Integer { minimum: Some(0), maximum: Some(4_294_967_295) }));
        assert_eq!(implicit(16, false), Some(Domain::Integer { minimum: Some(0), maximum: Some(65_535) }));
        assert_eq!(implicit(64, false), Some(Domain::Integer { minimum: Some(0), maximum: None }));
        assert_eq!(implicit(64, true), None);
        assert_eq!(implicit(32, true), Some(Domain::Integer { minimum: Some(-2_147_483_648), maximum: Some(2_147_483_647) }));
    }

    #[test]
    fn merging_keeps_the_tighter_bound_and_refuses_two_patterns() {
        let merged = merge(Some(json!({ "minimum": 0, "maximum": 4_294_967_295_u64 })), Some(json!({ "minimum": 1 }))).expect("merges");
        assert_eq!(merged, Some(json!({ "minimum": 1, "maximum": 4_294_967_295_u64 })));
        merge(Some(json!({ "pattern": "^a$" })), Some(json!({ "pattern": "^b$" }))).unwrap_err();
        assert_eq!(merge(None, Some(json!({ "minLength": 1 }))), Ok(Some(json!({ "minLength": 1 }))));
        assert_eq!(merge(None, None), Ok(None));
    }
}
