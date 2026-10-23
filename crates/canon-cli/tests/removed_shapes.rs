//! Every command shape replaced by an operand form is a usage error (exit 2)
//! whose `error.hint` names the replacement. Against the built `canon` binary.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn run(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).arg("--json").current_dir(repo).output().expect("spawn canon binary")
}

#[test]
fn every_removed_shape_is_refused_with_a_hint_naming_its_replacement() {
    let dir = TempDir::new().unwrap();
    let init = run(dir.path(), &["init", "--no-agents-md", "--no-policy"]);
    assert!(init.status.success(), "{}", String::from_utf8_lossy(&init.stderr));

    let cases: &[(&[&str], &[&str])] = &[
        (&["review", "add", "--project-id", "root", "--scenario-id", "a.b.01", "--reviewer", "r", "--pin", "p", "--upstream-ref", "x"], &["review scenario", "review change"]),
        (&["brief", "check", "--change-id", "x"], &["brief check"]),
        (&["brief", "approve", "--change-id", "x", "--by", "a", "--note", "b"], &["brief approve"]),
        (&["dispatch", "end", "--run-id", "R", "--status", "succeeded"], &["dispatch end"]),
        (&["subject", "adopt", "x", "--subject-id", "y"], &["subject adopt"]),
        (&["subject", "unadopt", "x", "--subject-id", "y"], &["subject unadopt"]),
        (&["prompt", "add", "--name", "n", "--version", "v", "--manifest", "f"], &["prompt add"]),
        (&["prompt", "show", "--name", "n", "--version", "v"], &["prompt show"]),
        (&["divergence", "add", "--project-id", "root", "--scenario-id", "s", "--sha", "abc"], &["divergence add"]),
        (&["finding", "close", "--change-id", "c", "--round", "1", "--seq", "1", "--disposition", "open"], &["finding close"]),
        (&["evidence", "add", "--evaluator-id", "e", "--ref", "r"], &["evidence audit"]),
        (&["evidence", "add", "--kind", "audit", "--scenario-id", "a.b.01", "--ref", "r", "--role", "implementer"], &["evidence audit"]),
    ];

    for (args, needles) in cases {
        let out = run(dir.path(), args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        let body: Value = serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{args:?}: stdout is not JSON ({e})"));
        assert_ne!(body["ok"], Value::Bool(true), "{args:?}: {body}");
        let hint = body["error"]["hint"].as_str().unwrap_or_default();
        assert!(!hint.is_empty(), "{args:?}: no hint: {body}");
        for needle in *needles {
            assert!(hint.contains(needle), "{args:?}: hint lacks {needle:?}: {hint}");
        }
    }
}
