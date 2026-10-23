//! 0.21 D5, against the built binary: `--fields` on `status`, `export`,
//! `divergence status` and `context`; `field`/`expected` on refusals; retry
//! safety of `evidence add` and `review add`; the exit-code exceptions.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(dir).env("CANON_ACTOR", "contract-d5").output().expect("spawn canon binary")
}

fn ok(dir: &Path, args: &[&str]) {
    let out = run(dir, args);
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn body(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is not one JSON document ({e}): {}", String::from_utf8_lossy(&out.stdout)))
}

/// A repo with subject `demo`, change `add-login` and scenario `auth.login.01`.
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let r = dir.path();
    ok(r, &["init", "--providers", "none"]);
    ok(r, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(r, &["change", "new", "add-login", "--subject-id", "demo", "--title", "Add login"]);
    ok(r, &["feature", "new", "auth.login", "--title", "Login"]);
    ok(r, &["scenario", "new", "auth.login.01", "--title", "Sign in", "--feature", ".canon/specs/kind=feature/area=auth/login.feature"]);
    ok(r, &["inventory", "sync"]);
    dir
}

fn keys(value: &Value) -> Vec<String> {
    value.as_object().unwrap().keys().cloned().collect()
}

/// Record versions written for `kind`: the ledger's and the archive's.
/// These tests ask whether a write happened, so they count versions.
fn files_under(dir: &Path, kind: &str) -> usize {
    fn count(path: &Path) -> usize {
        std::fs::read_dir(path).map(|entries| entries.flatten().map(|e| if e.path().is_dir() { count(&e.path()) } else { 1 }).sum()).unwrap_or(0)
    }
    [".canon/ledger", ".canon/archive"].iter().map(|root| count(&dir.join(root).join(format!("kind={kind}")))).sum()
}

// ---- 1. --fields ---------------------------------------------------------

#[test]
fn status_keeps_only_the_named_fields_and_refuses_an_unknown_one() {
    let dir = repo();
    let out = run(dir.path(), &["status", "--json", "--fields", "subjects,policy"]);
    assert!(out.status.success());
    let mut got = keys(&body(&out)["data"]);
    got.sort();
    assert_eq!(got, ["policy", "subjects"]);

    let out = run(dir.path(), &["status", "--json", "--fields", "subjects,nope"]);
    assert_eq!(out.status.code(), Some(2));
    let error = &body(&out)["error"];
    assert_eq!(error["code"], "UNKNOWN_FIELD");
    assert_eq!(error["field"], "--fields");
    assert!(error["expected"].as_array().unwrap().iter().any(|f| f == "subjects"), "{error}");
}

#[test]
fn context_keeps_only_the_named_sections() {
    let dir = repo();
    let out = run(dir.path(), &["context", "--json", "--fields", "kinds,enums"]);
    assert!(out.status.success());
    let mut got = keys(&body(&out)["data"]);
    got.sort();
    assert_eq!(got, ["enums", "kinds"]);
    let out = run(dir.path(), &["context", "--json", "--fields", "nope"]);
    assert_eq!((out.status.code(), body(&out)["error"]["code"].clone()), (Some(2), Value::from("UNKNOWN_FIELD")));
}

#[test]
fn divergence_status_projects_each_scenario_row() {
    let dir = repo();
    ok(dir.path(), &["divergence", "add", "auth.login.01", "--project-id", "root", "--sha", SHA, "--actor-id", "alice", "--role", "reviewer"]);
    let all = body(&run(dir.path(), &["divergence", "list", "--json"]));
    assert!(all["data"]["scenarios"][0].as_object().unwrap().len() >= 3, "{all}");
    let out = run(dir.path(), &["divergence", "list", "--json", "--fields", "scenarioId,state"]);
    let shown = body(&out);
    assert_eq!(shown["data"]["scenarios"], serde_json::json!([{ "scenarioId": "auth.login.01", "state": "open" }]));
    assert_eq!(shown["data"]["summary"], all["data"]["summary"], "the summary is not a row and is kept");
    let out = run(dir.path(), &["divergence", "list", "--json", "--fields", "nope"]);
    assert_eq!(body(&out)["error"]["code"], "UNKNOWN_FIELD");
}

#[test]
fn export_projects_each_record_but_keeps_the_manifest_digests() {
    let dir = repo();
    ok(dir.path(), &["evidence", "add", "--project-id", "root", "--scenario-id", "auth.login.01", "--class", "test-run", "--ref", "npm test", "--role", "implementer"]);
    let all = body(&run(dir.path(), &["export", "--kind", "evidence"]));
    assert_eq!(all["records"].as_array().unwrap().len(), 1);
    let out = run(dir.path(), &["export", "--kind", "evidence", "--fields", "id,digest"]);
    let shown = body(&out);
    let mut record_keys = keys(&shown["records"][0]);
    record_keys.sort();
    assert_eq!(record_keys, ["digest", "id"]);
    assert_eq!(shown["contentDigests"], all["contentDigests"], "the manifest's digests do not depend on the projection");
    let out = run(dir.path(), &["export", "--kind", "evidence", "--fields", "nope", "--json"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(body(&out)["error"]["code"], "UNKNOWN_FIELD");
}

// ---- 2. field and expected on errors --------------------------------------

fn error_of(dir: &Path, args: &[&str]) -> Value {
    let mut argv = args.to_vec();
    argv.push("--json");
    let out = run(dir, &argv);
    assert_eq!(out.status.code(), Some(2), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    body(&out)["error"].clone()
}

fn strings(value: &Value) -> Vec<&str> {
    value.as_array().unwrap_or_else(|| panic!("`expected` is not a list: {value}")).iter().map(|v| v.as_str().unwrap()).collect()
}

#[test]
fn usage_errors_name_the_argument_and_its_possible_values() {
    let dir = repo();
    let e = error_of(dir.path(), &["query", "--kind", "nope"]);
    assert_eq!((e["code"].as_str(), e["field"].as_str()), (Some("USAGE"), Some("--kind")));
    assert!(strings(&e["expected"]).contains(&"evidence"));

    let e = error_of(dir.path(), &["finding", "add", "--change-id", "add-login", "--round", "1", "--seq", "1", "--severity", "nope", "--actor-id", "a", "--summary", "s"]);
    assert_eq!(e["field"], "--severity");
    assert_eq!(strings(&e["expected"]), ["blocker", "should-fix", "note"]);

    let e = error_of(dir.path(), &["finding", "close", "add-login", "--round", "1", "--seq", "1", "--disposition", "nope"]);
    assert_eq!(e["field"], "--disposition");
    assert_eq!(strings(&e["expected"]), ["open", "fixed", "rejected", "deferred"]);

    let e = error_of(dir.path(), &["change", "new", "x", "--subject-id", "demo", "--title", "t", "--change-kind", "nope"]);
    assert_eq!(e["field"], "--change-kind");
    assert_eq!(strings(&e["expected"]), ["feature", "chore"]);

    let e = error_of(dir.path(), &["evidence", "add", "--class", "k", "--ref", "r", "--role", "implementer", "--verdict", "nope"]);
    assert_eq!(e["field"], "--verdict");
    assert_eq!(strings(&e["expected"]), ["faithful", "not-applicable", "divergent"]);

    // An unknown flag names itself and offers the flags the command does take.
    let e = error_of(dir.path(), &["status", "--bogus"]);
    assert_eq!((e["code"].as_str(), e["field"].as_str(), e["received"].as_str()), (Some("USAGE"), Some("--bogus"), Some("--bogus")));
    assert!(strings(&e["expected"]).contains(&"--json"), "{e}");
}

#[test]
fn a_subject_status_off_the_chain_lists_the_states_it_may_move_to() {
    let dir = repo();
    // The state argument is a closed set clap knows.
    let e = error_of(dir.path(), &["subject", "move", "demo", "--to", "nope"]);
    assert_eq!(strings(&e["expected"]), ["proposed", "specced", "building", "verifying", "shipped", "retired"]);
    // A known state that is not the next step names the next steps.
    let e = error_of(dir.path(), &["subject", "move", "demo", "--to", "shipped"]);
    assert_eq!(e["code"], "SUBJECT_INVALID_TRANSITION");
    assert_eq!(strings(&e["expected"]), ["specced", "retired"]);
}

#[test]
fn input_validation_refusals_name_the_field_and_the_accepted_values() {
    let dir = repo();
    let e = error_of(dir.path(), &["query", "--kind", "subject", "--status", "nope"]);
    assert_eq!((e["code"].as_str(), e["field"].as_str()), (Some("QUERY_INVALID_ARGUMENT"), Some("--status")));
    assert!(strings(&e["expected"]).contains(&"proposed"), "{e}");

    let e = error_of(dir.path(), &["query", "--kind", "evidence", "--domain", "dev"]);
    assert_eq!((e["code"].as_str(), e["field"].as_str()), (Some("QUERY_INVALID_ARGUMENT"), Some("--domain")));

    let e = error_of(dir.path(), &["finding", "close", "add-login", "--round", "1", "--seq", "1", "--disposition", "fixed"]);
    assert_eq!((e["code"].as_str(), e["field"].as_str()), (Some("FINDING_RESOLUTION_SHA_REQUIRED"), Some("--resolution-sha")));

    let regime = String::from_utf8(run(dir.path(), &["regime", "show", "--role-segment", "implementer", "--repo-name", "r", "--area", "a", "--hash", "abcdef012345"]).stdout).unwrap().trim().to_string();
    let e = error_of(dir.path(), &["dispatch", "begin", "--role", "reviewer", "--regime", &regime]);
    assert_eq!((e["code"].as_str(), e["field"].as_str()), (Some("DISPATCH_ROLE_REGIME_MISMATCH"), Some("--role")));
    assert_eq!(strings(&e["expected"]), ["implementer"]);
}

#[test]
fn a_refusal_about_no_particular_argument_carries_neither_key() {
    let dir = repo();
    let e = error_of(dir.path(), &["subject", "move", "ghost", "--to", "specced"]);
    assert_eq!(e["code"], "SUBJECT_NOT_FOUND");
    assert!(e.get("field").is_none() && e.get("expected").is_none(), "{e}");
}

// ---- 4. retry safety -------------------------------------------------------

fn evidence(dir: &Path, extra: &[&str]) -> Output {
    let mut args = vec!["evidence", "add", "--project-id", "root", "--scenario-id", "auth.login.01", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--json"];
    args.extend_from_slice(extra);
    run(dir, &args)
}

#[test]
fn repeating_evidence_add_returns_the_existing_record_and_writes_nothing() {
    let dir = repo();
    let first = evidence(dir.path(), &[]);
    assert!(first.status.success());
    assert!(body(&first)["meta"].get("unchanged").is_none(), "the first call writes");
    assert_eq!(files_under(dir.path(), "evidence_record"), 1);

    let again = evidence(dir.path(), &[]);
    assert!(again.status.success(), "{}", String::from_utf8_lossy(&again.stderr));
    let again = body(&again);
    assert_eq!(again["meta"]["unchanged"], true, "{again}");
    assert_eq!(again["data"]["records"][0]["committed"], body(&first)["data"]["records"][0]["committed"], "the existing record is the one returned");
    assert_eq!(files_under(dir.path(), "evidence_record"), 1, "a repeat writes no second record");
}

#[test]
fn evidence_that_differs_is_a_new_record_and_only_the_latest_one_counts_as_the_repeat() {
    let dir = repo();
    assert!(evidence(dir.path(), &[]).status.success());
    // A different verdict is a new fact.
    assert!(evidence(dir.path(), &["--verdict", "divergent"]).status.success());
    assert_eq!(files_under(dir.path(), "evidence_record"), 2);
    // Faithful again is a change from the latest (divergent), so it is written too...
    let third = evidence(dir.path(), &[]);
    assert!(body(&third)["meta"].get("unchanged").is_none());
    assert_eq!(files_under(dir.path(), "evidence_record"), 3);
    // ...and only now is a repeat of the latest.
    assert_eq!(body(&evidence(dir.path(), &[]))["meta"]["unchanged"], true);
    assert_eq!(files_under(dir.path(), "evidence_record"), 3);
    // A different summary is a different record.
    assert!(body(&evidence(dir.path(), &["--summary", "ran the suite"]))["meta"].get("unchanged").is_none());
    assert_eq!(files_under(dir.path(), "evidence_record"), 4);
}

#[test]
fn evidence_added_again_after_a_review_is_deliberate_and_is_written() {
    // A review pins the evidence it saw; attesting again afterwards supersedes
    // that evidence (and stales the review), so it is never read as a retry.
    let dir = repo();
    assert!(evidence(dir.path(), &[]).status.success());
    ok(dir.path(), &["review", "scenario", "auth.login.01", "--project-id", "root", "--actor-id", "alice", "--pin", "abc123", "--role", "reviewer", "--original-spec-ref", "docs/prd.md"]);
    let again = evidence(dir.path(), &[]);
    assert!(again.status.success());
    assert!(body(&again)["meta"].get("unchanged").is_none(), "{}", String::from_utf8_lossy(&again.stdout));
    assert_eq!(files_under(dir.path(), "evidence_record"), 2);
    // Nothing followed that one: repeating it is a retry.
    assert_eq!(body(&evidence(dir.path(), &[]))["meta"]["unchanged"], true);
    assert_eq!(files_under(dir.path(), "evidence_record"), 2);
}

fn review(dir: &Path, pin: &str) -> Output {
    run(dir, &["review", "scenario", "auth.login.01", "--project-id", "root", "--actor-id", "alice", "--pin", pin, "--role", "reviewer", "--original-spec-ref", "docs/prd.md", "--json"])
}

#[test]
fn repeating_review_add_returns_the_existing_review_and_writes_nothing() {
    let dir = repo();
    let first = review(dir.path(), "abc123");
    assert!(first.status.success(), "{}", String::from_utf8_lossy(&first.stderr));
    assert_eq!(files_under(dir.path(), "review"), 1);

    let again = body(&review(dir.path(), "abc123"));
    assert_eq!(again["meta"]["unchanged"], true, "{again}");
    assert_eq!(again["data"]["reviews"][0]["location"], body(&first)["data"]["reviews"][0]["location"]);
    assert_eq!(files_under(dir.path(), "review"), 1);

    // A new pin is a new review.
    let other = body(&review(dir.path(), "def456"));
    assert!(other["meta"].get("unchanged").is_none(), "{other}");
    assert_eq!(files_under(dir.path(), "review"), 2);
}

// ---- 5. exit-code exceptions -----------------------------------------------

#[test]
fn context_exits_zero_even_when_canon_yaml_is_unreadable_and_says_so() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("canon.yaml"), "tiers: [broken\n").unwrap();
    let out = run(dir.path(), &["context", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(body(&out)["data"]["policy"].is_object(), "the surface is still described");
}

#[test]
fn evidence_vault_exits_zero_and_lists_the_files_it_could_not_store() {
    let dir = repo();
    std::fs::write(dir.path().join("trace.log"), "ran\n").unwrap();
    assert!(evidence(dir.path(), &["--artifact", "trace.log"]).status.success());
    // The bytes are gone from the working tree and from the store.
    std::fs::remove_file(dir.path().join("trace.log")).unwrap();
    let store = dir.path().join(".canon/artifacts");
    if store.exists() {
        std::fs::remove_dir_all(&store).unwrap();
    }
    let out = run(dir.path(), &["artifacts", "store", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(body(&out)["data"]["notStored"].as_array().unwrap().len(), 1);
}

#[test]
fn dispatch_diff_exits_zero_whatever_it_finds() {
    let dir = repo();
    let out = run(dir.path(), &["dispatch", "diff", "--json"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(body(&out)["data"]["counts"].is_object());
}

/// A leaf that never gates, and one that cannot exit 1, say so in their schema:
/// the exit rows are the leaf's own, not a shared default.
#[test]
fn the_exceptions_declare_only_the_exits_they_have() {
    let dir = TempDir::new().unwrap();
    for (command, exits) in [(vec!["context"], vec![0, 2]), (vec!["dispatch", "diff"], vec![0, 2]), (vec!["artifacts", "store"], vec![0, 2]), (vec!["gate", "check"], vec![0, 1, 2])] {
        let mut args = vec!["schema"];
        args.extend(&command);
        args.push("--json");
        let schema = body(&run(dir.path(), &args))["data"].clone();
        let declared: Vec<u64> = schema["exits"].as_array().unwrap().iter().map(|e| e["code"].as_u64().unwrap()).collect();
        assert_eq!(declared, exits, "{command:?}");
    }
}
