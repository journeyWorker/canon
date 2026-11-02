//! `canon-0-35-crate-validate`: `atip::domain::admits` judges a domain without a JSON Schema validator and answers as the
//! validator does (`atip::domain::compile`, which needs the feature `validate`, on in this test build) for every domain
//! canon declares, with values inside and outside its bounds.

mod harness;

use harness::{leaves, options};
use serde_json::{json, Value};

/// A deterministic sequence of numbers, so a failure repeats.
struct Seed(u64);

impl Seed {
    fn next(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        usize::try_from(self.0 % u64::try_from(bound).unwrap_or(1)).unwrap_or(0)
    }
}

/// Values to ask of `domain`: its bounds and their neighbours, lengths around its length bounds, and random strings over the
/// alphabets the patterns of canon's id kinds are written for.
fn candidates(domain: &Value, seed: &mut Seed) -> Vec<Value> {
    let mut values: Vec<Value> = Vec::new();
    for keyword in ["minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum"] {
        if let Some(bound) = domain.get(keyword).and_then(Value::as_i64) {
            for step in [-2, -1, 0, 1, 2] {
                values.push(json!(bound + step));
            }
        }
    }
    values.extend([json!(0), json!(1), json!(-1), json!(100), json!(4_294_967_295_u64), json!(4_294_967_296_u64), json!(9_007_199_254_740_991_i64), json!(-9_007_199_254_740_991_i64), json!(1.5), json!(2.0), json!(""), json!(null), json!(true), json!([]), json!({})]);
    let lengths: Vec<u64> = ["minLength", "maxLength"].iter().filter_map(|keyword| domain.get(*keyword).and_then(Value::as_u64)).collect();
    for length in lengths.iter().flat_map(|length| [length.saturating_sub(1), *length, length + 1]).chain([0, 1, 2, 8, 26, 64]) {
        values.push(json!("a".repeat(usize::try_from(length).unwrap_or(0))));
        values.push(json!("é".repeat(usize::try_from(length).unwrap_or(0))));
    }
    if domain.get("pattern").is_some() {
        let alphabets = ["abz019-- #.", "abz019-..#_", "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefilou", "ab1 -\t\n\u{a0}\u{2003}\u{7}\u{200b}", "a-z0-9._/:@+ \\"];
        for alphabet in alphabets {
            let alphabet: Vec<char> = alphabet.chars().collect();
            for _ in 0..120 {
                let length = seed.next(30);
                values.push(json!((0..length).map(|_| alphabet[seed.next(alphabet.len())]).collect::<String>()));
            }
        }
        for sample in ["kebab-case", "Kebab", "a", "atip.crate12.01", "atip.contract11.16", "add-login#1", "01HZX5V0R2S4K9J3M8N6P7Q1WE", "01HZX5V0R2S4K9J3M8N6P7Q1W", "main", "HEAD", "a b", "a\nb", "-x", "x-", "x--y"] {
            values.push(json!(sample));
        }
    }
    values
}

#[test]
fn admits_answers_as_the_validator_does_for_every_domain_canon_declares() {
    let mut seed = Seed(0x2545_F491_4F6C_DD1D);
    let (mut domains, mut pairs, mut patterns) = (0, 0, 0);
    for doc in leaves() {
        for argument in options(doc).iter().chain(doc["operands"].as_array().into_iter().flatten()) {
            let Some(domain) = argument.get("domain") else { continue };
            domains += 1;
            patterns += usize::from(domain.get("pattern").is_some());
            let compiled = atip::domain::compile(domain).unwrap_or_else(|why| panic!("`{}` {}: {why}", doc["command"], argument["name"]));
            let defaults: Vec<Value> = match &argument["default"] {
                Value::Null => Vec::new(),
                Value::Array(items) => items.clone(),
                one => vec![one.clone()],
            };
            for value in candidates(domain, &mut seed).into_iter().chain(defaults) {
                let native = atip::domain::admits(domain, &value).unwrap_or_else(|why| panic!("`{}` {}: {why}", doc["command"], argument["name"]));
                assert_eq!(native, compiled.admits(&value), "`{}` {}: {domain} against {value}", doc["command"], argument["name"]);
                pairs += 1;
            }
        }
    }
    assert!(domains > 40 && patterns > 10, "canon declares {domains} domains, {patterns} of them patterns");
    assert!(pairs > 5_000, "{pairs} values asked");
}

#[test]
fn a_domain_that_does_not_compile_is_an_error_in_both() {
    for domain in [json!({ "minimum": "x" }), json!({ "pattern": "(" }), json!({ "minLength": -1 }), json!({ "type": "nope" })] {
        assert!(atip::domain::admits(&domain, &json!(1)).is_err(), "{domain}");
        assert!(atip::domain::compile(&domain).is_err(), "{domain}");
    }
}

#[test]
fn a_keyword_outside_the_profile_is_judged_by_the_validator_when_the_feature_is_on() {
    let domain = json!({ "anyOf": [{ "minimum": 5 }, { "maximum": -5 }] });
    for (value, expected) in [(json!(0), false), (json!(5), true), (json!(-7), true)] {
        assert_eq!(atip::domain::admits(&domain, &value), Ok(expected), "{value}");
    }
}
