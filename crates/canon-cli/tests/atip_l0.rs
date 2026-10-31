//! The atip/1 L0 output contract of canon 0.33 (`canon-0-33-atip-l0`, scenarios
//! `atip.l0.*`), against the built binary: the envelope's `meta`, members with no
//! value left out, paging in `meta.page`, next steps, the codes of a wrong call,
//! exits from the registry, and the removed names.
//!
//! Tests are named by scenario id (`atip_l0_01_…` proves `atip.l0.01`).

use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use canon_cli::cli_error::{CliError, CliErrorExt as _};
use serde_json::Value;
use tempfile::TempDir;

/// The members `meta` may hold: the contract's, and canon's `x-canon` (spec 6.6).
const META_MEMBERS: &[&str] = &["schemaVersion", "tool", "toolVersion", "command", "contract", "output", "context", "next", "warnings", "coverage", "page", "fields", "truncated", "x-canon"];

fn canon(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(dir).env_remove("CANON_ROLE").env("CANON_ACTOR", "atip-l0").output().expect("spawn canon binary")
}

fn canon_as(dir: &Path, role: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(dir).env("CANON_ROLE", role).env("CANON_ACTOR", "atip-l0").output().expect("spawn canon binary")
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("stdout is one JSON document ({e}): {}\nstderr: {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)))
}

fn ok(dir: &Path, args: &[&str]) {
    let out = canon(dir, args);
    assert!(out.status.success(), "{args:?}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}

/// A repo with a subject, a change, a feature and a scenario.
fn scratch_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    ok(repo, &["init", "--providers", "none"]);
    ok(repo, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(repo, &["subject", "new", "demo-two", "--domain", "dev", "--title", "Demo two"]);
    ok(repo, &["change", "new", "add-login", "--subject", "demo", "--title", "Add login"]);
    ok(repo, &["feature", "new", "auth.login", "--title", "Login"]);
    ok(repo, &["scenario", "new", "auth.login.01", "--title", "Sign in", "--feature", ".canon/specs/kind=feature/area=auth/login.feature"]);
    ok(repo, &["inventory", "sync"]);
    for args in [&["init", "-q"][..], &["add", "-A"], &["-c", "user.name=t", "-c", "user.email=t@example.com", "commit", "-q", "-m", "fixture"]] {
        assert!(Command::new("git").args(args).current_dir(repo).status().unwrap().success());
    }
    dir
}

/// Every value in `value` that is null, and every empty array or object.
fn voids(value: &Value, at: &str, found: &mut Vec<String>) {
    match value {
        Value::Null => found.push(format!("{at} is null")),
        Value::Array(items) if items.is_empty() => found.push(format!("{at} is []")),
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                voids(item, &format!("{at}/{i}"), found);
            }
        }
        Value::Object(map) if map.is_empty() => found.push(format!("{at} is {{}}")),
        // `coverage` says what a check examined: its content is canon's own (an empty `skipped` is information).
        Value::Object(_) if at.ends_with("meta/coverage") => {}
        Value::Object(map) => {
            for (key, item) in map {
                voids(item, &format!("{at}/{key}"), found);
            }
        }
        _ => {}
    }
}

fn meta_keys(document: &Value) -> BTreeSet<String> {
    document["meta"].as_object().expect("meta is an object").keys().cloned().collect()
}

#[test]
fn atip_l0_01_status_json_names_the_tool_its_version_and_the_contract() {
    let repo = scratch_repo();
    let version = String::from_utf8(canon(repo.path(), &["--version"]).stdout).unwrap();
    let document = json(&canon(repo.path(), &["status", "--json"]));
    let meta = &document["meta"];
    assert_eq!(meta["tool"], "canon");
    assert_eq!(meta["contract"], "atip/1");
    assert_eq!(version.trim(), format!("canon {}", meta["toolVersion"].as_str().unwrap()), "toolVersion is the version `canon --version` prints");
    assert!(meta.get("canonVersion").is_none(), "canonVersion is gone");
    assert_eq!(meta["command"], "status");
    assert!(meta["schemaVersion"].as_str().is_some_and(|v| v.split('.').count() == 2));
    assert!(atip::envelope_problems(&document).is_empty());
}

#[test]
fn atip_l0_02_an_output_is_named_when_the_command_declares_several() {
    let dir = TempDir::new().unwrap();
    let init = json(&canon(dir.path(), &["init", "--providers", "none", "--json"]));
    assert_eq!(init["meta"]["output"], "InitReport");
    let check = json(&canon(dir.path(), &["init", "--check", "--json"]));
    assert_eq!(check["meta"]["output"], "InitCheckReport");
    ok(dir.path(), &["feature", "new", "auth.login", "--title", "Login"]);
    let batch = json(&canon(dir.path(), &["scenario", "new", "--json", "--input", r#"[{"operands":["auth.login.01"],"title":"Sign in"},{"operands":["auth.login.02"],"title":"Sign out"}]"#]));
    assert_eq!(batch["meta"]["output"], "BatchScenarioNewReport");
    let single = json(&canon(dir.path(), &["scenario", "new", "auth.login.03", "--title", "Reset", "--json"]));
    assert_eq!(single["meta"]["output"], "ScenarioNewReport");
    let status = json(&canon(dir.path(), &["status", "--json"]));
    assert!(status["meta"].get("output").is_none(), "a command with one output does not name it");
}

#[test]
fn atip_l0_03_an_unresolved_command_leaves_command_out_and_a_half_resolved_one_names_the_path() {
    let dir = TempDir::new().unwrap();
    let top = json(&canon(dir.path(), &["nosuch", "--json"]));
    assert!(top["meta"].get("command").is_none(), "{top}");
    assert_eq!(top["meta"]["schemaVersion"], "1.0");
    assert_eq!(top["meta"]["tool"], "canon");
    let half = json(&canon(dir.path(), &["finding", "nosuch", "--json"]));
    assert_eq!(half["meta"]["command"], "finding");
    assert_eq!(half["error"]["code"], "UNKNOWN_COMMAND");
}

#[test]
fn atip_l0_04_context_holds_only_the_context_options_with_relative_paths() {
    let repo = scratch_repo();
    std::fs::create_dir_all(repo.path().join("sub/dir")).unwrap();
    let from_repo = json(&canon(repo.path(), &["status", "--json"]));
    assert_eq!(from_repo["meta"]["context"], serde_json::json!({ "repo": "." }));
    let nested = json(&canon(&repo.path().join("sub/dir"), &["status", "--json"]));
    assert_eq!(nested["meta"]["context"], serde_json::json!({ "repo": "../.." }));
    let given = json(&canon(&repo.path().join("sub"), &["status", "--repo", "..", "--json"]));
    assert_eq!(given["meta"]["context"], serde_json::json!({ "repo": ".." }));
    let with_role = json(&canon_as(repo.path(), "reviewer", &["status", "--json"]));
    assert_eq!(with_role["meta"]["context"], serde_json::json!({ "repo": "." }), "neither the role nor the actor is a context option");
    let schema = json(&canon(repo.path(), &["schema", "--json"]));
    assert!(schema["meta"].get("context").is_none(), "a leaf that takes no context option has no context");
}

#[test]
fn atip_l0_05_a_member_with_no_value_is_absent() {
    let repo = scratch_repo();
    let runs: Vec<Vec<&str>> = vec![
        vec!["status", "--json"],
        vec!["gate", "check", "--json"],
        vec!["subject", "show", "nosuch", "--json"],
        vec!["query", "--kind", "evidence", "--json"],
        vec!["nosuch", "--json"],
        vec!["status", "--bogus", "--json"],
        vec!["schema", "--json"],
    ];
    let mut found = Vec::new();
    for args in &runs {
        let document = json(&canon(repo.path(), args));
        voids(&document["meta"], &format!("{args:?} meta"), &mut found);
        if let Some(error) = document.get("error") {
            voids(error, &format!("{args:?} error"), &mut found);
            assert!(error["retryable"].is_boolean(), "{args:?}: retryable is always there");
        }
        let keys = meta_keys(&document);
        assert!(keys.iter().all(|key| META_MEMBERS.contains(&key.as_str())), "{args:?}: meta holds {keys:?}");
    }
    assert!(found.is_empty(), "members with no value: {found:?}");
    // The error value itself: no hint, no field, no suggestions is three absent keys.
    let bare = CliError::refused("USAGE", "bare").body();
    let text = serde_json::to_string(&bare).unwrap();
    assert_eq!(text, r#"{"code":"USAGE","message":"bare","retryable":false}"#);
}

#[test]
fn atip_l0_06_every_envelope_the_binary_prints_passes_the_contract_schema() {
    let repo = scratch_repo();
    let runs: Vec<Vec<&str>> = vec![
        vec!["status", "--json"],
        vec!["status", "--fields", "subjects", "--json"],
        vec!["gate", "check", "--json"],
        vec!["query", "--kind", "subject", "--limit", "1", "--json"],
        vec!["subject", "show", "demo", "--json"],
        vec!["context", "--json"],
        vec!["schema", "--errors", "--json"],
        vec!["nosuch", "--json"],
        vec!["--json"],
        vec!["finding", "--json"],
        vec!["status", "--bogus", "--json"],
        vec!["query", "--kind", "subject", "--cursor", "zz", "--json"],
    ];
    for args in &runs {
        let document = json(&canon(repo.path(), args));
        let problems = atip::envelope_problems(&document);
        assert!(problems.is_empty(), "{args:?} breaks the contract envelope schema: {problems:?}\n{document}");
    }
}

#[test]
fn atip_l0_08_a_page_is_in_meta_page_with_a_next_step_for_the_next_one() {
    let repo = scratch_repo();
    let first = json(&canon(repo.path(), &["query", "--kind", "subject", "--limit", "1", "--json"]));
    let meta = &first["meta"];
    assert_eq!(meta["page"]["returned"], 1);
    assert_eq!(meta["page"]["total"], 2);
    let cursor = meta["page"]["nextCursor"].as_str().expect("more follow, so a cursor");
    for old in ["hasMore", "returned", "total", "nextCursor"] {
        assert!(meta.get(old).is_none(), "`{old}` is not a top-level meta member any more");
    }
    let steps = meta["next"].as_array().expect("the next page is a step");
    let step = steps.iter().find(|s| s["argv"].as_array().is_some_and(|a| a.iter().any(|w| w == "--cursor"))).expect("a step with --cursor");
    let argv: Vec<&str> = step["argv"].as_array().unwrap().iter().map(|w| w.as_str().unwrap()).collect();
    let at = argv.iter().position(|w| *w == "--cursor").unwrap();
    assert_eq!(argv[at + 1], cursor);
    assert!(argv.contains(&"query") && argv.contains(&"--json") && argv.contains(&"--limit"));
    // Running the step gives the last page: neither a cursor nor a step for one.
    let second = json(&canon(repo.path(), &argv));
    assert_eq!(second["meta"]["page"]["returned"], 1);
    assert!(second["meta"]["page"].get("nextCursor").is_none(), "{second}");
    let leftover = second["meta"].get("next").and_then(Value::as_array).map_or(0, |steps| steps.iter().filter(|s| s["argv"].to_string().contains("--cursor")).count());
    assert_eq!(leftover, 0, "the last page has no cursor step");
}

#[test]
fn atip_l0_09_a_repeated_write_reports_unchanged_in_data_and_the_role_split_in_data() {
    let repo = scratch_repo();
    let args = ["evidence", "add", "--scenario", "auth.login.01", "--class", "test-run", "--ref", "cargo test", "--actor", "unit-1", "--session", "s-1", "--override-reason", "atip-l0 fixture", "--json"];
    let first = json(&canon_as(repo.path(), "implementer", &args));
    assert!(first["data"].get("unchanged").is_none(), "a first write is not unchanged: {first}");
    assert!(first["data"]["waitingOn"].is_object(), "under a declared role a write says what the other roles wait on, in data: {first}");
    assert!(first["meta"].get("waitingOn").is_none());
    let second = json(&canon_as(repo.path(), "implementer", &args));
    assert_eq!(second["data"]["unchanged"], true, "{second}");
    assert!(second["meta"].get("unchanged").is_none());
    assert!(atip::envelope_problems(&second).is_empty());
    // A report that is a stored record cannot take members: they go to meta.x-canon.
    let adopt = ["subject", "adopt", "demo", "--change", "add-login", "--json"];
    let once = json(&canon_as(repo.path(), "implementer", &adopt));
    let again = json(&canon_as(repo.path(), "implementer", &adopt));
    assert!(once["data"].get("unchanged").is_none() && again["data"].get("unchanged").is_none(), "the record stays verbatim: {again}");
    assert_eq!(again["meta"]["x-canon"]["unchanged"], true, "{again}");
}

#[test]
fn atip_l0_10_projected_and_paged_output_holds_only_defined_meta_members() {
    let repo = scratch_repo();
    for args in [vec!["query", "--kind", "subject", "--fields", "subject_id,status", "--json"], vec!["query", "--kind", "subject", "--limit", "1", "--json"], vec!["finding", "list", "--change", "add-login", "--json"], vec!["status", "--fields", "subjects", "--json"]] {
        let document = json(&canon(repo.path(), &args));
        let keys = meta_keys(&document);
        assert!(keys.iter().all(|key| META_MEMBERS.contains(&key.as_str())), "{args:?}: meta holds {keys:?}");
        assert!(atip::envelope_problems(&document).is_empty(), "{args:?}");
    }
    // A key the contract does not define fails the envelope validation.
    let mut document = json(&canon(repo.path(), &["status", "--json"]));
    document["meta"]["hasMore"] = Value::Bool(false);
    assert!(!atip::envelope_problems(&document).is_empty());
}

#[test]
fn atip_l0_11_a_cursor_canon_did_not_issue_is_refused_with_cursor_invalid() {
    let repo = scratch_repo();
    let out = canon(repo.path(), &["query", "--kind", "subject", "--cursor", "zzz", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    let document = json(&out);
    assert_eq!(document["error"]["code"], "CURSOR_INVALID");
    assert_eq!(document["error"]["field"], "--cursor");
    let other = json(&canon(repo.path(), &["query", "--kind", "subject", "--limit", "1", "--json"]))["meta"]["page"]["nextCursor"].as_str().unwrap().to_string();
    let foreign = canon(repo.path(), &["finding", "list", "--change", "add-login", "--cursor", &other, "--json"]);
    assert_eq!(foreign.status.code(), Some(2));
    assert_eq!(json(&foreign)["error"]["code"], "CURSOR_INVALID", "a cursor of another listing is refused");
}

#[test]
fn atip_l0_12_next_entries_are_runnable_with_reason_and_json() {
    let repo = scratch_repo();
    let mut seen = 0;
    for role in ["implementer", "planner", "reviewer", "orchestrator"] {
        let document = json(&canon_as(repo.path(), role, &["status", "--json"]));
        for step in document["meta"]["next"].as_array().into_iter().flatten() {
            seen += 1;
            assert!(step.get("why").is_none(), "`why` is `reason`: {step}");
            assert!(step["reason"].as_str().is_some_and(|r| !r.is_empty()), "{step}");
            let argv: Vec<&str> = step["argv"].as_array().expect("argv").iter().map(|w| w.as_str().unwrap()).collect();
            assert!(!argv.is_empty());
            assert!(argv.iter().all(|word| !word.contains('<')), "no placeholder in {argv:?}");
            assert!(argv.contains(&"--json"), "JSON mode runs the next step in JSON: {argv:?}");
            let mut shell = vec!["canon".to_string()];
            shell.extend(argv.iter().map(ToString::to_string));
            assert_eq!(atip::split_argv(step["command"].as_str().unwrap()), shell, "command splits into canon + argv");
            assert!(step["x-canon"]["actor"].is_string(), "{step}");
            assert!(step.get("actor").is_none() && step.get("role").is_none());
        }
    }
    assert!(seen > 0, "the repo has steps to check");
    // A write names its next steps the same way.
    let written = json(&canon_as(repo.path(), "implementer", &["scenario", "new", "auth.login.02", "--title", "Sign out", "--feature", ".canon/specs/kind=feature/area=auth/login.feature", "--json"]));
    for step in written["meta"]["next"].as_array().into_iter().flatten() {
        assert!(step["argv"].as_array().unwrap().iter().any(|w| w == "--json"), "{step}");
    }
}

#[test]
fn atip_l0_13_a_warning_is_a_coded_object_with_a_registered_code() {
    let repo = scratch_repo();
    // An unknown CANON_ROLE is ignored where the role does not matter, with a warning.
    let out = Command::new(env!("CARGO_BIN_EXE_canon")).args(["context", "--json"]).current_dir(repo.path()).env("CANON_ROLE", "bogus-role").output().unwrap();
    let document = json(&out);
    let warnings = document["meta"]["warnings"].as_array().expect("a warning in meta.warnings");
    assert!(!warnings.is_empty());
    let registered: BTreeSet<&str> = canon_cli::cli_error::all_codes().iter().map(|info| info.code).collect();
    for warning in warnings {
        assert!(warning["code"].as_str().is_some_and(|code| registered.contains(code)), "{warning}");
        assert!(warning["message"].as_str().is_some_and(|m| !m.is_empty()));
        assert!(warning.is_object() && !warning.is_string());
    }
    assert!(out.stderr.is_empty(), "JSON mode keeps stderr empty");
}

#[test]
fn atip_l0_14_guidance_with_a_placeholder_is_not_a_next_entry() {
    let repo = scratch_repo();
    for role in ["implementer", "planner", "reviewer", "orchestrator"] {
        let document = json(&canon_as(repo.path(), role, &["status", "--json"]));
        for step in document["meta"]["next"].as_array().into_iter().flatten() {
            let text = step.to_string();
            assert!(!text.contains("<area>") && !text.contains("<unit>") && !text.contains("<behavior>"), "{role}: a placeholder in {text}");
        }
    }
    // The same guidance is still said to a person.
    let text = canon_as(repo.path(), "implementer", &["status"]);
    let said = String::from_utf8_lossy(&text.stdout).to_string();
    assert!(said.contains("canon "), "{said}");
}

#[test]
fn atip_l0_15_an_unknown_command_answers_unknown_command() {
    let dir = TempDir::new().unwrap();
    let out = canon(dir.path(), &["nosuch", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stderr.is_empty());
    let document = json(&out);
    let error = &document["error"];
    assert_eq!(error["code"], "UNKNOWN_COMMAND");
    assert_eq!(error["received"], "nosuch");
    let expected: Vec<&str> = error["expected"].as_array().unwrap().iter().map(|w| w.as_str().unwrap()).collect();
    assert!(expected.contains(&"status") && expected.contains(&"finding") && expected.contains(&"gate"), "{expected:?}");
    let half = json(&canon(dir.path(), &["finding", "lisst", "--json"]));
    assert_eq!(half["error"]["code"], "UNKNOWN_COMMAND");
    assert!(half["error"]["expected"].as_array().unwrap().iter().filter_map(Value::as_str).any(|word| word == "list"));
    assert_eq!(half["error"]["suggestions"][0], "list", "the nearest word comes first: {half}");
    // `--json` before the path is accepted.
    let before = json(&canon(dir.path(), &["--json", "nosuch"]));
    assert_eq!(before["error"]["code"], "UNKNOWN_COMMAND");
}

#[test]
fn atip_l0_16_a_bare_tool_or_group_answers_command_required() {
    let dir = TempDir::new().unwrap();
    for (args, level) in [(vec!["--json"], "status"), (vec!["finding", "--json"], "list"), (vec!["--json", "finding"], "close")] {
        let out = canon(dir.path(), &args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(out.stderr.is_empty());
        let document = json(&out);
        assert_eq!(document["error"]["code"], "COMMAND_REQUIRED", "{args:?}: {document}");
        let expected: Vec<&str> = document["error"]["expected"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
        assert!(expected.contains(&level), "{args:?}: {expected:?}");
    }
}

#[test]
fn atip_l0_17_an_unknown_option_a_missing_value_or_an_unknown_field_name_is_usage() {
    let repo = scratch_repo();
    let bogus = canon(repo.path(), &["status", "--bogus", "--json"]);
    assert_eq!(bogus.status.code(), Some(2));
    let document = json(&bogus);
    assert_eq!(document["error"]["code"], "USAGE");
    assert_eq!(document["error"]["field"], "--bogus");
    assert_eq!(document["meta"]["command"], "status");
    let missing = json(&canon(repo.path(), &["query", "--kind", "--json"]));
    assert_eq!(missing["error"]["code"], "USAGE", "{missing}");
    let fields = canon(repo.path(), &["query", "--kind", "subject", "--fields", "nosuch", "--json"]);
    assert_eq!(fields.status.code(), Some(2));
    let document = json(&fields);
    assert_eq!(document["error"]["code"], "USAGE");
    assert_eq!(document["error"]["field"], "--fields");
    assert!(document["error"]["expected"].as_array().is_some_and(|fields| !fields.is_empty()), "expected lists the selectable fields: {document}");
    let status_fields = json(&canon(repo.path(), &["status", "--fields", "nosuch", "--json"]));
    assert_eq!(status_fields["error"]["code"], "USAGE");
    assert_eq!(status_fields["error"]["field"], "--fields");
}

#[test]
fn atip_l0_18_a_text_error_opens_stderr_with_the_tool_code_and_message() {
    let repo = scratch_repo();
    for args in [vec!["nosuch"], vec!["status", "--bogus"], vec!["finding"], vec!["query", "--kind", "subject", "--cursor", "zzz"], vec!["subject", "show", "nosuch"]] {
        let out = canon(repo.path(), &args);
        assert!(out.stdout.is_empty(), "{args:?}: stdout stays empty: {}", String::from_utf8_lossy(&out.stdout));
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        let first = stderr.lines().next().unwrap_or_default();
        let rest = first.strip_prefix("canon: ").unwrap_or_else(|| panic!("{args:?}: the first line opens with `canon: `: {stderr}"));
        let (code, message) = rest.split_once(": ").unwrap_or_else(|| panic!("{args:?}: `<CODE>: <message>`: {first}"));
        assert!(code.chars().all(|c| c.is_ascii_uppercase() || c == '_'), "{args:?}: {code}");
        assert!(!message.is_empty());
        assert_eq!(out.status.code(), Some(2), "{args:?}");
    }
    let unknown = String::from_utf8_lossy(&canon(repo.path(), &["nosuch"]).stderr).to_string();
    assert!(unknown.starts_with("canon: UNKNOWN_COMMAND: "), "{unknown}");
    assert!(unknown.contains("\n  expected: "), "{unknown}");
    let option = String::from_utf8_lossy(&canon(repo.path(), &["status", "--bogus"]).stderr).to_string();
    assert!(option.contains("\n  field: --bogus"), "{option}");
    assert!(!option.contains("Usage:"), "clap's own text is not printed: {option}");
}

#[test]
fn atip_l0_19_the_registry_holds_the_reserved_codes_and_none_of_the_old_names() {
    let rows: std::collections::BTreeMap<&str, (u8, bool)> = canon_cli::cli_error::all_codes().iter().map(|info| (info.code, (info.exit, info.retryable))).collect();
    let reserved: [(&str, u8); 11] = [("USAGE", 2), ("UNKNOWN_COMMAND", 2), ("COMMAND_REQUIRED", 2), ("INTERNAL", 1), ("UNVERIFIABLE", 1), ("INPUT_INVALID", 2), ("INPUT_CONFLICT", 2), ("INPUT_TOO_LARGE", 2), ("BATCH_INCOMPLETE", 1), ("PLAN_STALE", 2), ("CURSOR_INVALID", 2)];
    for (code, exit) in reserved {
        assert_eq!(rows.get(code).copied(), Some((exit, false)), "{code} carries the contract's exit and retryable");
    }
    assert!(!rows.contains_key("OPERATION_REUSED"), "canon has no --operation");
    for old in OLD_CODES {
        assert!(!rows.contains_key(old), "`{old}` is gone from the registry");
    }
    // Every row conforms to the contract's registry-entry schema.
    for info in canon_cli::cli_error::all_codes() {
        let row = serde_json::json!({ "code": info.code, "exit": info.exit, "retryable": info.retryable, "meaning": info.meaning, "fix": info.fix });
        assert!(atip::registry_entry_problems(&row).is_empty(), "{}: {:?}", info.code, atip::registry_entry_problems(&row));
    }
    // And no source, spec file or skill still spells an old code.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    for (dir, ext) in [("crates/canon-cli/src", "rs"), ("crates/atip/src", "rs"), ("crates/canon-vocab/src", "rs"), (".canon/specs", "feature"), ("canon/skills", "md")] {
        collect(&root.join(dir), ext, &mut files);
    }
    assert!(files.len() > 100, "the scan found {} files", files.len());
    let mut hits = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap();
        for old in OLD_CODES {
            if word_in(&text, old) {
                hits.push(format!("{} spells {old}", file.strip_prefix(&root).unwrap().display()));
            }
        }
    }
    assert!(hits.is_empty(), "{hits:#?}");
}

/// The error codes 0.33 deletes (the CHANGELOG's replacement table lists them).
const OLD_CODES: &[&str] = &["PLAN_CHANGED", "INPUT_ELEMENT_FAILED", "INPUT_BATCH_UNSUPPORTED", "INPUT_INVALID_TYPE", "INPUT_UNKNOWN_FIELD", "INPUT_UNREADABLE", "INPUT_STDIN_TTY", "INPUT_STDIN_CONFLICT", "INPUT_MISSING", "INVALID_CURSOR", "UNKNOWN_FIELD"];

/// `word` in `text` as a whole identifier (so `CONTEXT_INPUT_MISSING` is not `INPUT_MISSING`).
fn word_in(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        let part = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
        !part(before) && !part(after)
    })
}

fn collect(dir: &Path, ext: &str, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, ext, out);
        } else if path.extension().is_some_and(|e| e == ext) {
            out.push(path);
        }
    }
}

#[test]
fn atip_l0_20_a_panic_answers_internal_with_exit_one_never_101() {
    if !cfg!(debug_assertions) {
        eprintln!("skipped: the panic trigger exists only in debug builds");
        return;
    }
    let dir = TempDir::new().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_canon")).args(["status", "--json"]).current_dir(dir.path()).env("CANON_TEST_PANIC", "1").output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(out.stderr.is_empty(), "JSON mode keeps stderr empty: {}", String::from_utf8_lossy(&out.stderr));
    let document = json(&out);
    assert_eq!(document["error"]["code"], "INTERNAL");
    assert_eq!(document["error"]["retryable"], false);
    let hint = document["error"]["hint"].as_str().unwrap();
    assert!(hint.contains("report"), "{hint}");
    assert!(atip::envelope_problems(&document).is_empty());
    let text = Command::new(env!("CARGO_BIN_EXE_canon")).args(["status"]).current_dir(dir.path()).env("CANON_TEST_PANIC", "1").output().unwrap();
    assert_eq!(text.status.code(), Some(1));
    assert!(text.stdout.is_empty());
    assert!(String::from_utf8_lossy(&text.stderr).starts_with("canon: INTERNAL: "));
}

#[cfg(unix)]
#[test]
fn atip_l0_21_a_closed_stdout_is_a_signal_exit_outside_the_registry() {
    use std::os::unix::process::ExitStatusExt;
    let (reader, writer) = std::io::pipe().expect("pipe");
    drop(reader);
    let repo = scratch_repo();
    let out = Command::new(env!("CARGO_BIN_EXE_canon")).args(["query", "--kind", "subject", "--all", "--json"]).current_dir(repo.path()).stdout(Stdio::from(writer)).stderr(Stdio::piped()).output().unwrap();
    assert_eq!(out.status.signal(), Some(13), "a signal exit, not a registry code: {:?}", out.status);
    assert_eq!(out.status.code(), None);
    assert!(out.stderr.is_empty(), "{}", String::from_utf8_lossy(&out.stderr));
}

/// Runs `canon` with its stdin as a terminal (a pty from `script`), when `script` exists.
#[cfg(unix)]
fn canon_on_a_terminal(dir: &Path, args: &[&str]) -> Option<String> {
    let exe = env!("CARGO_BIN_EXE_canon");
    let mut command = Command::new("script");
    if cfg!(target_os = "macos") {
        command.args(["-q", "/dev/null", exe]).args(args);
    } else {
        let line = std::iter::once(exe).chain(args.iter().copied()).map(atip::quote).collect::<Vec<_>>().join(" ");
        command.args(["-qec", &line, "/dev/null"]);
    }
    let out = command.current_dir(dir).env_remove("CANON_ROLE").stdin(Stdio::null()).output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

#[cfg(unix)]
#[test]
fn atip_l0_22_input_dash_beside_another_stdin_reader_is_usage_and_on_a_terminal_input_invalid() {
    use std::io::Write;
    let repo = scratch_repo();
    let mut child = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["evidence", "add", "--input", "-", "--report", "junit:-", "--json"])
        .current_dir(repo.path())
        .env_remove("CANON_ROLE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"{}").unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let document = json(&out);
    assert_eq!(document["error"]["code"], "USAGE", "{document}");
    assert_eq!(document["error"]["field"], "--input");
    // On a terminal `--input -` has nothing to read.
    // A terminal stdin needs a pty: `script` makes one. Without it the case cannot run, and that is said, not passed over.
    let Some(text) = canon_on_a_terminal(repo.path(), &["scenario", "new", "--input", "-", "--json"]) else {
        panic!("atip.l0.22 needs the `script` command to give canon a terminal on stdin; install it or run the case on a machine that has it");
    };
    // The pty echoes its own control characters ahead of the document: take it from the first `{"ok"`.
    let at = text.find("{\"ok\"").unwrap_or_else(|| panic!("no JSON document from the terminal run: {text:?}"));
    let document: Value = serde_json::from_str(text[at..].lines().next().unwrap().trim()).unwrap();
    assert_eq!(document["error"]["code"], "INPUT_INVALID", "{document}");
    assert_eq!(document["error"]["field"], "--input");
    assert_eq!(document["error"]["x-canon"]["reason"], "tty", "{document}");
    assert!(atip::envelope_problems(&document).is_empty());
}

#[test]
fn atip_l0_26_the_spec_file_and_the_conformance_corpus_are_the_new_contract() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    // The checked-in spec is the generated one, and carries the contract's name, not a canon envelope schema.
    let on_disk = std::fs::read_to_string(root.join(canon_cli::spec::SPEC_PATH)).unwrap();
    assert_eq!(on_disk, canon_cli::spec::render(), "docs/cli/canon.cli.json is generated: CANON_BLESS_CLI_SPEC=1 cargo test -p canon-cli --test cli_spec");
    let spec: Value = serde_json::from_str(&on_disk).unwrap();
    assert_eq!(spec["contract"], "atip/1");
    assert!(spec.get("envelopes").is_none());
    // Every JSON envelope a conformance case freezes passes the contract's envelope schema and names the tool.
    let mut envelopes = 0;
    let mut cases = Vec::new();
    collect(&root.join("conformance/cases"), "out", &mut cases);
    for file in cases {
        for line in std::fs::read_to_string(&file).unwrap().lines().filter(|line| line.starts_with("{\"ok\":")) {
            let document: Value = serde_json::from_str(line).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
            let problems = atip::envelope_problems(&document);
            assert!(problems.is_empty(), "{} breaks the contract envelope schema: {problems:?}\n{line}", file.display());
            assert_eq!(document["meta"]["tool"], "canon");
            envelopes += 1;
        }
    }
    assert!(envelopes > 30, "the corpus holds {envelopes} envelopes");
}

#[test]
fn atip_l0_28_a_failure_names_the_tool_its_version_and_the_contract() {
    let repo = scratch_repo();
    let version = String::from_utf8(canon(repo.path(), &["--version"]).stdout).unwrap();
    for args in [vec!["status", "--bogus", "--json"], vec!["nosuch", "--json"], vec!["--json"], vec!["query", "--kind", "subject", "--cursor", "zz", "--json"]] {
        let out = canon(repo.path(), &args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        let document = json(&out);
        assert_eq!(document["ok"], false);
        let meta = &document["meta"];
        assert_eq!(meta["tool"], "canon", "{args:?}");
        assert_eq!(meta["contract"], "atip/1");
        assert_eq!(format!("canon {}", meta["toolVersion"].as_str().unwrap()), version.trim());
        assert!(meta["schemaVersion"].is_string());
        assert!(document["error"]["retryable"].is_boolean());
        assert!(meta.get("canonVersion").is_none());
    }
}

#[test]
fn atip_l0_29_input_twice_or_without_a_value_is_usage_on_input() {
    let repo = scratch_repo();
    let before = std::fs::read_to_string(repo.path().join(".canon/specs/kind=feature/area=auth/login.feature")).unwrap();
    let twice = canon(repo.path(), &["scenario", "new", "--input", "{}", "--input", "{}", "--json"]);
    let bare = canon(repo.path(), &["scenario", "new", "--json", "--input"]);
    for out in [&twice, &bare] {
        assert_eq!(out.status.code(), Some(2));
        let document = json(out);
        assert_eq!(document["error"]["code"], "USAGE", "{document}");
        assert_eq!(document["error"]["field"], "--input");
        assert!(document["error"].get("x-canon").is_none(), "a shape fault has no reason: {document}");
        assert!(atip::envelope_problems(&document).is_empty());
    }
    assert_eq!(before, std::fs::read_to_string(repo.path().join(".canon/specs/kind=feature/area=auth/login.feature")).unwrap(), "nothing was written");
}

#[test]
fn atip_l0_30_the_framing_faults_of_input_carry_a_reason_and_a_document_fault_a_pointer() {
    let repo = scratch_repo();
    let missing = repo.path().join("no-such-file.json");
    let cases: Vec<(Vec<String>, &str)> = vec![
        (vec!["--input".into(), missing.display().to_string()], "unreadable"),
        (vec!["--input".into(), String::new()], "empty"),
        (vec!["--input".into(), "{ not json".into()], "syntax"),
    ];
    for (input, reason) in cases {
        let mut args: Vec<String> = vec!["scenario".to_string(), "new".to_string()];
        args.extend(input);
        args.push("--json".into());
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = canon(repo.path(), &refs);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        let document = json(&out);
        let error = &document["error"];
        assert_eq!(error["code"], "INPUT_INVALID", "{args:?}: {document}");
        assert_eq!(error["field"], "--input", "{args:?}");
        assert_eq!(error["x-canon"]["reason"], reason, "{args:?}: {document}");
        assert!(atip::error_problems(error).is_empty(), "{args:?}: {:?}", atip::error_problems(error));
    }
    // An array to a command that takes one object.
    let array = json(&canon(repo.path(), &["purge", "--input", r#"[{"repo":"."}]"#, "--json"]));
    assert_eq!(array["error"]["code"], "INPUT_INVALID", "{array}");
    assert_eq!(array["error"]["x-canon"]["reason"], "batch-unsupported", "{array}");
    // A document that breaks its fragment schema: a pointer, and no reason.
    let fragment = json(&canon(repo.path(), &["scenario", "new", "--input", r#"{"operands":["auth.login.51"],"title":"t","nosuch":1}"#, "--json"]));
    assert_eq!(fragment["error"]["code"], "INPUT_INVALID", "{fragment}");
    assert_eq!(fragment["error"]["field"], "/nosuch", "{fragment}");
    assert!(fragment["error"].get("x-canon").is_none(), "{fragment}");
}

#[test]
fn atip_l0_31_every_error_object_passes_the_contract_error_schema() {
    let repo = scratch_repo();
    let runs: Vec<Vec<&str>> = vec![
        vec!["status", "--bogus", "--json"],
        vec!["status", "--fields", "nosuch", "--json"],
        vec!["nosuch", "--json"],
        vec!["brief", "--json"],
        vec!["report", "nosuch", "--json"],
        vec!["--json"],
        vec!["scenario", "new", "--json"],
        vec!["scenario", "new", "--json", "--input"],
        vec!["scenario", "new", "--input", "{ nope", "--json"],
        vec!["query", "--kind", "subject", "--cursor", "zz", "--json"],
        vec!["subject", "show", "nosuch", "--json"],
        vec!["retrieve", "--for-role", "reviewer", "--json"],
    ];
    for args in &runs {
        let out = canon(repo.path(), args);
        let document = json(&out);
        assert_eq!(document["ok"], false, "{args:?}: {document}");
        let problems = atip::error_problems(&document["error"]);
        assert!(problems.is_empty(), "{args:?}: {problems:?}\n{document}");
        let mut found = Vec::new();
        voids(&document["error"], "error", &mut found);
        assert!(found.is_empty(), "{args:?}: {found:?}");
    }
    // The in-process error value, with the fields a tool sets.
    let error = CliError::refused("USAGE", "m").hint("").field("").received("");
    let body = serde_json::to_value(error.body_with(None, Vec::new())).unwrap();
    assert!(atip::error_problems(&body).is_empty(), "empty members are left out: {body}");
}
