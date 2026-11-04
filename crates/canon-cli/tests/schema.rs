//! `canon schema` (0.20 D1) through the built binary, run from a directory
//! that is not a canon repo.

use std::process::{Command, Output};

fn canon(args: &[&str]) -> Output {
    let dir = tempfile::tempdir().expect("tempdir");
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(dir.path()).output().expect("spawn canon binary")
}

fn canon_env(args: &[&str], role: &str) -> Output {
    let dir = tempfile::tempdir().expect("tempdir");
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).env("CANON_ROLE", role).current_dir(dir.path()).output().expect("spawn canon binary")
}

fn listing_len(out: &Output) -> usize {
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("stdout is JSON");
    v["data"]["commands"].as_array().unwrap().len()
}

fn json(args: &[&str]) -> serde_json::Value {
    let out = canon(args);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("stdout is JSON");
    assert_eq!(v["meta"]["command"], "schema", "{v}");
    v["data"].clone()
}

/// The declaration of a command: its document in the full model form (every option, implied options and codes listed).
fn model_command(name: &str) -> serde_json::Value {
    canon_cli::spec::model()["commands"].as_array().unwrap().iter().find(|c| c["command"] == name).unwrap_or_else(|| panic!("no command {name}")).clone()
}

fn option_of(doc: &serde_json::Value, name: &str) -> serde_json::Value {
    doc["options"].as_array().unwrap().iter().find(|o| o["name"] == name).unwrap_or_else(|| panic!("no --{name}")).clone()
}

/// The names a 1.4 command document lists: a string names a tree option or a reserved option, an object is the command's own option.
fn option_names(doc: &serde_json::Value) -> Vec<String> {
    doc["options"].as_array().map(|list| list.iter().map(|o| o.as_str().map_or_else(|| o["name"].as_str().unwrap().to_string(), str::to_string)).collect()).unwrap_or_default()
}

#[test]
fn bare_schema_lists_every_group_and_command_with_an_effect_in_a_stable_order() {
    let first = json(&["schema", "--json"]);
    assert_eq!(first, json(&["schema", "--json"]));
    let commands = first["commands"].as_array().unwrap();
    let names: Vec<&str> = commands.iter().map(|c| c["command"].as_str().unwrap()).collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);
    assert!(names.contains(&"evidence add") && names.contains(&"schema") && names.contains(&"evidence"));
    let purge = commands.iter().find(|c| c["command"] == "purge").unwrap();
    assert_eq!(purge["effect"], "destructive");
    let group = commands.iter().find(|c| c["command"] == "evidence").unwrap();
    assert_eq!((group["kind"].clone(), group.get("effect")), (serde_json::json!("group"), None));
}

#[test]
fn one_command_shows_options_with_their_shape() {
    // What the command DECLARES, read in the full model form.
    let m = model_command("evidence add");
    assert_eq!(m["command"], "evidence add");
    assert_eq!(option_of(&m, "all-scenarios")["valueType"], "switch");
    assert_eq!(option_of(&m, "report")["repeatable"], true);
    assert_eq!(option_of(&m, "report")["valueType"], "string");
    assert!(option_of(&m, "json")["flagOnly"] == true);
    let codes: Vec<&str> = m["errors"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
    for code in ["EVIDENCE_NOTE_INVALID", "BRIEF_APPROVAL_REQUIRED", "BRIEF_APPROVAL_STALE", "EVIDENCE_ARGUMENT_REQUIRED", "USAGE", "INPUT_INVALID", "BATCH_INCOMPLETE"] {
        assert!(codes.contains(&code), "{code} missing from {codes:?}");
    }
    assert!(!codes.contains(&"ARTIFACTS_STORE_FAILED"), "a sibling command's code leaked in: {codes:?}");
    // What `canon schema` ANSWERS: the 1.4 form, the call surface only.
    let v = json(&["schema", "evidence", "add", "--json"]);
    assert_eq!(v["command"], "evidence add");
    assert_eq!(v["effect"], "write");
    let names = option_names(&v);
    assert!(names.contains(&"all-scenarios".to_string()) && names.contains(&"report".to_string()), "{names:?}");
    assert!(!names.contains(&"json".to_string()) && !names.contains(&"input".to_string()), "implied options are not listed: {names:?}");
    let answered: Vec<&str> = v["errors"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
    for code in ["EVIDENCE_NOTE_INVALID", "BRIEF_APPROVAL_REQUIRED", "BRIEF_APPROVAL_STALE"] {
        assert!(answered.contains(&code), "{code} missing from {answered:?}");
    }
    for implied in ["USAGE", "INPUT_INVALID", "BATCH_INCOMPLETE", "EVIDENCE_ARGUMENT_REQUIRED"] {
        assert!(!answered.contains(&implied), "{implied} is implied or a finer USAGE and is not listed: {answered:?}");
    }
    assert!(!answered.contains(&"ARTIFACTS_STORE_FAILED"), "a sibling command's code leaked in: {answered:?}");
    assert!(v.get("exits").is_none() && v.get("errorCodes").is_none() && v.get("path").is_none() && v.get("schemas").is_none());
}

#[test]
fn a_command_lists_the_codes_it_can_emit_and_a_read_lists_no_writes_codes() {
    // The declaration (model) lists every code; the 1.4 answer omits the implied ones.
    let errors = |cmd: &[&str]| model_command(&cmd.join(" "))["errors"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect::<Vec<_>>();
    let answered = |cmd: &[&str]| {
        let mut args = vec!["schema"];
        args.extend_from_slice(cmd);
        args.push("--json");
        json(&args)["errors"].as_array().map(|list| list.iter().map(|c| c.as_str().unwrap().to_string()).collect::<Vec<_>>()).unwrap_or_default()
    };
    let context = errors(&["context"]);
    let finding_list = errors(&["finding", "list"]);
    assert!(context.contains(&"USAGE".to_string()) && finding_list.contains(&"USAGE".to_string()));
    assert!(finding_list.contains(&"CURSOR_INVALID".to_string()), "a command that pages lists the cursor refusal");
    assert!(!context.contains(&"CURSOR_INVALID".to_string()) && !context.contains(&"INPUT_INVALID".to_string()), "{context:?}");
    // `PROMOTION_FAILED` and `FINDING_INCOMPLETE` (exit 1) are codes `finding add` lists.
    assert!(errors(&["finding", "add"]).contains(&"PROMOTION_FAILED".to_string()));
    assert!(errors(&["finding", "add"]).contains(&"FINDING_INCOMPLETE".to_string()));
    assert!(errors(&["purge"]).contains(&"PLAN_STALE".to_string()));
    // The answer lists what the command emits besides the implied codes.
    assert!(answered(&["finding", "add"]).contains(&"PROMOTION_FAILED".to_string()));
    assert!(answered(&["finding", "add"]).contains(&"FINDING_INCOMPLETE".to_string()));
    for implied in ["USAGE", "INTERNAL", "CURSOR_INVALID", "PLAN_STALE", "INPUT_INVALID"] {
        assert!(!answered(&["finding", "list"]).contains(&implied.to_string()) && !answered(&["purge"]).contains(&implied.to_string()), "{implied} is implied and is not listed");
    }
}

#[test]
fn errors_lists_the_whole_registry() {
    let v = json(&["schema", "--errors", "--json"]);
    let listed: Vec<&str> = v["errors"].as_array().unwrap().iter().map(|e| e["code"].as_str().unwrap()).collect();
    let registry: Vec<&str> = canon_cli::cli_error::all_codes().iter().map(|info| info.code).collect();
    assert_eq!(listed, registry);
    assert!(v["errors"].as_array().unwrap().iter().all(|e| !e["meaning"].as_str().unwrap().is_empty()));
    let human = canon(&["schema", "--errors"]);
    assert!(human.status.success() && String::from_utf8_lossy(&human.stdout).contains("BRIEF_APPROVAL_STALE"));
    assert_eq!(canon(&["schema", "--errors", "evidence", "add"]).status.code(), Some(2));
}

#[test]
fn enum_options_list_their_values_and_defaults() {
    let m = model_command("init");
    let policy = option_of(&m, "policy");
    assert!(policy["possibleValues"].as_array().unwrap().iter().any(|p| p == "strict"));
    // `init` sets up the directory as given (default `.`); every other leaf has no `--repo` default: absent means discovery.
    assert_eq!(option_of(&m, "repo")["default"], ".");
    let gate = model_command("gate check");
    let repo = option_of(&gate, "repo");
    assert!(repo["default"].is_null(), "{repo}");
    assert!(repo["defaultNote"].as_str().unwrap().contains("canon.yaml"), "{repo}");
    // The answer: `init` defines its own `--repo` (default `.`), `gate check` names the tree's `--repo` (no default); the tree defines it once.
    let v = json(&["schema", "init", "--json"]);
    let own = v["options"].as_array().unwrap().iter().find(|o| o["name"] == "repo").expect("init defines its own --repo");
    assert_eq!(own["default"], ".");
    let answered_policy = v["options"].as_array().unwrap().iter().find(|o| o["name"] == "policy").unwrap();
    assert!(answered_policy["possibleValues"].as_array().unwrap().iter().any(|p| p == "strict") && answered_policy["default"] == "strict");
    let tree = json(&["schema", "--json"]);
    let shared = tree["options"].as_array().unwrap().iter().find(|o| o["name"] == "repo").expect("the tree defines --repo once");
    assert!(shared.get("default").is_none() && shared["defaultNote"].as_str().unwrap().contains("canon.yaml"), "{shared}");
    assert!(option_names(&json(&["schema", "gate", "check", "--json"])).contains(&"repo".to_string()));
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
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("UNKNOWN_COMMAND") && stderr.contains("suggestions: add") && stderr.contains("expected: add"), "{stderr}");
}

#[test]
fn error_returns_that_codes_row_and_an_unknown_code_exits_2() {
    let v = json(&["schema", "--error", "CONFIG_REQUIRED", "--json"]);
    assert_eq!(v["error"]["code"], "CONFIG_REQUIRED", "{v}");
    assert_eq!(v["error"]["exit"], 2, "{v}");
    let out = canon(&["schema", "--error", "NOPE", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    let body: serde_json::Value = serde_json::from_slice(&out.stdout).expect("stdout is JSON");
    assert_eq!(body["ok"], false);
    assert_eq!((body["error"]["code"].as_str(), body["error"]["field"].as_str()), (Some("USAGE"), Some("--error")), "{body}");
}

#[test]
fn errors_error_and_an_operand_conflict() {
    assert_eq!(canon(&["schema", "--errors", "--error", "CONFIG_REQUIRED"]).status.code(), Some(2));
    assert_eq!(canon(&["schema", "--warnings", "--warning", "OLD_LAYOUT_FOUND"]).status.code(), Some(2));
    assert_eq!(canon(&["schema", "--warning", "OLD_LAYOUT_FOUND", "evidence", "add"]).status.code(), Some(2));
    assert_eq!(canon(&["schema", "--error", "CONFIG_REQUIRED", "evidence", "add"]).status.code(), Some(2));
    assert_eq!(canon(&["schema", "--errors", "--error", "CONFIG_REQUIRED", "evidence", "add"]).status.code(), Some(2));
}

#[test]
fn schema_does_not_read_canon_role_so_a_bogus_one_is_neither_a_warning_nor_a_failure() {
    for args in [&["schema", "--json"][..], &["schema", "--errors", "--json"][..], &["schema", "finding", "--json"][..]] {
        let out = canon_env(args, "bogus");
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("stdout is JSON");
        assert_eq!(v["ok"], true, "{v}");
        assert!(v["meta"].get("warnings").is_none(), "schema reads no CANON_ROLE, so it has nothing to warn about: {v}");
        assert_eq!(out.stdout, canon(args).stdout, "{args:?}: the answer is the same with and without the variable");
    }
}

#[test]
fn canon_role_never_filters_the_listing_but_as_does() {
    assert_eq!(canon_env(&["schema", "--json"], "reviewer").stdout, canon(&["schema", "--json"]).stdout);
    let all = listing_len(&canon(&["schema", "--json"]));
    assert_eq!(listing_len(&canon_env(&["schema", "--json"], "implementer")), all);
    let filtered = listing_len(&canon(&["schema", "--as", "implementer", "--json"]));
    assert!(filtered < all, "--as implementer must filter: {filtered} of {all}");
}
