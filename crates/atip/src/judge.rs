//! Judging a value against a domain without a JSON Schema validator.
//!
//! A domain is written in the contract's fixed profile of JSON Schema (spec 10.7.4), so [`Node`] reads exactly that
//! profile and nothing else: `type`, `enum`, `const`, the numeric keywords (`minimum`, `maximum`, `exclusiveMinimum`,
//! `exclusiveMaximum`, `multipleOf`), the string keywords (`minLength`, `maxLength`, `pattern`) and the structural
//! ones of a `json` argument (`properties`, `required`, `additionalProperties`, `items`), with the annotations
//! (`title`, `description`, `$comment`, `format`, `default`, ...) ignored. It answers as the JSON Schema 2020-12
//! validator the `validate` feature links does: a keyword applies to the kind of value it is about and to no other, a
//! number is compared exactly, `1` and `1.0` are one value, a length counts characters, and a `pattern` is a search
//! (the profile writes `^...$`). `domain::admits` is built on it, so a tool that refuses an argument outside its domain
//! links no validator. A keyword of JSON Schema that is outside the profile is [`Refused::Unsupported`]: it is judged
//! only with the feature `validate`.

use std::cmp::Ordering;

use regex::Regex;
use serde_json::{Map, Number, Value};

/// Why a domain is not judged here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Refused {
    /// A keyword of JSON Schema that the profile does not have.
    Unsupported(String),
    /// A schema that is not one: a keyword with a value of the wrong kind, a `pattern` that does not compile.
    Invalid(String),
}

/// Keywords the member of a schema may carry that say nothing about a value.
const ANNOTATIONS: [&str; 7] = ["title", "description", "$comment", "format", "default", "examples", "deprecated"];
/// `readOnly` and `writeOnly` annotate too; a separate list keeps each line short.
const MORE_ANNOTATIONS: [&str; 2] = ["readOnly", "writeOnly"];
/// Keywords of JSON Schema that this module does not judge (the profile has none of them).
const BEYOND_THE_PROFILE: [&str; 36] = [
    "$ref",
    "$dynamicRef",
    "$defs",
    "$id",
    "$schema",
    "$anchor",
    "$dynamicAnchor",
    "$vocabulary",
    "allOf",
    "anyOf",
    "oneOf",
    "not",
    "if",
    "then",
    "else",
    "dependentRequired",
    "dependentSchemas",
    "dependencies",
    "definitions",
    "patternProperties",
    "propertyNames",
    "prefixItems",
    "additionalItems",
    "contains",
    "minContains",
    "maxContains",
    "minItems",
    "maxItems",
    "uniqueItems",
    "minProperties",
    "maxProperties",
    "unevaluatedItems",
    "unevaluatedProperties",
    "contentEncoding",
    "contentMediaType",
    "contentSchema",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Null,
    Boolean,
    Object,
    Array,
    Number,
    Integer,
    String,
}

impl Kind {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "null" => Kind::Null,
            "boolean" => Kind::Boolean,
            "object" => Kind::Object,
            "array" => Kind::Array,
            "number" => Kind::Number,
            "integer" => Kind::Integer,
            "string" => Kind::String,
            _ => return None,
        })
    }

    fn holds(self, value: &Value) -> bool {
        match (self, value) {
            (Kind::Null, Value::Null) | (Kind::Boolean, Value::Bool(_)) | (Kind::Object, Value::Object(_)) | (Kind::Array, Value::Array(_)) | (Kind::Number, Value::Number(_)) | (Kind::String, Value::String(_)) => true,
            (Kind::Integer, Value::Number(number)) => is_integer(number),
            _ => false,
        }
    }
}

#[derive(Debug)]
pub(crate) enum Rule {
    Type(Vec<Kind>),
    Enum(Vec<Value>),
    Const(Value),
    Minimum(Number),
    Maximum(Number),
    ExclusiveMinimum(Number),
    ExclusiveMaximum(Number),
    MultipleOf(Number),
    MinLength(u64),
    MaxLength(u64),
    Pattern(Regex),
    Required(Vec<String>),
    Properties(Vec<(String, Node)>),
    /// The schema of the members `properties` does not declare, with the names it does declare.
    AdditionalProperties(Box<Node>, Vec<String>),
    Items(Box<Node>),
}

/// A domain read once, to be asked of many values.
#[derive(Debug)]
pub(crate) enum Node {
    Bool(bool),
    Rules(Vec<Rule>),
}

impl Node {
    /// Reads `schema`, or says why it is not judged here.
    pub(crate) fn compile(schema: &Value) -> Result<Node, Refused> {
        let map = match schema {
            Value::Bool(flag) => return Ok(Node::Bool(*flag)),
            Value::Object(map) => map,
            other => return Err(Refused::Invalid(format!("a schema is an object or a boolean, not {other}"))),
        };
        let mut rules = Vec::new();
        for (keyword, value) in map {
            if ANNOTATIONS.contains(&keyword.as_str()) || MORE_ANNOTATIONS.contains(&keyword.as_str()) {
                continue;
            }
            if BEYOND_THE_PROFILE.contains(&keyword.as_str()) {
                return Err(Refused::Unsupported(keyword.clone()));
            }
            if let Some(rule) = rule(keyword, value, map)? {
                rules.push(rule);
            }
        }
        Ok(Node::Rules(rules))
    }

    /// Whether `value` is inside the domain.
    pub(crate) fn is_valid(&self, value: &Value) -> bool {
        match self {
            Node::Bool(flag) => *flag,
            Node::Rules(rules) => rules.iter().all(|rule| rule.holds(value)),
        }
    }
}

fn invalid<T>(keyword: &str, why: &str) -> Result<T, Refused> {
    Err(Refused::Invalid(format!("{keyword} {why}")))
}

fn rule(keyword: &str, value: &Value, siblings: &Map<String, Value>) -> Result<Option<Rule>, Refused> {
    Ok(Some(match keyword {
        "type" => {
            let names: Vec<&Value> = match value {
                Value::Array(items) => items.iter().collect(),
                single => vec![single],
            };
            let mut kinds = Vec::new();
            for name in names {
                match name.as_str().and_then(Kind::parse) {
                    Some(kind) => kinds.push(kind),
                    None => return invalid("type", &format!("names {name}, which is not a type")),
                }
            }
            Rule::Type(kinds)
        }
        "enum" => match value {
            Value::Array(items) => Rule::Enum(items.clone()),
            _ => return invalid("enum", "is an array"),
        },
        "const" => Rule::Const(value.clone()),
        "minimum" | "maximum" | "exclusiveMinimum" | "exclusiveMaximum" => {
            let Value::Number(bound) = value else { return invalid(keyword, "is a number") };
            let bound = bound.clone();
            match keyword {
                "minimum" => Rule::Minimum(bound),
                "maximum" => Rule::Maximum(bound),
                "exclusiveMinimum" => Rule::ExclusiveMinimum(bound),
                _ => Rule::ExclusiveMaximum(bound),
            }
        }
        "multipleOf" => match value {
            Value::Number(multiple) if multiple.as_f64().is_some_and(|multiple| multiple > 0.0) => Rule::MultipleOf(multiple.clone()),
            _ => return invalid("multipleOf", "is a number above zero"),
        },
        "minLength" | "maxLength" => {
            let Some(length) = length_of(value) else { return invalid(keyword, "is a non-negative integer") };
            if keyword == "minLength" {
                Rule::MinLength(length)
            } else {
                Rule::MaxLength(length)
            }
        }
        "pattern" => match value {
            Value::String(pattern) => match Regex::new(pattern) {
                Ok(compiled) => Rule::Pattern(compiled),
                Err(error) => return invalid("pattern", &format!("does not compile: {error}")),
            },
            _ => return invalid("pattern", "is a string"),
        },
        "required" => match value {
            Value::Array(items) if items.iter().all(Value::is_string) => Rule::Required(items.iter().filter_map(Value::as_str).map(str::to_string).collect()),
            _ => return invalid("required", "is an array of strings"),
        },
        "properties" => {
            let Value::Object(members) = value else { return invalid("properties", "is an object") };
            let mut compiled = Vec::new();
            for (name, schema) in members {
                compiled.push((name.clone(), Node::compile(schema)?));
            }
            Rule::Properties(compiled)
        }
        "additionalProperties" => {
            let declared = siblings.get("properties").and_then(Value::as_object).map(|members| members.keys().cloned().collect()).unwrap_or_default();
            Rule::AdditionalProperties(Box::new(Node::compile(value)?), declared)
        }
        "items" => match value {
            Value::Array(_) => return invalid("items", "is a schema, not a list of them"),
            schema => Rule::Items(Box::new(Node::compile(schema)?)),
        },
        // A keyword JSON Schema does not define says nothing, as in the validator.
        _ => return Ok(None),
    }))
}

/// A non-negative integer written as a JSON number (`3` and `3.0`).
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "the float is checked to be a whole number between zero and 2^64 first")]
fn length_of(value: &Value) -> Option<u64> {
    let Value::Number(number) = value else { return None };
    if let Some(unsigned) = number.as_u64() {
        return Some(unsigned);
    }
    let float = number.as_f64()?;
    (float >= 0.0 && float.fract() == 0.0 && float < 1.8e19).then_some(float as u64)
}

impl Rule {
    fn holds(&self, value: &Value) -> bool {
        match (self, value) {
            (Rule::Type(kinds), value) => kinds.iter().any(|kind| kind.holds(value)),
            (Rule::Enum(items), value) => items.iter().any(|item| equal(item, value)),
            (Rule::Const(expected), value) => equal(expected, value),
            (Rule::Minimum(bound), Value::Number(number)) => compare(number, bound) != Ordering::Less,
            (Rule::Maximum(bound), Value::Number(number)) => compare(number, bound) != Ordering::Greater,
            (Rule::ExclusiveMinimum(bound), Value::Number(number)) => compare(number, bound) == Ordering::Greater,
            (Rule::ExclusiveMaximum(bound), Value::Number(number)) => compare(number, bound) == Ordering::Less,
            (Rule::MultipleOf(multiple), Value::Number(number)) => is_multiple_of(number, multiple),
            (Rule::MinLength(least), Value::String(text)) => u64::try_from(text.chars().count()).unwrap_or(u64::MAX) >= *least,
            (Rule::MaxLength(most), Value::String(text)) => u64::try_from(text.chars().count()).unwrap_or(u64::MAX) <= *most,
            (Rule::Pattern(pattern), Value::String(text)) => pattern.is_match(text),
            (Rule::Required(names), Value::Object(members)) => names.iter().all(|name| members.contains_key(name)),
            (Rule::Properties(declared), Value::Object(members)) => declared.iter().all(|(name, node)| members.get(name).is_none_or(|member| node.is_valid(member))),
            (Rule::AdditionalProperties(node, declared), Value::Object(members)) => members.iter().all(|(name, member)| declared.contains(name) || node.is_valid(member)),
            (Rule::Items(node), Value::Array(items)) => items.iter().all(|item| node.is_valid(item)),
            // A keyword is about one kind of value and says nothing of the others.
            _ => true,
        }
    }
}

fn is_integer(number: &Number) -> bool {
    number.is_i64() || number.is_u64() || number.as_f64().is_some_and(|float| float.fract() == 0.0)
}

/// `a` and `b` as the JSON data model equates them: numbers by value (`1` is `1.0`), objects by members, arrays by order.
fn equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => compare(x, y) == Ordering::Equal,
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(x, y)| equal(x, y)),
        (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().all(|(key, x)| y.get(key).is_some_and(|y| equal(x, y))),
        (x, y) => x == y,
    }
}

fn as_integer(number: &Number) -> Option<i128> {
    number.as_i64().map(i128::from).or_else(|| number.as_u64().map(i128::from))
}

/// The order of two numbers, exactly: two integers by value, a float and an integer without rounding the integer.
fn compare(a: &Number, b: &Number) -> Ordering {
    match (as_integer(a), as_integer(b)) {
        (Some(x), Some(y)) => x.cmp(&y),
        (Some(x), None) => compare_integer_to_float(x, b.as_f64().unwrap_or(f64::NAN)),
        (None, Some(y)) => compare_integer_to_float(y, a.as_f64().unwrap_or(f64::NAN)).reverse(),
        (None, None) => a.as_f64().unwrap_or(f64::NAN).partial_cmp(&b.as_f64().unwrap_or(f64::NAN)).unwrap_or(Ordering::Equal),
    }
}

/// `integer` against `float`, as the integer's order to the float.
#[expect(clippy::cast_possible_truncation, reason = "the float is checked to lie within the range of an i128 first, and only its whole part is kept")]
fn compare_integer_to_float(integer: i128, float: f64) -> Ordering {
    if float.is_nan() {
        return Ordering::Equal;
    }
    // Beyond the range of an i128 the float is larger in magnitude than any integer a JSON number holds here.
    if float >= 1.7e38 {
        return Ordering::Less;
    }
    if float <= -1.7e38 {
        return Ordering::Greater;
    }
    let whole = float.trunc();
    match integer.cmp(&(whole as i128)) {
        Ordering::Equal if float > whole => Ordering::Less,
        Ordering::Equal if float < whole => Ordering::Greater,
        order => order,
    }
}

/// `value` is a multiple of `multiple` (above zero): exactly, so `0.3` is no multiple of `0.1` and `1e22` is one of `1e10`.
fn is_multiple_of(value: &Number, multiple: &Number) -> bool {
    let Some(multiple_float) = multiple.as_f64() else { return false };
    if multiple_float.fract() == 0.0 {
        // An integer multiple: integers by remainder, a float that is whole by the float remainder.
        if let (Some(whole), Some(step)) = (as_integer(value), as_integer(multiple)) {
            return step > 0 && whole % step == 0;
        }
        let Some(value_float) = value.as_f64() else { return false };
        return value_float.fract() == 0.0 && value_float % multiple_float == 0.0;
    }
    let Some(value_float) = value.as_f64() else { return false };
    if value_float == 0.0 {
        return true;
    }
    if value_float.abs() < multiple_float {
        return false;
    }
    // The numbers are read as the decimals they were written as (`0.3` is three tenths, a multiple of `0.1`), as the
    // validator reads them. With value = a * 10^p and multiple = b * 10^q, value / multiple is a * 10^(p-q) / b.
    let (value_digits, value_exponent) = decimal(value_float.abs());
    let (step_digits, step_exponent) = decimal(multiple_float);
    let divisor = gcd(value_digits, step_digits);
    let (numerator, denominator) = (value_digits / divisor, step_digits / divisor);
    let shift = value_exponent - step_exponent;
    if shift >= 0 {
        // In lowest terms the quotient is whole when the denominator divides 10^shift: it is 2^x * 5^y with x, y <= shift.
        let (twos, rest) = split_prime(denominator, 2);
        let (fives, rest) = split_prime(rest, 5);
        return rest == 1 && twos <= shift && fives <= shift;
    }
    // Otherwise the numerator must be a whole multiple of the denominator (1, in lowest terms) and of 10^-shift.
    denominator == 1 && split_prime(numerator, 2).0 >= -shift && split_prime(numerator, 5).0 >= -shift
}

/// `x` (finite, above zero) as `digits * 10^exponent`, from its shortest decimal form.
fn decimal(x: f64) -> (u64, i32) {
    let text = format!("{x:e}");
    let (mantissa, exponent) = text.split_once('e').unwrap_or((&text, "0"));
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits: u64 = format!("{whole}{fraction}").parse().unwrap_or(1);
    let exponent: i32 = exponent.parse().unwrap_or_default();
    (digits, exponent - i32::try_from(fraction.len()).unwrap_or_default())
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 {
        a.max(1)
    } else {
        gcd(b, a % b)
    }
}

/// `n` (above zero) as `prime^count * rest`: `(count, rest)`.
fn split_prime(mut n: u64, prime: u64) -> (i32, u64) {
    let mut count = 0;
    while n > 0 && n.is_multiple_of(prime) {
        n /= prime;
        count += 1;
    }
    (count, n)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn native(schema: &Value, instance: &Value) -> bool {
        Node::compile(schema).unwrap_or_else(|why| panic!("{schema}: {why:?}")).is_valid(instance)
    }

    fn schemas() -> Vec<Value> {
        vec![
            json!({ "minimum": 1 }),
            json!({ "maximum": 10 }),
            json!({ "minimum": 1, "maximum": 9_007_199_254_740_991_i64 }),
            json!({ "minimum": -9_007_199_254_740_991_i64 }),
            json!({ "exclusiveMinimum": 0 }),
            json!({ "exclusiveMaximum": 5 }),
            json!({ "minimum": 0.5 }),
            json!({ "maximum": 2.5, "exclusiveMinimum": 0.1 }),
            json!({ "multipleOf": 3 }),
            json!({ "multipleOf": 7.0 }),
            json!({ "multipleOf": 0.1 }),
            json!({ "multipleOf": 0.25 }),
            json!({ "multipleOf": 0.001 }),
            json!({ "multipleOf": 1e10 }),
            json!({ "minLength": 2 }),
            json!({ "maxLength": 3 }),
            json!({ "minLength": 1, "maxLength": 4 }),
            json!({ "minLength": 2.0 }),
            json!({ "pattern": "^[a-z]+$" }),
            json!({ "pattern": "^(a|b)c$" }),
            json!({ "pattern": "^[\\p{L}]+$" }),
            json!({ "pattern": "^[0-9]{2,4}$" }),
            json!({ "pattern": "^abc$" }),
            json!({ "pattern": "^ab" }),
            json!({ "pattern": "b" }),
            json!({ "pattern": "^[^\\n]*$" }),
            json!({ "type": "integer" }),
            json!({ "type": "number" }),
            json!({ "type": ["string", "null"] }),
            json!({ "type": "object", "required": ["a"], "properties": { "a": { "type": "integer", "minimum": 1 } }, "additionalProperties": false }),
            json!({ "properties": { "a": { "type": "string" } }, "additionalProperties": { "type": "integer" } }),
            json!({ "additionalProperties": { "type": "integer" } }),
            json!({ "additionalProperties": false }),
            json!({ "required": ["a", "b"] }),
            json!({ "type": "array", "items": { "type": "string", "minLength": 1 } }),
            json!({ "items": false }),
            json!({ "enum": [1, "a", null, { "x": [1, 2] }] }),
            json!({ "enum": [] }),
            json!({ "const": 1 }),
            json!({ "const": { "a": 1.0 } }),
            json!({ "const": [1, "a"] }),
            json!(true),
            json!(false),
            json!({}),
            json!({ "format": "date", "title": "t", "description": "d", "$comment": "c", "default": 1, "examples": [1], "deprecated": true, "readOnly": true, "writeOnly": false }),
            json!({ "x-tool": { "anything": 1 }, "minimum": 2 }),
            json!({ "type": "string", "minLength": 1, "pattern": "^[a-z]+$", "maxLength": 5 }),
        ]
    }

    fn instances() -> Vec<Value> {
        let mut all: Vec<Value> = vec![json!(null), json!(true), json!(false), json!([]), json!([1]), json!(["a"]), json!([""]), json!(["a", ""]), json!({}), json!({ "a": 1 }), json!({ "a": 0 }), json!({ "a": "x" }), json!({ "b": 1 }), json!({ "a": 1, "b": "x" }), json!({ "a": 1, "b": 2 }), json!({ "a": 1.0 }), json!({ "x": [1, 2] }), json!({ "x": [1.0, 2] })];
        for number in [json!(0), json!(1), json!(2), json!(3), json!(5), json!(9), json!(10), json!(11), json!(-1), json!(21), json!(14), json!(7.0), json!(6.0), json!(1.0), json!(3.0), json!(0.1), json!(0.2), json!(0.3), json!(0.25), json!(0.5), json!(0.75), json!(2.5), json!(0.001), json!(0.003), json!(1e10), json!(3e10), json!(1e22), json!(9_007_199_254_740_991_i64), json!(9_007_199_254_740_992_u64), json!(-9_007_199_254_740_991_i64), json!(18_446_744_073_709_551_615_u64), json!(1e300)] {
            all.push(number);
        }
        for text in ["", "a", "ab", "abc", "abcd", "abcde", "ABC", "12", "123", "12345", "é", "日本語", "😀😀", "a\n", "ac", "bc", "abd", "b", "x b y", "x\ny", "\u{663}\u{664}"] {
            all.push(json!(text));
        }
        all
    }

    /// The profile answers as the JSON Schema validator does, for every schema of the profile and every instance.
    #[test]
    fn every_keyword_of_the_profile_answers_as_the_validator_does() {
        let mut checked = 0;
        for schema in schemas() {
            let validator = jsonschema::validator_for(&schema).unwrap_or_else(|error| panic!("{schema}: {error}"));
            let node = Node::compile(&schema).unwrap_or_else(|why| panic!("{schema}: {why:?}"));
            for instance in instances() {
                assert_eq!(node.is_valid(&instance), validator.is_valid(&instance), "{schema} against {instance}");
                checked += 1;
            }
        }
        assert!(checked > 1000, "{checked} pairs");
    }

    #[test]
    fn a_keyword_applies_to_its_own_kind_of_value_only() {
        assert!(native(&json!({ "minimum": 5 }), &json!("a")), "a number keyword says nothing of a string");
        assert!(native(&json!({ "minLength": 5 }), &json!(1)));
        assert!(native(&json!({ "required": ["a"] }), &json!([])));
        assert!(native(&json!({ "items": false }), &json!({})));
        assert!(!native(&json!({ "type": "integer", "minimum": 5 }), &json!(4)));
        assert!(native(&json!({ "type": "integer" }), &json!(2.0)), "2.0 is an integer");
        assert!(!native(&json!({ "type": "integer" }), &json!(2.5)));
        assert!(native(&json!({ "const": 1 }), &json!(1.0)) && native(&json!({ "enum": [1] }), &json!(1.0)), "1 and 1.0 are one value");
        assert!(native(&json!({ "minLength": 2, "maxLength": 2 }), &json!("😀😀")), "a length counts characters, not bytes");
        assert!(native(&json!({ "pattern": "b" }), &json!("abc")), "a pattern searches; the profile anchors it");
        assert!(!native(&json!({ "pattern": "^b$" }), &json!("abc")));
        assert!(!native(&json!({ "pattern": "^abc$" }), &json!("abc\n")), "`$` is the end of the text");
    }

    #[test]
    fn a_number_is_compared_without_rounding() {
        let beyond = json!(9_007_199_254_740_993_u64);
        assert!(!native(&json!({ "maximum": 9_007_199_254_740_992_u64 }), &beyond));
        assert!(native(&json!({ "minimum": 9_007_199_254_740_992_u64 }), &beyond));
        assert!(native(&json!({ "multipleOf": 0.1 }), &json!(0.3)), "0.3 is three tenths: a multiple of 0.1, as the validator reads decimals");
        assert!(!native(&json!({ "multipleOf": 0.1 }), &json!(0.35)));
        assert!(native(&json!({ "multipleOf": 0.001 }), &json!(12.345)));
        assert!(!native(&json!({ "multipleOf": 0.01 }), &json!(12.345)));
        assert!(native(&json!({ "multipleOf": 0.25 }), &json!(0.75)));
        assert!(native(&json!({ "multipleOf": 1e10 }), &json!(1e22)));
    }

    #[test]
    fn a_schema_that_is_not_one_is_invalid_and_a_keyword_outside_the_profile_is_unsupported() {
        for schema in [json!({ "minimum": "x" }), json!({ "multipleOf": 0 }), json!({ "multipleOf": -1 }), json!({ "minLength": -1 }), json!({ "minLength": 1.5 }), json!({ "pattern": "(" }), json!({ "pattern": 1 }), json!({ "type": "foo" }), json!({ "type": 1 }), json!({ "required": "a" }), json!({ "required": [1] }), json!({ "enum": 1 }), json!({ "properties": [] }), json!({ "properties": { "a": 1 } }), json!({ "items": [] }), json!(1), json!("x"), json!(null)] {
            assert!(matches!(Node::compile(&schema), Err(Refused::Invalid(_))), "{schema}");
            assert!(jsonschema::validator_for(&schema).is_err(), "{schema}: the validator refuses it too");
        }
        for keyword in ["anyOf", "allOf", "oneOf", "not", "$ref", "patternProperties", "propertyNames", "minItems", "maxItems", "uniqueItems", "minProperties", "contains", "if", "dependentRequired", "prefixItems", "unevaluatedProperties"] {
            let schema = json!({ keyword: [] });
            assert_eq!(Node::compile(&schema).err(), Some(Refused::Unsupported(keyword.to_string())), "{keyword}");
        }
    }
}
