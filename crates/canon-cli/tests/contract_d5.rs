//! 0.21 D5, against the built binary: `--fields` on `status`, `export`,
//! `divergence status` and `context`; `field`/`expected` on refusals; retry
//! safety of `evidence add` and `review add`; the exit-code exceptions.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

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

/// `unchanged: true` of an answer: a report carries it in `data`, a report that
/// is a stored record verbatim in `meta.x-canon`; never in `meta` itself.
fn unchanged(answer: &Value) -> Option<bool> {
    assert!(answer["meta"].get("unchanged").is_none(), "`unchanged` is not a meta member any more: {answer}");
    answer["data"].get("unchanged").or_else(|| answer["meta"]["x-canon"].get("unchanged")).map(|v| v.as_bool().unwrap_or_else(|| panic!("`unchanged` is true or absent: {answer}")))
}

/// A repo with subject `demo`, change `add-login` and scenario `auth.login.01`.
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let r = dir.path();
    ok(r, &["init", "--providers", "none"]);
    ok(r, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(r, &["change", "new", "add-login", "--subject", "demo", "--title", "Add login"]);
    ok(r, &["feature", "new", "auth.login", "--title", "Login"]);
    ok(r, &["scenario", "new", "auth.login.01", "--title", "Sign in", "--feature", ".canon/specs/kind=feature/area=auth/login.feature"]);
    ok(r, &["inventory", "sync"]);
    // Revision-binding commands need a work tree with a commit.
    git(r, &["init", "-q", "-b", "main"]);
    git(r, &["config", "user.name", "t"]);
    git(r, &["config", "user.email", "t@example.com"]);
    git(r, &["add", "-A"]);
    git(r, &["commit", "-q", "-m", "x"]);
    dir
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().expect("git must be on PATH for this test");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn keys(value: &Value) -> Vec<String> {
    value.as_object().unwrap().keys().cloned().collect()
}

/// Record versions written for `kind`: the ledger's and the archive's.
/// These tests ask whether a write happened, so they count versions.
fn files_under(dir: &Path, kind: &str) -> usize {
    fn count(path: &Path) -> usize {
        std::fs::read_dir(path).map_or(0, |entries| entries.flatten().map(|e| if e.path().is_dir() { count(&e.path()) } else { 1 }).sum())
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
    assert_eq!(error["code"], "USAGE");
    assert_eq!(error["field"], "--fields");
    assert!(error["expected"].as_array().unwrap().iter().any(|f| f == "subjects"), "{error}");
}

#[test]
fn context_keeps_only_the_named_sections() {
    // A directory with no canon.yaml: `context` needs none.
    let dir = TempDir::new().unwrap();
    assert!(!dir.path().join("canon.yaml").exists());
    let out = run(dir.path(), &["context", "--json", "--fields", "kinds,enums"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let mut got = keys(&body(&out)["data"]);
    got.sort();
    assert_eq!(got, ["enums", "kinds"]);

    let out = run(dir.path(), &["context", "--json", "--fields", "nope"]);
    assert_eq!((out.status.code(), body(&out)["error"]["code"].clone()), (Some(2), Value::from("USAGE")));
    assert_eq!(body(&out)["error"]["field"], "--fields", "the refusal names the option");

    // Without --json the refusal is the same exit code.
    let out = run(dir.path(), &["context", "--fields", "nope"]);
    assert_eq!(out.status.code(), Some(2), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(out.stdout.is_empty(), "a refusal prints no report");
}

#[test]
fn divergence_status_projects_each_scenario_row() {
    let dir = repo();
    ok(dir.path(), &["divergence", "add", "auth.login.01", "--project", "root", "--actor", "alice", "--role", "reviewer"]);
    let all = body(&run(dir.path(), &["divergence", "list", "--json"]));
    assert!(all["data"]["scenarios"][0].as_object().unwrap().len() >= 3, "{all}");
    let out = run(dir.path(), &["divergence", "list", "--json", "--fields", "scenarioId,state"]);
    let shown = body(&out);
    assert_eq!(shown["data"]["scenarios"], serde_json::json!([{ "scenarioId": "auth.login.01", "state": "open" }]));
    assert_eq!(shown["data"]["summary"], all["data"]["summary"], "the summary is not a row and is kept");
    let out = run(dir.path(), &["divergence", "list", "--json", "--fields", "nope"]);
    assert_eq!(body(&out)["error"]["code"], "USAGE");
    assert_eq!(body(&out)["error"]["field"], "--fields");
}

#[test]
fn export_projects_each_record_but_keeps_the_manifest_digests() {
    let dir = repo();
    ok(dir.path(), &["evidence", "add", "--project", "root", "--scenario", "auth.login.01", "--class", "test-run", "--ref", "npm test", "--role", "implementer"]);
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
    assert_eq!(body(&out)["error"]["code"], "USAGE");
    assert_eq!(body(&out)["error"]["field"], "--fields");
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

    let e = error_of(dir.path(), &["finding", "add", "--change", "add-login", "--round", "1", "--seq", "1", "--severity", "nope", "--actor", "a", "--summary", "s"]);
    assert_eq!(e["field"], "--severity");
    assert_eq!(strings(&e["expected"]), ["blocker", "should-fix", "note"]);

    let e = error_of(dir.path(), &["finding", "close", "add-login", "--round", "1", "--seq", "1", "--disposition", "nope"]);
    assert_eq!(e["field"], "--disposition");
    assert_eq!(strings(&e["expected"]), ["open", "fixed", "rejected", "deferred"]);

    let e = error_of(dir.path(), &["change", "new", "x", "--subject", "demo", "--title", "t", "--change-kind", "nope"]);
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

    let e = error_of(dir.path(), &["finding", "close", "add-login", "--round", "1", "--seq", "1", "--disposition", "rejected", "--reason", "by design", "--resolution-rev", "HEAD"]);
    assert_eq!((e["code"].as_str(), e["field"].as_str()), (Some("FINDING_RESOLUTION_REV_UNEXPECTED"), Some("--resolution-rev")));

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
    let mut args = vec!["evidence", "add", "--project", "root", "--scenario", "auth.login.01", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--json"];
    args.extend_from_slice(extra);
    run(dir, &args)
}

#[test]
fn repeating_evidence_add_returns_the_existing_record_and_writes_nothing() {
    let dir = repo();
    let first = evidence(dir.path(), &[]);
    assert!(first.status.success());
    assert_eq!(unchanged(&body(&first)), None, "the first call writes");
    assert_eq!(files_under(dir.path(), "evidence"), 1);

    let again = evidence(dir.path(), &[]);
    assert!(again.status.success(), "{}", String::from_utf8_lossy(&again.stderr));
    let again = body(&again);
    assert_eq!(unchanged(&again), Some(true), "{again}");
    assert_eq!(again["data"]["records"][0]["committed"], body(&first)["data"]["records"][0]["committed"], "the existing record is the one returned");
    assert_eq!(files_under(dir.path(), "evidence"), 1, "a repeat writes no second record");
}

#[test]
fn evidence_that_differs_is_a_new_record_and_only_the_latest_one_counts_as_the_repeat() {
    let dir = repo();
    assert!(evidence(dir.path(), &[]).status.success());
    // A different verdict is a new fact.
    assert!(evidence(dir.path(), &["--verdict", "divergent"]).status.success());
    assert_eq!(files_under(dir.path(), "evidence"), 2);
    // Faithful again is a change from the latest (divergent), so it is written too...
    let third = evidence(dir.path(), &[]);
    assert_eq!(unchanged(&body(&third)), None);
    assert_eq!(files_under(dir.path(), "evidence"), 3);
    // ...and only now is a repeat of the latest.
    assert_eq!(unchanged(&body(&evidence(dir.path(), &[]))), Some(true));
    assert_eq!(files_under(dir.path(), "evidence"), 3);
    // A different summary is a different record.
    assert_eq!(unchanged(&body(&evidence(dir.path(), &["--summary", "ran the suite"]))), None);
    assert_eq!(files_under(dir.path(), "evidence"), 4);
}

#[test]
fn evidence_added_again_after_a_review_is_deliberate_and_is_written() {
    // A review pins the evidence it saw; attesting again afterwards supersedes
    // that evidence (and stales the review), so it is never read as a retry.
    let dir = repo();
    assert!(evidence(dir.path(), &[]).status.success());
    ok(dir.path(), &["review", "scenario", "auth.login.01", "--project", "root", "--actor", "alice", "--role", "reviewer", "--original-spec-ref", "docs/prd.md"]);
    let again = evidence(dir.path(), &[]);
    assert!(again.status.success());
    assert_eq!(unchanged(&body(&again)), None, "{}", String::from_utf8_lossy(&again.stdout));
    assert_eq!(files_under(dir.path(), "evidence"), 2);
    // Nothing followed that one: repeating it is a retry.
    assert_eq!(unchanged(&body(&evidence(dir.path(), &[]))), Some(true));
    assert_eq!(files_under(dir.path(), "evidence"), 2);
}

fn review(dir: &Path, rev: &str) -> Output {
    run(dir, &["review", "scenario", "auth.login.01", "--project", "root", "--actor", "alice", "--rev", rev, "--role", "reviewer", "--original-spec-ref", "docs/prd.md", "--json"])
}

#[test]
fn repeating_review_add_returns_the_existing_review_and_writes_nothing() {
    let dir = repo();
    let first = review(dir.path(), "HEAD");
    assert!(first.status.success(), "{}", String::from_utf8_lossy(&first.stderr));
    assert_eq!(files_under(dir.path(), "review"), 1);

    let again = body(&review(dir.path(), "HEAD"));
    assert_eq!(unchanged(&again), Some(true), "{again}");
    assert_eq!(again["data"]["reviews"][0]["location"], body(&first)["data"]["reviews"][0]["location"]);
    assert_eq!(files_under(dir.path(), "review"), 1);

    // A different commit is a new review.
    git(dir.path(), &["commit", "-q", "--allow-empty", "-m", "next"]);
    let other = body(&review(dir.path(), "HEAD"));
    assert_eq!(unchanged(&other), None, "{other}");
    assert_eq!(files_under(dir.path(), "review"), 2);
}

// ---- 4b. every replay-safe or content-keyed leaf: an identical repeat is unchanged -----------

/// Every file of the repo with its bytes, except git's own, the hot store's
/// SQLite files and the access-audit log (a run that read records
/// appends to it by design, and says so).
fn tree(dir: &Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            // the hot store's SQLite file is page-level state (a checkpoint may rewrite it), never records
            if name == ".git" || name == "hot.db" || name.ends_with("-shm") || name.ends_with("-wal") || path.ends_with(".canon/audit") {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(path.strip_prefix(root).unwrap().to_path_buf(), std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// Runs `args` (plus `--json`) twice: both exit 0, the second says
/// `unchanged: true` and leaves the tree as the first left it.
fn repeats_unchanged(dir: &Path, args: &[&str]) {
    let mut argv = args.to_vec();
    argv.push("--json");
    let first = run(dir, &argv);
    assert_eq!(first.status.code(), Some(0), "{args:?}: {}{}", String::from_utf8_lossy(&first.stdout), String::from_utf8_lossy(&first.stderr));
    let after_first = tree(dir);
    let again = run(dir, &argv);
    assert_eq!(again.status.code(), Some(0), "{args:?} again: {}{}", String::from_utf8_lossy(&again.stdout), String::from_utf8_lossy(&again.stderr));
    assert_eq!(unchanged(&body(&again)), Some(true), "{args:?}: {}", String::from_utf8_lossy(&again.stdout));
    let after_again = tree(dir);
    let differing: Vec<&std::path::PathBuf> = after_first.keys().chain(after_again.keys()).filter(|k| after_first.get(*k) != after_again.get(*k)).collect();
    assert!(differing.is_empty(), "{args:?}: the repeat changed the tree: {differing:?}");
}

/// A destructive leaf is previewed, then committed with the preview's digest.
fn commit_repeats_unchanged(dir: &Path, args: &[&str]) {
    let digest = |dir: &Path| {
        let mut preview = args.to_vec();
        preview.push("--json");
        body(&run(dir, &preview))["data"]["planDigest"].as_str().unwrap().to_string()
    };
    let first = digest(dir);
    let mut commit = args.to_vec();
    commit.extend(["--commit", "--plan-digest", first.as_str()]);
    ok(dir, &commit);
    let second = digest(dir);
    let mut commit = args.to_vec();
    commit.extend(["--commit", "--plan-digest", second.as_str()]);
    repeats_unchanged(dir, &commit);
}

#[test]
fn an_identical_repeat_of_a_write_leaf_is_unchanged_and_writes_nothing() {
    let dir = repo();
    let home = TempDir::new().unwrap();
    let d = dir.path();
    std::fs::write(d.join("README.md"), "hello\n").unwrap();
    std::fs::write(
        d.join(".canon/context-manifest.json"),
        r#"{"manifestVersion":1,"capabilityVersion":1,"selectedDocs":["README.md"],"examples":[],"toolSchemas":[],"injectedGuidance":[]}"#,
    )
    .unwrap();
    std::fs::write(
        d.join(".canon/prompt-manifest.json"),
        r#"{"manifestVersion":1,"capabilityVersion":1,"selectedDocs":[],"prompt":"README.md","examples":[],"toolSchemas":[],"injectedGuidance":[]}"#,
    )
    .unwrap();
    let home_dir = home.path().to_str().unwrap();
    for args in [
        &["inventory", "sync"][..],
        &["init", "--providers", "none"],
        &["gate", "install-hooks"],
        &["gate", "promote"],
        &["report", "build"],
        &["report", "snapshot", "snapshot-out"],
        &["subject", "adopt", "demo", "--change", "add-login"],
        &["artifacts", "store"],
        &["policy", "approve", "--approver", "alice", "--note", "reviewed"],
        &["context-pack", "add", "--manifest", ".canon/context-manifest.json"],
        &["prompt", "add", "reviewer@v1", "--manifest", ".canon/prompt-manifest.json"],
        &["divergence", "add", "auth.login.01", "--actor", "alice", "--role", "reviewer"],
        &["export", "--kind", "evidence", "--output", "evidence.json"],
        &["ingest", "records"],
        &["ingest", "plans"],
        &["ingest", "sessions", "--home", home_dir],
    ] {
        repeats_unchanged(d, args);
    }
    // Unlinking comes last: it takes the adoption the cases above rely on away.
    repeats_unchanged(d, &["subject", "unadopt", "demo", "--change", "add-login"]);
    for args in [&["ledger", "compact"][..], &["migrate"], &["tier", "age"], &["purge", "--kind", "session", "--before", "2020-01-01T00:00:00Z"], &["artifacts", "move", "--tier", "local"]] {
        commit_repeats_unchanged(d, args);
    }
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

/// A leaf that never gates, and one that cannot exit 1 for a finding, say so in
/// their schema: the exit rows are the leaf's own, not a shared default. Exit 1
/// is declared for every leaf (a panic is `INTERNAL`, exit 1); on these two it is
/// the only thing that can answer it.
#[test]
fn the_exceptions_declare_only_the_exits_they_have() {
    let dir = TempDir::new().unwrap();
    for (command, exits) in [(vec!["context"], vec![0, 1, 2]), (vec!["dispatch", "diff"], vec![0, 1, 2]), (vec!["artifacts", "store"], vec![0, 1, 2]), (vec!["gate", "check"], vec![0, 1, 2])] {
        let mut args = vec!["schema"];
        args.extend(&command);
        args.push("--json");
        let schema = body(&run(dir.path(), &args))["data"].clone();
        let rows = schema["exits"].as_array().unwrap();
        let declared: Vec<u64> = rows.iter().map(|e| e["code"].as_u64().unwrap()).collect();
        assert_eq!(declared, exits, "{command:?}");
        if matches!(command[0], "context" | "dispatch") {
            let one = rows.iter().find(|e| e["code"] == 1).unwrap()["meaning"].as_str().unwrap();
            assert_eq!(one, "failed: INTERNAL", "{command:?} cannot gate: exit 1 is only a panic");
        }
    }
}
