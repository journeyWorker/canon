//! Human-only acts (0.28 D10): `brief approve`, `policy approve` and
//! `divergence accept` refuse every agent role, however it is declared, and no
//! flag grants them. Against the built `canon` binary.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

const ROLES: [&str; 4] = ["planner", "implementer", "reviewer", "orchestrator"];

const LEAVES: [(&str, &[&str]); 3] = [
    ("brief approve", &["brief", "approve", "--approver", "alice", "--note", "approved as written"]),
    ("policy approve", &["policy", "approve", "--approver", "alice", "--note", "reviewed"]),
    ("divergence accept", &["divergence", "accept", "auth.login.01", "--actor-id", "alice", "--reason", "known gap", "--tracking", "https://example.com/issues/1", "--role", "reviewer"]),
];

fn run(repo: &Path, args: &[&str], role_flag: Option<&str>, role_env: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_canon"));
    command.args(args).arg("--json").current_dir(repo).env_remove("CANON_ROLE");
    if let Some(role) = role_flag {
        command.args(["--as", role]);
    }
    if let Some(role) = role_env {
        command.env("CANON_ROLE", role);
    }
    command.output().expect("spawn canon")
}

fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let init = run(dir.path(), &["init", "--no-agents-md", "--policy", "light", "--providers", "none"], None, None);
    assert!(init.status.success(), "{}{}", String::from_utf8_lossy(&init.stdout), String::from_utf8_lossy(&init.stderr));
    dir
}

fn code_of(out: &Output) -> Value {
    let body: Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {}", String::from_utf8_lossy(&out.stdout)));
    body["error"]["code"].clone()
}

#[test]
fn every_agent_role_is_refused_by_flag_and_by_environment() {
    let dir = repo();
    for (leaf, args) in LEAVES {
        for role in ROLES {
            for (flag, env) in [(Some(role), None), (None, Some(role))] {
                let out = run(dir.path(), args, flag, env);
                assert_eq!(out.status.code(), Some(2), "{leaf} as {role}: {}", String::from_utf8_lossy(&out.stdout));
                let body: Value = serde_json::from_slice(&out.stdout).unwrap();
                assert_eq!(body["ok"], Value::Bool(false));
                assert_eq!(body["error"]["code"], "HUMAN_ONLY", "{leaf} as {role}: {body}");
                assert_eq!(body["error"]["retryable"], Value::Bool(false), "{leaf} as {role}: {body}");
                let hint = body["error"]["hint"].as_str().unwrap_or_default();
                assert!(hint.contains(leaf), "the hint names the leaf ({leaf}): {hint}");
                assert!(hint.contains("--approver") || hint.contains("--actor-id"), "the hint names the options only the human supplies: {hint}");
            }
        }
    }
}

#[test]
fn a_refusal_changes_nothing() {
    let dir = repo();
    let before = std::fs::read_dir(dir.path().join(".canon")).unwrap().count();
    let out = run(dir.path(), LEAVES[1].1, Some("implementer"), None);
    assert_eq!(code_of(&out), "HUMAN_ONLY");
    assert!(!dir.path().join(".canon/policy.approval.json").exists());
    assert_eq!(std::fs::read_dir(dir.path().join(".canon")).unwrap().count(), before);
}

#[test]
fn without_a_declared_role_the_leaf_runs() {
    let dir = repo();
    let out = run(dir.path(), LEAVES[1].1, None, None);
    assert_eq!(out.status.code(), Some(0), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(dir.path().join(".canon/policy.approval.json").exists());
    // The other two reach their own checks (no change, no scenario) rather than the role refusal.
    for (leaf, args) in [LEAVES[0], LEAVES[2]] {
        let out = run(dir.path(), args, None, None);
        assert_ne!(code_of(&out), "HUMAN_ONLY", "{leaf} refused a human");
    }
}

#[test]
fn the_code_is_registered_non_retryable() {
    let out = Command::new(env!("CARGO_BIN_EXE_canon")).args(["schema", "--error", "HUMAN_ONLY", "--json"]).output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let body: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(body["data"]["error"]["retryable"], Value::Bool(false));
    assert_eq!(body["data"]["error"]["exit"], 2);
}
