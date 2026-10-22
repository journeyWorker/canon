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
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("stdout is JSON");
    assert_eq!(v["meta"]["command"], "schema", "{v}");
    v["data"].clone()
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
    assert_eq!(v["supportsJson"], v["options"].as_array().unwrap().iter().any(|o| o["name"] == "json"));
    let codes: Vec<&str> = v["errorCodes"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
    for code in ["EVIDENCE_FABRICATED", "BRIEF_UNAPPROVED", "BRIEF_CHANGED_SINCE_APPROVAL", "EVIDENCE_INVALID_ARGUMENT", "USAGE"] {
        assert!(codes.contains(&code), "{code} missing from {codes:?}");
    }
    assert!(!codes.contains(&"EVIDENCE_VAULT_FAILED"), "a sibling command's code leaked in: {codes:?}");
    let exits: Vec<u64> = v["exits"].as_array().unwrap().iter().map(|e| e["code"].as_u64().unwrap()).collect();
    assert_eq!(exits, [0, 1, 2]);
}

#[test]
fn exits_follow_each_commands_own_contract() {
    let exits = |cmd: &[&str]| {
        let mut args = vec!["schema"];
        args.extend_from_slice(cmd);
        args.push("--json");
        json(&args)["exits"].as_array().unwrap().iter().map(|e| e["code"].as_u64().unwrap()).collect::<Vec<_>>()
    };
    assert_eq!(exits(&["context"]), [0]);
    assert_eq!(exits(&["dispatch", "diff"]), [0, 2]);
    assert_eq!(exits(&["finding", "list"]), [0, 2]);
    assert_eq!(exits(&["evidence", "vault"]), [0, 2]);
    assert_eq!(exits(&["gate", "check"]), [0, 1, 2]);
    assert_eq!(exits(&["brief", "check"]), [0, 1, 2]);
    assert_eq!(exits(&["finding", "add"]), [0, 2]);
}

#[test]
fn errors_lists_the_whole_registry() {
    let v = json(&["schema", "--errors", "--json"]);
    let listed: Vec<&str> = v["errors"].as_array().unwrap().iter().map(|e| e["code"].as_str().unwrap()).collect();
    let registry: Vec<&str> = canon_cli::cli_error::all_codes().iter().map(|(c, _)| *c).collect();
    assert_eq!(listed, registry);
    assert!(v["errors"].as_array().unwrap().iter().all(|e| !e["meaning"].as_str().unwrap().is_empty()));
    let human = canon(&["schema", "--errors"]);
    assert!(human.status.success() && String::from_utf8_lossy(&human.stdout).contains("BRIEF_CHANGED_SINCE_APPROVAL"));
    assert_eq!(canon(&["schema", "--errors", "evidence", "add"]).status.code(), Some(2));
}

#[test]
fn enum_options_list_their_values_and_defaults() {
    let v = json(&["schema", "init", "--json"]);
    let policy = v["options"].as_array().unwrap().iter().find(|o| o["name"] == "policy").unwrap().clone();
    assert!(policy["possibleValues"].as_array().unwrap().iter().any(|p| p == "strict"));
    let repo = v["options"].as_array().unwrap().iter().find(|o| o["name"] == "repo").unwrap().clone();
    assert_eq!(repo["default"][0], "nearest ancestor directory with a canon.yaml", "`.` is resolved to the nearest canon.yaml, so the schema must not report it as the default");
}

#[test]
fn an_alias_resolves_to_the_canonical_command() {
    let v = json(&["schema", "fmt", "--json"]);
    assert_eq!(v["command"], "format");
    assert_eq!(v["aliases"][0], "fmt");
}

/// Dogfood eval 8: an agent quoted the path as one word.
#[test]
fn a_quoted_command_path_names_the_same_command() {
    assert_eq!(json(&["schema", "evidence add", "--json"]), json(&["schema", "evidence", "add", "--json"]));
}

#[test]
fn an_unknown_path_is_a_refusal_naming_the_nearest_matches() {
    let out = canon(&["schema", "evidence", "ad"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    assert!(String::from_utf8_lossy(&out.stderr).contains("evidence add"));
}
