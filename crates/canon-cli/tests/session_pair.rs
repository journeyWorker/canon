//! The attribution pair (0.28): every leaf that takes `--actor-id` and records an actor also takes
//! `--session-id`, and the session lands in the record as `actor.session_id`. Against the built
//! `canon` binary, reading what the leaves wrote.

use std::path::Path;
use std::process::Command;

use serde_json::Value;

fn canon(repo: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(repo).env_remove("CANON_ROLE").env_remove("CANON_ACTOR").output().expect("spawn canon")
}

fn ok(repo: &Path, args: &[&str]) -> Value {
    let out = canon(repo, args);
    assert!(out.status.success(), "canon {args:?}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).unwrap_or(Value::Null)
}

fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    ok(dir.path(), &["init", "--no-agents-md", "--policy", "light", "--providers", "none"]);
    dir
}

fn records(repo: &Path, kind: &str) -> Vec<Value> {
    ok(repo, &["query", "--kind", kind, "--history", "--all", "--json"])["data"]["records"].as_array().cloned().unwrap_or_default()
}

#[test]
fn subject_new_and_subject_move_record_the_session() {
    let dir = repo();
    let repo = dir.path();
    ok(repo, &["subject", "new", "pause", "--domain", "dev", "--title", "X", "--actor-id", "plan-1", "--session-id", "sess-new", "--json"]);
    ok(repo, &["subject", "move", "pause", "--to", "specced", "--reason", "scoped", "--actor-id", "plan-1", "--session-id", "sess-move", "--json"]);
    let all = records(repo, "subject");
    let moved = all.iter().find(|r| r["subject_id"] == "pause" && !r["status_reason"].is_null()).expect("the moved version");
    assert_eq!(moved["status_reason"]["actor"]["session_id"], "sess-move", "{moved}");
    let created = all.iter().find(|r| r["subject_id"] == "pause" && r["status_reason"].is_null()).expect("the created version");
    assert_eq!(created["actor"]["session_id"], "sess-new", "{created}");
}

#[test]
fn feature_new_and_scenario_new_stamp_the_session_in_their_provenance() {
    let dir = repo();
    let repo = dir.path();
    ok(repo, &["feature", "new", "cart.add", "--title", "Add", "--actor-id", "impl-1", "--session-id", "sess-f", "--json"]);
    ok(repo, &["scenario", "new", "cart.add.01", "--title", "Adds", "--case", "happy", "--actor-id", "impl-1", "--session-id", "sess-s", "--json"]);
    // The same pair through `--input`.
    ok(repo, &["scenario", "new", "--actor-id", "impl-1", "--session-id", "sess-i", "--input", r#"[{"operands":["cart.add.02"],"title":"Adds two"}]"#, "--json"]);
    let feature = std::fs::read_to_string(repo.join(".canon/specs/kind=feature/area=cart/add.feature")).unwrap();
    let sessions: Vec<String> = feature
        .lines()
        .filter_map(|l| l.trim().strip_prefix("# canon: "))
        .filter_map(|j| serde_json::from_str::<Value>(j).ok())
        .filter_map(|p| p["actor"]["session_id"].as_str().map(str::to_string))
        .collect();
    assert_eq!(sessions, ["sess-f", "sess-s", "sess-i"], "{feature}");
}

#[test]
fn dispatch_begin_records_the_session_on_the_run_actor() {
    let dir = repo();
    let repo = dir.path();
    let body = ok(repo, &["dispatch", "begin", "--role", "dev", "--regime", "dev/canon/join-spine/9c93d024b1a2", "--actor-id", "orch-1", "--session-id", "sess-d", "--json"]);
    let manifest = body["data"]["manifest"].as_str().expect("the manifest path");
    let run: Value = serde_json::from_str(&std::fs::read_to_string(repo.join(manifest)).unwrap()).unwrap();
    assert_eq!(run["actor"]["agent_id"], "orch-1", "{run}");
    assert_eq!(run["actor"]["session_id"], "sess-d", "{run}");
}

#[test]
fn divergence_add_defer_and_resolve_record_the_session() {
    let dir = repo();
    let repo = dir.path();
    for args in [&["init", "-q"][..], &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "--allow-empty", "-m", "start"][..]] {
        assert!(Command::new("git").args(args).current_dir(repo).status().unwrap().success());
    }
    ok(repo, &["scenario", "new", "game.run.01", "--title", "A run ends", "--case", "failure"]);
    ok(repo, &["divergence", "add", "game.run.01", "--project-id", "root", "--actor-id", "r", "--role", "reviewer", "--session-id", "sess-a", "--json"]);
    ok(repo, &["divergence", "defer", "game.run.01", "--project-id", "root", "--actor-id", "r", "--role", "reviewer", "--session-id", "sess-b", "--reason", "later", "--expiry", "2099-01-01T00:00:00Z", "--round", "2", "--json"]);
    ok(repo, &["divergence", "resolve", "game.run.01", "--project-id", "root", "--actor-id", "r", "--role", "reviewer", "--session-id", "sess-c", "--round", "3", "--json"]);
    let mut sessions: Vec<String> = records(repo, "divergence").iter().filter_map(|r| r["actor"]["session_id"].as_str().map(str::to_string)).collect();
    sessions.sort();
    assert_eq!(sessions, ["sess-a", "sess-b", "sess-c"]);
}

#[test]
fn an_option_the_leaf_lacks_is_a_usage_error_with_and_without_input() {
    let dir = repo();
    let repo = dir.path();
    // `strategy rollback` records no actor session: it has no `--session-id`.
    for args in [
        vec!["strategy", "rollback", "x", "--session-id", "s", "--json"],
        vec!["scenario", "new", "--actor-id", "a", "--bogus", "s", "--input", r#"[{"operands":["x.y.01"],"title":"T"}]"#, "--json"],
        vec!["scenario", "new", "x.y.01", "--title", "T", "--bogus", "s", "--json"],
    ] {
        let out = canon(repo, &args);
        let body: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {body}");
        assert_eq!(body["error"]["code"], "USAGE", "{args:?}: {body}");
    }
    let out = canon(repo, &["scenario", "new", "--actor-id", "a", "--bogus", "s", "--input", r#"[{"operands":["x.y.01"],"title":"T"}]"#, "--json"]);
    let body: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(body["error"]["field"], "--bogus", "the refused option is named, not turned into an operand conflict: {body}");
}
