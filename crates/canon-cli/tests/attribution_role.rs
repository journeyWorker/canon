//! Attribution defaults to the declared role (0.28 D12): a record written by a
//! leaf with no `--role` option carries the role declared with `--as` /
//! `CANON_ROLE`; with none declared it carries what it carried before. Against
//! the built `canon` binary, reading what the leaves wrote.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

fn canon(repo: &Path, args: &[&str], role_flag: Option<&str>, role_env: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_canon"));
    command.args(args).current_dir(repo).env_remove("CANON_ROLE").env_remove("CANON_ACTOR");
    if let Some(role) = role_flag {
        command.args(["--as", role]);
    }
    if let Some(role) = role_env {
        command.env("CANON_ROLE", role);
    }
    command.output().expect("spawn canon")
}

fn ok(repo: &Path, args: &[&str], role_flag: Option<&str>, role_env: Option<&str>) -> Output {
    let out = canon(repo, args, role_flag, role_env);
    assert!(out.status.success(), "canon {args:?}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    out
}

fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    ok(dir.path(), &["init", "--no-agents-md", "--policy", "light", "--providers", "none"], None, None);
    dir
}

/// The latest `subject` record of `id`.
fn subject(repo: &Path, id: &str) -> Value {
    let out = ok(repo, &["query", "--kind", "subject", "--json"], None, None);
    let body: Value = serde_json::from_slice(&out.stdout).unwrap();
    let records = body["data"]["records"].as_array().unwrap().clone();
    records.into_iter().filter(|r| r["subject_id"] == id).max_by_key(|r| r["at"].as_str().unwrap_or_default().to_string()).unwrap_or_else(|| panic!("no subject `{id}`: {body}"))
}

fn role_of(actor: &Value) -> Option<&str> {
    actor["role"].as_str()
}

#[test]
fn subject_new_is_attributed_to_the_declared_role_and_owned_by_the_owner_role() {
    let dir = repo();
    let repo = dir.path();
    ok(repo, &["subject", "new", "pause-menu", "--domain", "dev", "--title", "X", "--actor", "planner-1", "--json"], Some("planner"), None);
    let planned = subject(repo, "pause-menu");
    assert_eq!(role_of(&planned["actor"]), Some("planner"), "{planned}");
    assert_eq!(planned["actor"]["agent_id"], "planner-1");
    assert_eq!(planned["owner_role"], "implementer", "the owner is another field: {planned}");

    ok(repo, &["subject", "new", "by-env", "--domain", "dev", "--title", "X", "--json"], None, Some("reviewer"));
    assert_eq!(role_of(&subject(repo, "by-env")["actor"]), Some("reviewer"));

    // The flag wins over the variable.
    ok(repo, &["subject", "new", "by-flag", "--domain", "dev", "--title", "X", "--json"], Some("orchestrator"), Some("reviewer"));
    assert_eq!(role_of(&subject(repo, "by-flag")["actor"]), Some("orchestrator"));
}

#[test]
fn without_a_declared_role_a_subject_is_attributed_as_before() {
    let dir = repo();
    let repo = dir.path();
    ok(repo, &["subject", "new", "plain", "--domain", "dev", "--title", "X", "--json"], None, None);
    let plain = subject(repo, "plain");
    assert_eq!(role_of(&plain["actor"]), Some("implementer"), "the owner role is the default: {plain}");
    ok(repo, &["subject", "new", "owned", "--domain", "dev", "--title", "X", "--owner-role", "reviewer", "--json"], None, None);
    assert_eq!(role_of(&subject(repo, "owned")["actor"]), Some("reviewer"));
}

#[test]
fn subject_move_records_the_declared_role_with_its_reason() {
    let dir = repo();
    let repo = dir.path();
    ok(repo, &["subject", "new", "mover", "--domain", "dev", "--title", "X", "--json"], None, None);
    ok(repo, &["subject", "move", "mover", "--to", "specced", "--reason", "scoped", "--actor", "plan-1", "--json"], Some("planner"), None);
    let moved = subject(repo, "mover");
    assert_eq!(moved["status_reason"]["actor"]["agent_id"], "plan-1", "{moved}");
    assert_eq!(role_of(&moved["status_reason"]["actor"]), Some("planner"), "{moved}");

    ok(repo, &["subject", "move", "mover", "--to", "building", "--reason", "started", "--actor", "plan-2", "--json"], None, None);
    let moved = subject(repo, "mover");
    assert_eq!(moved["status_reason"]["actor"]["agent_id"], "plan-2", "{moved}");
    assert_eq!(role_of(&moved["status_reason"]["actor"]), None, "no declared role, no role recorded: {moved}");
}

/// The `# canon: {...}` provenance lines of every feature file under the repo's specs.
fn provenance(repo: &Path) -> Vec<Value> {
    let mut lines = Vec::new();
    let mut stack = vec![repo.join(".canon/specs")];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "feature") {
                let text = std::fs::read_to_string(&path).unwrap();
                lines.extend(text.lines().filter_map(|l| l.trim().strip_prefix("# canon: ")).filter_map(|j| serde_json::from_str::<Value>(j).ok()));
            }
        }
    }
    lines
}

#[test]
fn feature_new_and_scenario_new_stamp_the_declared_role() {
    let dir = repo();
    let repo = dir.path();
    ok(repo, &["feature", "new", "cart.add", "--title", "Add to cart", "--actor", "plan-1"], Some("planner"), None);
    ok(repo, &["scenario", "new", "cart.add.01", "--title", "Adds an item", "--case", "happy", "--actor", "plan-1"], Some("planner"), None);
    let stamped = provenance(repo);
    assert!(stamped.len() >= 2, "{stamped:?}");
    assert!(stamped.iter().all(|p| p["actor"]["agent_id"] == "plan-1" && p["actor"]["role"] == "planner"), "{stamped:?}");

    let other = repo_with_scaffold();
    let stamped = provenance(other.path());
    assert!(stamped.len() >= 2, "{stamped:?}");
    assert!(stamped.iter().all(|p| p["actor"]["role"].is_null()), "no declared role, no role recorded: {stamped:?}");
}

fn repo_with_scaffold() -> tempfile::TempDir {
    let dir = repo();
    ok(dir.path(), &["feature", "new", "cart.add", "--title", "Add to cart"], None, None);
    ok(dir.path(), &["scenario", "new", "cart.add.01", "--title", "Adds an item", "--case", "happy"], None, None);
    dir
}
