//! `canon-0-35-crate-validate`: `atip::domain::admits` judges a domain without a JSON Schema validator and answers as the
//! validator does (`atip::domain::compile`, which needs the feature `validate`, on in this test build) for every domain
//! canon declares, with values inside and outside its bounds.

#[path = "support/hermetic.rs"]
mod hermetic;
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
            for _ in 0..400 {
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

/// A domain as a key: its whole value in one canonical text (members sorted by name), so two domains are one only when every keyword is equal
/// (the same pattern with another `maxLength` is another domain).
fn key(domain: &Value) -> String {
    match domain {
        Value::Object(members) => {
            let mut names: Vec<&String> = members.keys().collect();
            names.sort();
            format!("{{{}}}", names.iter().map(|name| format!("{}:{}", json!(name), key(&members[*name]))).collect::<Vec<_>>().join(","))
        }
        Value::Array(items) => format!("[{}]", items.iter().map(key).collect::<Vec<_>>().join(",")),
        other => other.to_string(),
    }
}

/// Every argument canon declares a domain for: the command, the argument, its domain and its default values.
struct Declared {
    label: String,
    domain: Value,
    defaults: Vec<Value>,
}

fn declared() -> Vec<Declared> {
    let mut all = Vec::new();
    for doc in leaves() {
        for argument in options(doc).iter().chain(doc["operands"].as_array().into_iter().flatten()) {
            let Some(domain) = argument.get("domain") else { continue };
            let defaults = match &argument["default"] {
                Value::Null => Vec::new(),
                Value::Array(items) => items.clone(),
                one => vec![one.clone()],
            };
            all.push(Declared { label: format!("`{}` {}", doc["command"], argument["name"]), domain: domain.clone(), defaults });
        }
    }
    all
}

/// The distinct domains of `declared`, each with the first argument that declares it, in the order they first appear.
fn distinct(declared: &[Declared]) -> Vec<(String, &Declared)> {
    let mut seen = std::collections::BTreeSet::new();
    declared.iter().filter(|one| seen.insert(key(&one.domain))).map(|one| (key(&one.domain), one)).collect()
}

#[test]
fn admits_answers_as_the_validator_does_for_every_domain_canon_declares() {
    let mut seed = Seed(0x2545_F491_4F6C_DD1D);
    let declared = declared();
    let patterns = declared.iter().filter(|one| one.domain.get("pattern").is_some()).count();
    let mut compiled = std::collections::BTreeMap::new();
    let mut asked: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut pairs = 0;
    // Every distinct domain is asked once, the values it is always asked (its bounds, lengths, samples) and the random strings of one seed.
    for (domain_key, one) in distinct(&declared) {
        let domain = &one.domain;
        let validator = atip::domain::compile(domain).unwrap_or_else(|why| panic!("{}: {why}", one.label));
        for value in candidates(domain, &mut seed) {
            let native = atip::domain::admits(domain, &value).unwrap_or_else(|why| panic!("{}: {why}", one.label));
            assert_eq!(native, validator.admits(&value), "{}: {domain} against {value}", one.label);
            pairs += 1;
        }
        asked.insert(domain_key.clone());
        compiled.insert(domain_key, validator);
    }
    // The default of every argument is asked of its own domain.
    for one in &declared {
        let validator = &compiled[&key(&one.domain)];
        for value in &one.defaults {
            let native = atip::domain::admits(&one.domain, value).unwrap_or_else(|why| panic!("{}: {why}", one.label));
            assert_eq!(native, validator.admits(value), "{}: {} against {value}", one.label, one.domain);
            pairs += 1;
        }
    }
    // The distinct domains, found again another way (a list searched by equality), are the ones asked; every argument's domain is among them.
    let mut again: Vec<&Value> = Vec::new();
    for one in &declared {
        if !again.contains(&&one.domain) {
            again.push(&one.domain);
        }
    }
    assert_eq!(again.len(), asked.len(), "the distinct domains asked");
    assert!(again.iter().all(|domain| asked.contains(&key(domain))), "a domain was not asked");
    assert!(declared.iter().all(|one| asked.contains(&key(&one.domain))), "an argument's domain was not asked");
    println!("{} arguments declare a domain, {patterns} of them a pattern; {} distinct domains; {pairs} values asked", declared.len(), asked.len());
    assert!(declared.len() > 40 && patterns > 10, "canon declares {} domains, {patterns} of them patterns", declared.len());
    assert!(pairs > 10_000, "{pairs} values asked");
}

/// Two arguments with the same pattern and another length bound are two domains; the same domain written with its members in another order is one.
#[test]
fn a_domain_is_keyed_by_its_whole_value() {
    let one = |domain: Value| Declared { label: String::new(), domain, defaults: Vec::new() };
    let all = [one(json!({ "type": "string", "pattern": "^a$", "maxLength": 10 })), one(json!({ "type": "string", "pattern": "^a$", "maxLength": 20 })), one(json!({ "maxLength": 10, "pattern": "^a$", "type": "string" }))];
    assert_eq!(distinct(&all).len(), 2);
    assert_ne!(key(&all[0].domain), key(&all[1].domain));
    assert_eq!(key(&all[0].domain), key(&all[2].domain));
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
