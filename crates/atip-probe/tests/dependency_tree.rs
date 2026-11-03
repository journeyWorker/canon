//! `canon-0-35-crate-validate`: `jsonschema` is linked only with the feature `validate`.
//!
//! The proof is `cargo tree`, which resolves features for the package it is asked about alone, so the workspace's own
//! `atip-check` and `canon-cli`, which turn `validate` on, do not change the answer for the probe or for `atip`.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use serde_json::json;

/// The crates `cargo tree` lists for `args` (names with versions, each once).
fn tree(args: &[&str]) -> BTreeSet<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(env!("CARGO"))
        .arg("tree")
        .args(args)
        .args(["--offline", "--locked", "--prefix", "none", "--color", "never"])
        .current_dir(root)
        .output()
        .expect("spawn cargo tree");
    assert!(output.status.success(), "cargo tree {args:?}: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).expect("UTF-8").lines().map(|line| line.split(" (").next().unwrap_or(line).trim().to_string()).filter(|line| !line.is_empty()).collect()
}

fn names(crates: &BTreeSet<String>) -> BTreeSet<&str> {
    crates.iter().filter_map(|entry| entry.split(' ').next()).collect()
}

#[test]
fn a_tool_that_depends_on_atip_alone_links_no_json_schema_validator() {
    let probe = tree(&["-p", "atip-probe"]);
    assert!(!names(&probe).contains("jsonschema"), "the probe depends on atip with its default features: {probe:?}");
    let atip = tree(&["-p", "atip", "-e", "normal"]);
    assert!(!names(&atip).contains("jsonschema"), "atip's default features: {atip:?}");
    assert!(names(&atip).contains("regex"), "domain patterns are judged with the regex crate");
}

#[test]
fn the_feature_validate_brings_jsonschema_in() {
    let with = tree(&["-p", "atip", "-e", "normal", "--features", "validate"]);
    assert!(names(&with).contains("jsonschema"), "{with:?}");
    let without = tree(&["-p", "atip", "-e", "normal"]);
    let added: Vec<&str> = names(&with).difference(&names(&without)).copied().collect();
    assert!(added.len() > 10, "the validator is a subtree of crates, not one: {added:?}");
    eprintln!("atip default features: {} crates; with validate: {} crates; the feature adds {}", without.len(), with.len(), added.len());
}

#[test]
fn the_checker_and_canon_turn_the_feature_on() {
    let checker = tree(&["-p", "atip-check", "-e", "normal"]);
    assert!(names(&checker).contains("jsonschema"), "atip-check validates documents");
    let canon = tree(&["-p", "canon-cli", "-e", "normal"]);
    assert!(names(&canon).contains("jsonschema"), "canon links it through canon-fmt and, for its debug validation, atip's validators");
}

#[test]
fn the_probe_does_what_a_tool_without_a_validator_does() {
    assert_eq!(atip_probe::words(&atip_probe::next_command("acme", &["member", "tag", "it's"])), ["acme", "member", "tag", "it's"]);
    assert_eq!(atip_probe::reserved_exit("STOPPED_MIDWAY"), Some(1));
    assert_eq!(atip_probe::refuses(&json!({ "minimum": 1 }), &json!(0)), Ok(true));
    assert_eq!(atip_probe::refuses(&json!({ "minimum": 1 }), &json!(1)), Ok(false));
    assert_eq!(atip_probe::refuses(&json!({ "pattern": "^[a-z]+$" }), &json!("abc")), Ok(false));
    // A keyword outside the profile is judged only with the validator: an answer with it, an error naming the keyword without.
    match atip_probe::refuses(&json!({ "anyOf": [{ "minimum": 1 }] }), &json!(0)) {
        Ok(refused) => assert!(refused, "with the validator the answer is the validator's"),
        Err(why) => assert!(why.contains("anyOf") && why.contains("validate"), "{why}"),
    }
}
