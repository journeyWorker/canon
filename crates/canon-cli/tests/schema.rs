//! `canon schema` (0.20 D1) through the built binary, run from a directory
//! that is not a canon repo.

use std::process::{Command, Output};

fn canon(args: &[&str]) -> Output {
    let dir = tempfile::tempdir().expect("tempdir");
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(dir.path()).output().expect("spawn canon binary")
}

fn json(args: &[&str]) -> serde_json::Value {
    let out = canon(args);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).expect("stdout is JSON")
}

#[test]
fn bare_schema_lists_every_leaf_with_an_effect_in_a_stable_order() {
    let first = json(&["schema", "--json"]);
    assert_eq!(first, json(&["schema", "--json"]));
    let commands = first["commands"].as_array().unwrap();
    let names: Vec<&str> = commands.iter().map(|c| c["command"].as_str().unwrap()).collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);
    assert!(names.contains(&"evidence add") && names.contains(&"schema"));
    let purge = commands.iter().find(|c| c["command"] == "purge").unwrap();
    assert_eq!(purge["effect"], "destructive");
}

#[test]
fn one_command_shows_options_with_their_shape() {
    let v = json(&["schema", "evidence", "add", "--json"]);
    assert_eq!(v["command"], "evidence add");
    let opt = |name: &str| v["options"].as_array().unwrap().iter().find(|o| o["name"] == name).unwrap_or_else(|| panic!("no --{name}")).clone();
    assert_eq!(opt("all-scenarios")["kind"], "flag");
    assert_eq!(opt("report")["repeatable"], true);
    assert_eq!(v["effect"], "write");
    assert_eq!(v["supports_json"], v["options"].as_array().unwrap().iter().any(|o| o["name"] == "json"));
    assert!(v["error_codes"].as_array().unwrap().is_empty());
    assert!(v["exits"].as_array().unwrap().iter().any(|e| e["code"] == 2));
}

#[test]
fn enum_options_list_their_values_and_defaults() {
    let v = json(&["schema", "init", "--json"]);
    let policy = v["options"].as_array().unwrap().iter().find(|o| o["name"] == "policy").unwrap().clone();
    assert!(policy["possible_values"].as_array().unwrap().iter().any(|p| p == "strict"));
    let repo = v["options"].as_array().unwrap().iter().find(|o| o["name"] == "repo").unwrap().clone();
    assert_eq!(repo["default"][0], ".");
}

#[test]
fn an_alias_resolves_to_the_canonical_command() {
    let v = json(&["schema", "fmt", "--json"]);
    assert_eq!(v["command"], "format");
    assert_eq!(v["aliases"][0], "fmt");
}

#[test]
fn an_unknown_path_is_a_refusal_naming_the_nearest_matches() {
    let out = canon(&["schema", "evidence", "ad"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).contains("evidence add"));
}
