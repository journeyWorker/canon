//! The two regular-expression engines of `L1.patterns` against the contract's record of them (approaches/checker.md 6, L1.patterns;
//! spec 10.7.4): `tests/fixtures/contract/regex-cases.json` is `scripts/regex-cases.json` of the contract tag v1.1.1 (byte-identical at v1.1.2), whose `ecmascript` column is
//! what Bun's `RegExp(pattern, "u")` answered and whose `rust` column is what `regex` 1.13.1 answered. The checker is a Rust program, so
//! its ECMAScript engine is `regress`; it is the evidence that `regress` reads a pattern as Bun does that every case answers as recorded.

use std::path::Path;

use serde_json::Value;

use atip_check::l1::regex::{
    compare, ecmascript_answer, form_problem, rust_answer, Answer, Comparison, ECMASCRIPT_UNICODE, RUST_UNICODE,
};

fn cases() -> Vec<Value> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/contract/regex-cases.json");
    let text = std::fs::read_to_string(path).expect("regex-cases.json");
    let document: Value = serde_json::from_str(&text).expect("JSON");
    document["cases"].as_array().expect("cases").clone()
}

fn name(case: &Value) -> String {
    format!("{} on {}", case["pattern"], case["value"])
}

#[test]
fn atip_checkl1_25_regex_the_ecmascript_engine_answers_every_recorded_case_as_the_record_says() {
    let cases = cases();
    assert!(cases.len() >= 60, "{}", cases.len());
    for case in &cases {
        let got = ecmascript_answer(case["pattern"].as_str().expect("pattern"), case["value"].as_str().expect("value")).recorded();
        assert_eq!(got, case["ecmascript"], "ECMAScript: {}", name(case));
    }
}

#[test]
fn atip_checkl1_25_regex_the_rust_engine_answers_every_recorded_case_as_the_record_says() {
    for case in &cases() {
        let got = rust_answer(case["pattern"].as_str().expect("pattern"), case["value"].as_str().expect("value")).recorded();
        assert_eq!(got, case["rust"], "Rust: {}", name(case));
    }
}

#[test]
fn atip_checkl1_25_regex_the_form_of_every_recorded_pattern_is_the_one_the_record_says() {
    for case in &cases() {
        let pattern = case["pattern"].as_str().expect("pattern");
        let of_form = form_problem(pattern).is_none();
        assert_eq!(Value::Bool(of_form), case["form"], "form of {}: {:?}", name(case), form_problem(pattern));
    }
}

#[test]
fn atip_checkl1_25_regex_a_pattern_of_the_form_that_both_engines_compile_agrees_unless_the_record_names_a_unicode_version_difference() {
    let mut differences = 0;
    for case in &cases() {
        let (pattern, value) = (case["pattern"].as_str().expect("pattern"), case["value"].as_str().expect("value"));
        if case["form"] != true {
            continue;
        }
        let verdict = compare(pattern, value);
        match (&verdict, case.get("difference")) {
            (Comparison::Agree(_), None) => {}
            (Comparison::Uncompilable { .. }, None) => {
                assert!(
                    case["ecmascript"] == "error" || case["rust"] == "error",
                    "{} is uncompilable in one engine, so the record says so",
                    name(case)
                );
            }
            (Comparison::UnicodeVersion { difference, .. }, Some(recorded)) => {
                differences += 1;
                assert_eq!(recorded["kind"], "unicode-version");
                assert_eq!(Value::String(difference.code_point.clone()), recorded["codePoint"], "{}", name(case));
                assert_eq!(Value::String(difference.ecmascript_category.to_string()), recorded["ecmascript"], "{}", name(case));
                assert_eq!(Value::String(difference.rust_category.to_string()), recorded["rust"], "{}", name(case));
                assert!(difference.describe().contains(ECMASCRIPT_UNICODE) && difference.describe().contains(RUST_UNICODE));
            }
            (other, recorded) => panic!("{}: {other:?} against the record's difference {recorded:?}", name(case)),
        }
    }
    assert!(differences >= 1, "the record has a Unicode-version difference and the checker names it");
}

#[test]
fn atip_checkl1_25_regex_a_pattern_outside_the_form_that_the_engines_read_differently_is_not_a_version_difference() {
    // `^[a&&b]+$` is not of the form; on "a" ECMAScript accepts and Rust rejects, and no code point or category explains it.
    assert!(form_problem("^[a&&b]+$").is_some());
    assert!(matches!(compare("^[a&&b]+$", "a"), Comparison::Differs { ecmascript: true, rust: false }));
    // A single ASCII character under a pattern with a category is not a version difference either: the categories do not differ.
    assert!(matches!(compare("^\\p{L}$", "a"), Comparison::Agree(true)));
    assert!(matches!(
        ecmascript_answer("^(", "a"),
        Answer::Refuses(_)
    ));
}
