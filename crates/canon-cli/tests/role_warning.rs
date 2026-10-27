//! `--role` is an open slug (owner decision 2026-10-23): a policy may require a
//! custom role, so an unknown role runs and warns with the closest known role;
//! it is never refused. Against the built `canon` binary.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(repo).env_remove("CANON_ROLE").env_remove("CANON_ACTOR").output().expect("spawn canon")
}

fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git").current_dir(repo).args(["-c", "user.name=canon", "-c", "user.email=canon@example.invalid"]).args(args).output().expect("spawn git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// A repo with a commit and a policy that names the custom role `auditor`.
fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    git(root, &["init", "-q"]);
    let init = canon(root, &["init", "--no-agents-md", "--policy", "light", "--providers", "none"]);
    assert!(init.status.success(), "{}", String::from_utf8_lossy(&init.stderr));
    let policy = root.join(".canon/policy.yaml");
    let text = std::fs::read_to_string(&policy).unwrap();
    std::fs::write(&policy, format!("{text}\nrisk_routing:\n  auditor: true\n")).unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "fixture"]);
    dir
}

/// `finding add` as `role`, returning its envelope.
fn add(repo: &Path, seq: &str, role: &str) -> Value {
    let out = canon(repo, &["finding", "add", "--change", "c-role", "--round", "1", "--seq", seq, "--severity", "note", "--actor", "x", "--summary", "s", "--role", role, "--no-promote", "--json"]);
    assert!(out.status.success(), "role {role}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    serde_json::from_slice(&out.stdout).expect("one JSON document")
}

fn warnings(body: &Value) -> Vec<String> {
    body["meta"]["warnings"].as_array().map(|list| list.iter().map(|w| w.as_str().unwrap().to_string()).collect()).unwrap_or_default()
}

#[test]
fn a_misspelt_role_runs_and_warns_with_the_closest_known_role() {
    let dir = repo();
    let body = add(dir.path(), "1", "reviwer");
    assert_eq!(body["ok"], true, "{body}");
    let found = warnings(&body);
    assert!(found.iter().any(|w| w.contains("reviwer") && w.contains("reviewer")), "the warning names the closest role: {found:?}");
}

#[test]
fn a_known_role_and_a_policy_named_role_do_not_warn() {
    let dir = repo();
    for (seq, role) in [("1", "reviewer"), ("2", "auditor")] {
        let body = add(dir.path(), seq, role);
        assert!(warnings(&body).is_empty(), "{role}: {:?}", warnings(&body));
    }
}

#[test]
fn an_unknown_role_with_nothing_close_still_runs_and_warns_without_a_suggestion() {
    let dir = repo();
    let body = add(dir.path(), "1", "zzzzzzzz");
    assert_eq!(body["ok"], true, "{body}");
    let found = warnings(&body);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(!found[0].contains("did you mean"), "{found:?}");
}
