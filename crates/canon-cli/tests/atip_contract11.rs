//! atip/1 1.1 adoption (canon 0.34, `canon-0-34-atip-1-1`), the parts that need the built binary:
//! `meta.contractVersion` on every envelope, the empty `received`, the `USAGE` hint when no argument can be
//! named, and the tool-wide options after a group word. Tests are named after the scenario they prove
//! (`atip.contract11.NN`).

use std::path::Path;
use std::process::{Command, Output};

use canon_cli::cli_error::{CliError, CliErrorExt as _};
use serde_json::Value;
use tempfile::TempDir;

fn canon(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(dir).env_remove("CANON_ROLE").env("CANON_ACTOR", "atip-contract11").output().expect("spawn canon binary")
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("stdout is one JSON document ({e}): {}\nstderr: {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)))
}

fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let out = canon(dir.path(), &["init", "--providers", "none"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    dir
}

#[test]
fn atip_contract11_01_every_envelope_declares_the_contract_and_its_version_and_a_registry_answer_names_its_kind() {
    let dir = repo();
    for args in [vec!["status", "--json"], vec!["nosuch", "--json"], vec!["finding", "nosuch", "--json"], vec!["--json"], vec!["schema", "--json"], vec!["schema", "status", "--json"], vec!["schema", "--errors", "--json"], vec!["schema", "--warnings", "--json"], vec!["gate", "check", "--json"]] {
        let document = json(&canon(dir.path(), &args));
        assert_eq!(document["meta"]["contract"], "atip/1", "{args:?}: {document}");
        assert_eq!(document["meta"]["contractVersion"], "1.1", "{args:?}: the same build says the same version everywhere: {document}");
        assert_eq!(atip::envelope_problems(&document), Vec::<String>::new(), "{args:?}");
    }
    for (flag, kind) in [("--errors", "errors"), ("--warnings", "warnings")] {
        let document = json(&canon(dir.path(), &["schema", flag, "--json"]));
        assert_eq!(document["data"]["kind"], kind);
        assert_eq!(document["meta"]["schemaVersion"], "1.1", "{document}");
        assert_eq!(document["meta"]["command"], "schema");
    }
    for (flag, code, kind) in [("--error", "USAGE", "error"), ("--warning", "OLD_LAYOUT_FOUND", "warning")] {
        let document = json(&canon(dir.path(), &["schema", flag, code, "--json"]));
        assert_eq!((document["data"]["kind"].as_str(), document["meta"]["schemaVersion"].as_str()), (Some(kind), Some("1.1")), "{document}");
    }
    // The output of another command keeps its own version: only the registry answers and `schema` itself are 1.1.
    assert_eq!(json(&canon(dir.path(), &["nosuch", "--json"]))["meta"]["schemaVersion"], "1.0");
}

#[test]
fn atip_contract11_08_an_empty_value_the_caller_sent_is_received_empty_and_a_value_never_sent_has_no_received() {
    let dir = repo();
    let empty = json(&canon(dir.path(), &["finding", "add", "--change", "", "--round", "1", "--seq", "1", "--severity", "note", "--actor", "a", "--summary", "s", "--json"]));
    assert_eq!(empty["error"]["code"], "USAGE", "{empty}");
    assert_eq!(empty["error"]["field"], "--change", "{empty}");
    assert_eq!(empty["error"]["received"], "", "{empty}");
    assert_eq!(atip::envelope_problems(&empty), Vec::<String>::new());
    let absent = json(&canon(dir.path(), &["finding", "add", "--json"]));
    assert_eq!(absent["error"]["code"], "USAGE", "{absent}");
    assert!(absent["error"].get("received").is_none(), "a value never sent has no received: {absent}");
    // An option with no value is a value never sent: clap reports it as an empty value, canon leaves `received` out.
    for args in [vec!["finding", "add", "--change", "--json"], vec!["finding", "add", "--json", "--change"]] {
        let missing = json(&canon(dir.path(), &args));
        assert_eq!((missing["error"]["code"].as_str(), missing["error"]["field"].as_str()), (Some("USAGE"), Some("--change")), "{args:?}: {missing}");
        assert!(missing["error"].get("received").is_none(), "{args:?}: a value never sent has no received: {missing}");
    }
    let equals = json(&canon(dir.path(), &["finding", "add", "--change=", "--json"]));
    assert_eq!(equals["error"]["received"], "", "an explicit empty value keeps received: {equals}");
    // The same through `--input`: an empty key of the document is reported as the caller sent it.
    let document = json(&canon(dir.path(), &["scenario", "new", "--input", r#"{"operands":["auth.login.01"],"title":"t","":1}"#, "--json"]));
    assert_eq!((document["error"]["code"].as_str(), document["error"]["reason"].as_str(), document["error"]["received"].as_str()), (Some("INPUT_INVALID"), Some("UNKNOWN_KEY"), Some("")), "{document}");
    // A secret is still hidden, and an empty value of a secret-named option stays empty.
    let secret = CliError::refused("USAGE", "m").field("--api-token").received("hunter2").body();
    assert_eq!(secret.received.as_deref(), Some("<redacted>"));
}

#[test]
fn atip_contract11_09_a_usage_that_cannot_name_its_argument_has_no_field_and_points_at_the_schema_of_the_command() {
    let unnamed = CliError::refused("USAGE", "the request breaks a rule no single argument owns");
    let under_command = unnamed.envelope("canon finding add");
    assert_eq!(under_command["error"]["hint"], "canon schema finding add --json", "{under_command}");
    assert!(under_command["error"].get("field").is_none(), "{under_command}");
    let group = unnamed.envelope("canon finding");
    assert_eq!(group["error"]["hint"], "canon schema finding --json");
    let unresolved = unnamed.envelope("canon");
    assert_eq!(unresolved["error"]["hint"], "canon schema --json", "{unresolved}");
    for document in [&under_command, &group, &unresolved] {
        assert_eq!(atip::envelope_problems(document), Vec::<String>::new());
    }
    // An error that names its argument keeps its own hint.
    let named = CliError::refused("USAGE", "bad").field("--x").hint("pass --x 1").envelope("canon finding add");
    assert_eq!(named["error"]["hint"], "pass --x 1");
    // In text mode the same hint is printed.
    let text = canon_cli::cli_error::with_usage_hint(&unnamed, "canon finding add").human_text();
    assert!(text.ends_with("hint: canon schema finding add --json"), "{text}");
}

#[test]
fn atip_contract11_10_a_retrieve_with_no_scope_hints_a_template_and_not_prose_that_starts_with_canon() {
    let dir = repo();
    let document = json(&canon(dir.path(), &["retrieve", "--for-role", "reviewer", "--json"]));
    assert_eq!(document["error"]["code"], "USAGE", "{document}");
    let hint = document["error"]["hint"].as_str().unwrap();
    assert!(!hint.starts_with("canon ") || hint == "canon retrieve --for-role <ROLE> --domain <DOMAIN>", "{hint}");
    assert_eq!(hint, "canon retrieve --for-role <ROLE> --domain <DOMAIN>");
    assert_eq!(document["error"]["field"], "--regime");
}

#[test]
fn atip_contract11_11_a_tool_wide_option_after_a_group_word_is_command_required_and_an_unknown_one_is_usage() {
    let dir = repo();
    for args in [vec!["finding", "--repo", ".", "--json"], vec!["finding", "--as", "planner", "--json"], vec!["finding", "--repo=.", "--as=reviewer", "--json"], vec!["--repo", ".", "--json"], vec!["brief", "--as", "planner", "--json"]] {
        let out = canon(dir.path(), &args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        let document = json(&out);
        assert_eq!(document["error"]["code"], "COMMAND_REQUIRED", "{args:?}: {document}");
        assert!(document["error"]["expected"].as_array().is_some_and(|e| !e.is_empty()), "{args:?}: expected holds the commands of that level: {document}");
        assert!(document["error"].get("received").is_none(), "{args:?}: {document}");
    }
    assert_eq!(json(&canon(dir.path(), &["finding", "--json"]))["error"]["expected"], json(&canon(dir.path(), &["finding", "--repo", ".", "--json"]))["error"]["expected"], "the group's own commands");
    let bogus = json(&canon(dir.path(), &["finding", "--bogus", "--json"]));
    assert_eq!((bogus["error"]["code"].as_str(), bogus["error"]["field"].as_str()), (Some("USAGE"), Some("--bogus")), "{bogus}");
    // With a command after the group word the option reaches the leaf that takes it.
    let listed = json(&canon(dir.path(), &["finding", "--repo", ".", "list", "--change", "add-login", "--json"]));
    assert_eq!(listed["ok"], true, "{listed}");
    assert_eq!(listed["meta"]["command"], "finding list");
    assert_eq!(listed["meta"]["context"]["repo"], ".");
    // A leaf that does not take the option still refuses it as USAGE, naming it.
    let refused = json(&canon(dir.path(), &["skills", "--repo", ".", "show", "canon", "--json"]));
    assert_eq!((refused["error"]["code"].as_str(), refused["error"]["field"].as_str()), (Some("USAGE"), Some("--repo")), "{refused}");
}
