//! An approved policy fails closed (0.22 D3) and status follows the brief
//! (0.22 D4), against the built binary on a copy of the
//! `subject-ship-allowed` conformance repo.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

const POLICY_WITH_REVIEW: &str = "spec_coverage:\n  require_cases: [failure]\n  require_review: {}\n";
const POLICY_WEAKENED: &str = "spec_coverage:\n  require_cases: []\n";

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(repo).env("CANON_ACTOR", "agent-a").env_remove("CANON_ROLE").output().expect("spawn canon binary")
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// The shippable conformance repo, with `policy` written and approved.
fn approved_repo(policy: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    copy_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/cases/subject-ship-allowed/repo"), dir.path());
    std::fs::write(dir.path().join(".canon/policy.yaml"), policy).unwrap();
    let out = canon(dir.path(), &["policy", "approve", "--approver", "human", "--note", "reviewed"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    dir
}

fn err_of(out: &Output) -> Value {
    serde_json::from_slice::<Value>(&out.stdout).unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {}", String::from_utf8_lossy(&out.stdout)))["error"].clone()
}

#[test]
fn a_policy_weakened_after_approval_fails_the_gate_and_the_ship_guard() {
    let dir = approved_repo(POLICY_WITH_REVIEW);
    let repo = dir.path();
    std::fs::write(repo.join(".canon/policy.yaml"), POLICY_WEAKENED).unwrap();

    let gate = canon(repo, &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(1), "{}{}", String::from_utf8_lossy(&gate.stdout), String::from_utf8_lossy(&gate.stderr));
    let text = format!("{}{}", String::from_utf8_lossy(&gate.stdout), String::from_utf8_lossy(&gate.stderr));
    assert!(text.contains("policy changed since approval"), "{text}");

    for extra in [&[][..], &["--override-reason", "ship it"][..]] {
        let mut args = vec!["subject", "move", "checkout", "--to", "shipped", "--json"];
        args.extend_from_slice(extra);
        let out = canon(repo, &args);
        assert_eq!(out.status.code(), Some(1), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
        let error = err_of(&out);
        assert_eq!(error["code"], "POLICY_CHANGED_SINCE_APPROVAL", "{args:?}: {error}");
    }
    let shown = canon(repo, &["subject", "show", "checkout"]);
    assert!(String::from_utf8_lossy(&shown.stdout).contains("verifying"), "the refused ship left the subject verifying");

    // Restoring the approved bytes lifts it (the approved policy's own review check still applies).
    std::fs::write(repo.join(".canon/policy.yaml"), POLICY_WITH_REVIEW).unwrap();
    let restored = canon(repo, &["gate", "check"]);
    assert!(!String::from_utf8_lossy(&restored.stdout).contains("policy changed since approval"), "{}", String::from_utf8_lossy(&restored.stdout));
    let ship = canon(repo, &["subject", "move", "checkout", "--to", "shipped", "--json"]);
    assert_eq!(err_of(&ship)["code"], "SUBJECT_GATE_VIOLATION", "the approved policy requires a review the repo lacks");
}

#[test]
fn an_unreadable_approval_fails_closed_too() {
    let dir = approved_repo(POLICY_WITH_REVIEW);
    let repo = dir.path();
    std::fs::write(repo.join(".canon/policy.approval.json"), "{ not json").unwrap();
    assert_eq!(canon(repo, &["gate", "check"]).status.code(), Some(1));
    let out = canon(repo, &["subject", "move", "checkout", "--to", "shipped", "--json"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(err_of(&out)["code"], "POLICY_APPROVAL_UNREADABLE");
}

#[test]
fn a_policy_never_approved_stays_advisory() {
    let dir = approved_repo(POLICY_WITH_REVIEW);
    let repo = dir.path();
    std::fs::remove_file(repo.join(".canon/policy.approval.json")).unwrap();
    std::fs::write(repo.join(".canon/policy.yaml"), POLICY_WEAKENED).unwrap();
    let gate = canon(repo, &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(0), "{}", String::from_utf8_lossy(&gate.stderr));
    assert!(String::from_utf8_lossy(&gate.stdout).contains("policy not approved"));
    let ship = canon(repo, &["subject", "move", "checkout", "--to", "shipped"]);
    assert_eq!(ship.status.code(), Some(0), "{}", String::from_utf8_lossy(&ship.stderr));
}

#[test]
fn policy_show_reads_back_the_approval() {
    let dir = approved_repo(POLICY_WITH_REVIEW);
    let repo = dir.path();
    let show = |repo: &Path| -> Value {
        let out = canon(repo, &["policy", "show", "--json"]);
        assert_eq!(out.status.code(), Some(0));
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["data"].clone()
    };
    let data = show(repo);
    assert_eq!(data["state"], "approved");
    assert_eq!(data["approvedBy"], "human");
    assert_eq!(data["note"], "reviewed");
    assert_eq!(data["policy"], ".canon/policy.yaml");
    assert_eq!(data["policyDigest"], data["approvedDigest"]);
    assert!(data["approvedAt"].as_str().unwrap().ends_with('Z'));

    std::fs::write(repo.join(".canon/policy.yaml"), POLICY_WEAKENED).unwrap();
    let data = show(repo);
    assert_eq!(data["state"], "changed");
    assert_ne!(data["policyDigest"], data["approvedDigest"]);

    std::fs::remove_file(repo.join(".canon/policy.approval.json")).unwrap();
    let data = show(repo);
    assert_eq!(data["state"], "unapproved");
    assert!(data["approvedBy"].is_null() && data["approvedDigest"].is_null());
    assert!(String::from_utf8_lossy(&canon(repo, &["policy", "show"]).stdout).contains("approval: unapproved"));
}

#[test]
fn a_fresh_change_is_the_planners_step_until_its_brief_is_clean() {
    let dir = TempDir::new().unwrap();
    let repo = dir.path();
    for args in [&["init", "--providers", "none"][..], &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"][..]] {
        assert!(canon(repo, args).status.success(), "{args:?}");
    }
    let created = canon(repo, &["change", "new", "add-login", "--subject", "demo", "--title", "Add login", "--json"]);
    assert!(created.status.success(), "{}", String::from_utf8_lossy(&created.stderr));
    let created: Value = serde_json::from_slice(&created.stdout).unwrap();
    let step = &created["meta"]["next"][0];
    assert_eq!((step["x-canon"]["actor"].as_str(), step["x-canon"]["role"].as_str()), (Some("agent"), Some("planner")), "{step}");
    assert_eq!(step["command"], "canon brief check add-login --json");

    // The planner sees it under its own role; nothing waits on the human yet.
    let out = Command::new(env!("CARGO_BIN_EXE_canon")).args(["status", "--json"]).current_dir(repo).env("CANON_ROLE", "planner").env("CANON_ACTOR", "p").output().unwrap();
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(report["meta"]["next"].as_array().unwrap().iter().any(|s| s["command"] == "canon brief check add-login --json"), "{report}");
    assert!(report["meta"]["waitingOn"].get("human").is_none(), "{report}");
}
