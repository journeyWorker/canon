//! The `--json` contract (0.20 D2), against the built binary and over every
//! leaf command `canon schema` lists: each one accepts `--json`, writes
//! exactly one JSON value to stdout (a refusal counts), and a non-zero exit
//! carries a registered `error.code`.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

/// Leaf commands the sweep does not run, and why. A new leaf must either run
/// or be named here.
const SKIPPED: &[(&str, &str)] = &[("dashboard", "binds a port and serves until the process is stopped")];

const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

fn canon(dir: &Path, args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(dir).env("CANON_ACTOR", "json-contract").output().expect("spawn canon binary")
}

fn run(dir: &Path, args: &[&str]) -> Output {
    canon(dir, &args.iter().map(|a| a.to_string()).collect::<Vec<_>>())
}

fn ok(dir: &Path, args: &[&str]) {
    let out = run(dir, args);
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// A repo with a subject and a change, so the write commands have something
/// to act on and the read commands something to read.
fn scratch_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    ok(repo, &["init", "--providers", "none"]);
    ok(repo, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(repo, &["change", "new", "add-login", "--subject", "demo", "--title", "Add login"]);
    ok(repo, &["feature", "new", "auth.login", "--title", "Login"]);
    ok(repo, &["scenario", "new", "auth.login.01", "--title", "Sign in", "--feature", ".canon/specs/kind=feature/area=auth/login.feature"]);
    dir
}

/// The value a required argument gets: by command first, then by name.
fn value_for(command: &str, name: &str, regime: &str) -> String {
    let by_command: BTreeMap<(&str, &str), &str> = BTreeMap::from([
        (("evidence add", "kind"), "test-run"),
        (("export", "kind"), "evidence_record"),
        (("query", "kind"), "evidence_record"),
        (("purge", "kind"), "evidence_record"),
        (("subject adopt", "subject"), "demo"),
        (("change new", "subject"), "demo"),
        (("change new", "slug"), "another-change"),
        (("dispatch end", "status"), "succeeded"),
        (("subject new", "id"), "demo-two"),
        (("subject status", "id"), "demo"),
    ]);
    if let Some(value) = by_command.get(&(command, name)) {
        return value.to_string();
    }
    match name {
        "repo" => ".",
        "response" => "response.json",
        "by" | "approval-by" | "principal" | "actor" | "reviewer" => "alice",
        "note" | "summary" | "reason" | "title" | "name" => "text",
        "slug" | "change-id" | "change_id" => "add-login",
        "subject" => "demo",
        "domain" => "dev",
        "manifest" | "evaluation" | "signature-file" => "missing.json",
        "id" => "sha256:00",
        "role" => "reviewer",
        "regime" => regime,
        "run" => ULID,
        "status" => "ok",
        "project-id" => "app-a",
        "scenario-id" => "auth.login.01",
        "sha" | "artifact-sha" => SHA,
        "pin" => "abc123",
        "expiry" | "approved-at" | "approval-at" | "before" => "2030-01-01T00:00:00Z",
        "ref" => "npm test",
        "round" | "seq" => "1",
        "severity" => "should-fix",
        "version" => "v1",
        "disposition" => "open",
        "root" => ".canon/specs",
        "task_id" => "add-login#1",
        "strategy_id" => ULID,
        "plugin_id" => "no-such-plugin",
        "hash" => "abcdef012345",
        "area" => "area",
        "surface" => "auth.login",
        "tag" => "auth.login.02",
        _ => "x",
    }
    .to_string()
}

/// Flags that keep a command's invocation local to the scratch directory.
fn extra_args(command: &str, scratch: &Path) -> Vec<String> {
    let path = |p: &str| scratch.join(p).display().to_string();
    match command {
        "init" => vec!["--repo".into(), path("fresh"), "--providers".into(), "none".into()],
        "demo init" => vec!["--repo".into(), path("demo")],
        // `--role` is optional in the schema (a declared role supplies it), but the sweep declares none.
        "divergence accept" | "evidence add" | "review add" => vec!["--role".into(), "reviewer".into()],
        "ingest sessions" => vec!["--home".into(), path("home")],
        "skills install" | "skills check" | "skills doctor" => vec!["--target".into(), path("skills-target"), "--source".into(), path("no-skills-source")],
        _ => Vec::new(),
    }
}

#[test]
fn every_leaf_command_answers_json_with_one_document_and_a_coded_error() {
    let repo = scratch_repo();
    let listing = run(repo.path(), &["schema", "--json"]);
    let listing: Value = serde_json::from_slice(&listing.stdout).expect("schema lists the leaves");
    let leaves: Vec<String> = listing["data"]["commands"].as_array().unwrap().iter().map(|c| c["command"].as_str().unwrap().to_string()).collect();
    assert!(leaves.len() > 60, "{leaves:?}");

    let regime = String::from_utf8(run(repo.path(), &["regime-key", "--role", "reviewer", "--repo", "r", "--area", "a", "--hash", "abcdef012345"]).stdout).unwrap().trim().to_string();
    let registry: Vec<&str> = canon_cli::cli_error::all_codes().iter().map(|(code, _)| *code).collect();
    let skipped: Vec<&str> = SKIPPED.iter().map(|(command, _)| *command).collect();
    let mut problems = Vec::new();
    let mut usage = Vec::new();

    for leaf in &leaves {
        let mut schema_args = vec!["schema".to_string()];
        schema_args.extend(leaf.split(' ').map(str::to_string));
        schema_args.push("--json".to_string());
        let schema: Value = serde_json::from_slice(&canon(repo.path(), &schema_args).stdout).unwrap();
        let schema = schema["data"].clone();
        let has = |name: &str| schema["options"].as_array().unwrap().iter().any(|o| o["name"] == name);
        if !has("json") {
            problems.push(format!("{leaf}: does not accept --json"));
            continue;
        }
        if skipped.contains(&leaf.as_str()) {
            continue;
        }

        let mut args: Vec<String> = leaf.split(' ').map(str::to_string).collect();
        for positional in schema["arguments"].as_array().unwrap().iter().filter(|a| a["required"] == true) {
            args.push(value_for(leaf, positional["name"].as_str().unwrap(), &regime));
        }
        for option in schema["options"].as_array().unwrap().iter().filter(|o| o["required"] == true && o["name"] != "json") {
            let name = option["name"].as_str().unwrap();
            args.push(format!("--{name}"));
            if option["kind"] != "flag" {
                args.push(value_for(leaf, name, &regime));
            }
        }
        // One member of each required one-of group, the way an agent reading
        // `requiredOneOf` would build the line.
        for group in schema["requiredOneOf"].as_array().into_iter().flatten() {
            let flag = group[0].as_str().unwrap();
            if !args.iter().any(|a| a == flag) {
                args.push(flag.to_string());
                args.push(value_for(leaf, flag.trim_start_matches("--"), &regime));
            }
        }
        args.extend(extra_args(leaf, repo.path()));
        args.push("--json".to_string());

        let out = canon(repo.path(), &args);
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let body: Value = match serde_json::from_slice(&out.stdout) {
            Ok(body) => body,
            Err(e) => {
                problems.push(format!("{args:?}: stdout is not exactly one JSON value ({e}): {stdout:?}"));
                continue;
            }
        };
        if out.status.success() {
            continue;
        }
        match body["error"]["code"].as_str() {
            Some("USAGE") => usage.push(leaf.clone()),
            Some(code) if registry.contains(&code) => {}
            other => problems.push(format!("{args:?}: exit {:?} without a registered error.code ({other:?}): {stdout}", out.status.code())),
        }
    }

    assert!(problems.is_empty(), "{}", problems.join("\n"));
    // A usage error means the sweep built the wrong command line, not that the
    // command was exercised; none may hide behind it.
    assert!(usage.is_empty(), "the sweep's arguments are wrong for: {usage:?}");
}

#[test]
fn gate_check_reports_its_violations_as_json_and_keeps_its_exit_code() {
    let repo = scratch_repo();
    let out = run(repo.path(), &["gate", "check", "--json"]);
    let body: Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stdout)));
    assert_eq!(out.status.code(), Some(1), "{body}");
    assert_eq!(body["error"]["code"], "GATE_VIOLATIONS");
    assert_eq!(body["meta"]["command"], "gate check");
    let violations = body["data"]["violations"].as_array().expect("violations[]");
    assert!(!violations.is_empty(), "{body}");
    for violation in violations {
        assert!(violation["class"].is_string() && violation["subject"].is_string() && violation["message"].is_string(), "{violation}");
    }
    assert!(body["data"]["advisories"].is_array());
    assert_eq!(body["data"]["clean"], false);
    // The human form still prints the report on stdout and exits the same way.
    let human = run(repo.path(), &["gate", "check"]);
    assert_eq!(human.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&human.stdout).contains("violation(s)"));
}

#[test]
fn a_clean_gate_is_data_with_exit_zero() {
    let dir = TempDir::new().unwrap();
    ok(dir.path(), &["init", "--providers", "none"]);
    let out = run(dir.path(), &["gate", "check", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    let body: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(body["data"]["clean"], true);
    assert_eq!(body["data"]["violations"].as_array().unwrap().len(), 0);
    assert_eq!(body["meta"], serde_json::json!({ "command": "gate check" }));
}

#[test]
fn progress_and_refresh_notices_stay_off_stdout_in_json_mode() {
    let repo = scratch_repo();
    // `scenario new` and `inventory sync` both narrate in human mode.
    let out = run(repo.path(), &["scenario", "new", "auth.login.02", "--title", "Sign out", "--feature", ".canon/specs/kind=feature/area=auth/login.feature", "--json"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let body: Value = serde_json::from_slice(&out.stdout).expect("one document");
    assert_eq!(body["data"]["scenarioId"], "auth.login.02");
    assert!(String::from_utf8_lossy(&out.stderr).contains("wrote `@auth.login.02`"), "the human line moved to stderr");

    let out = run(repo.path(), &["inventory", "sync", "--json"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let body: Value = serde_json::from_slice(&out.stdout).expect("one document");
    assert_eq!(body["data"]["clean"], true);
    assert_eq!(body["meta"]["command"], "inventory sync");
}

#[test]
fn a_failing_format_check_is_an_error_with_the_violations_as_data() {
    let dir = TempDir::new().unwrap();
    ok(dir.path(), &["init", "--providers", "none"]);
    std::fs::create_dir_all(dir.path().join("corpus/kind=feature")).unwrap();
    std::fs::write(dir.path().join("corpus/kind=feature/bad.feature"), "Feature: no provenance\n  Scenario: s\n    Given x\n").unwrap();
    let out = run(dir.path(), &["format", "corpus", "--json"]);
    let body: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(1), "{body}");
    assert_eq!(body["error"]["code"], "FORMAT_VIOLATIONS");
    assert!(!body["data"]["violations"].as_array().unwrap().is_empty(), "{body}");
}
