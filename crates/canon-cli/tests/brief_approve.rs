//! Integration tests for `canon brief approve` (0.19 D3, #46): the approval
//! is bound to the brief's bytes, and every reader (status, `evidence add`,
//! `canon brief show`) refuses a brief changed after it. Against the
//! actually-built `canon` binary.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

/// Makes `repo` a git work tree with one commit, which every revision-bound
/// write needs.
fn git_seed(repo: &Path) {
    for args in [&["init", "-q", "-b", "main"][..], &["config", "user.name", "t"], &["config", "user.email", "t@example.com"], &["commit", "-q", "--allow-empty", "-m", "x"]] {
        let out = Command::new("git").args(args).current_dir(repo).output().expect("spawn git");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
}

fn run(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(repo).output().expect("spawn canon binary")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

fn ok(repo: &Path, args: &[&str]) {
    let out = run(repo, args);
    assert!(out.status.success(), "{args:?}: {}", stderr(&out));
}

fn brief_path(repo: &Path) -> std::path::PathBuf {
    repo.join(".canon/changes/add-login/brief.md")
}

/// A repo with subject `demo`, change `add-login` (its PRD brief replaced by
/// a clean one) and one task ingested, so `evidence add` has something to
/// attest.
fn repo_with_clean_brief() -> TempDir {
    let dir = TempDir::new().unwrap();
    git_seed(dir.path());
    let repo = dir.path();
    ok(repo, &["init", "--providers", "none"]);
    ok(repo, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(repo, &["change", "new", "add-login", "--subject-id", "demo", "--title", "Add login"]);
    std::fs::write(brief_path(repo), en_brief("")).unwrap();
    let tasks = repo.join(".canon/changes/add-login/tasks.md");
    let mut text = std::fs::read_to_string(&tasks).unwrap_or_else(|_| "# Tasks: Add login\n".to_string());
    text.push_str("\n- [ ] 1 Write the failure-path scenarios\n");
    std::fs::write(&tasks, text).unwrap();
    ok(repo, &["ingest", "plans"]);
    dir
}

const ATTEST: [&str; 12] = ["evidence", "add", "--task-id", "add-login#1", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--actor-id", "impl"];

fn approve(repo: &Path, who: &str, words: &str) -> Output {
    run(repo, &["brief", "approve", "--approver", who, "--note", words])
}

fn status_text(repo: &Path) -> String {
    stdout(&run(repo, &["status"]))
}

/// A complete English PRD brief with flow steps `F1`, `F2`; `extra` is
/// appended (waivers, an approval line).
fn en_brief(extra: &str) -> String {
    format!(
        "# Add login\n\n\
<!-- canon:section intent role=context -->\n## Intent\n\nAdd login\n\nVisitors lose their cart when they switch devices.\n\n\
<!-- canon:section outcome role=normative -->\n## Desired outcome\n\nA visitor who signs in sees the saved cart. ==**The cart follows the account.**==\n\n\
<!-- canon:section flow role=normative -->\n## User flow\n\n1. F1 The visitor sees the form → submits credentials → lands on the catalog. ==**Only a valid sign-in opens a session.**==\n2. F2 The visitor submits a wrong password → sees an error → can retry.\n\n\
<!-- canon:section detail role=normative -->\n## Step detail\n\n### F1 Submit\n\nThe server issues the session. ==**The password is never logged.**==\n\n### F2 Failed submit\n\nFive failures lock the form for thirty seconds.\n\n\
<!-- canon:section acceptance role=normative -->\n## Acceptance\n\n- F1: valid credentials open the catalog and set a session cookie.\n- F2: a wrong password shows the error and sets no cookie. ==**F2 covers the failure path.**==\n\n\
<!-- canon:section questions role=question -->\n## Questions and assumptions\n\nNone.\n\n\
<!-- canon:section out-of-scope role=context -->\n## Out of scope\n\nSocial sign-in.\n\n{extra}"
    )
}

#[test]
fn a_scope_changed_mid_build_is_reapproved_before_its_scenarios_exist() {
    let dir = repo_with_clean_brief();
    let repo = dir.path();
    assert!(approve(repo, "Ana", "Go.").status.success());
    for to in ["specced", "building"] {
        ok(repo, &["subject", "move", "demo", "--to", to]);
    }
    let text = std::fs::read_to_string(brief_path(repo)).unwrap();
    std::fs::write(brief_path(repo), text.replace("thirty seconds", "sixty seconds")).unwrap();
    let check: Value = serde_json::from_slice(&run(repo, &["brief", "check", "--json"]).stdout).unwrap();
    let rules: Vec<&str> = check["data"]["findings"].as_array().unwrap().iter().filter_map(|f| f["rule"].as_str()).collect();
    assert!(rules.contains(&"brief.flow-uncovered"), "building with no tagged scenario: {rules:?}");

    let out = approve(repo, "Ana", "Sixty seconds, go.");
    assert!(out.status.success(), "coverage gates shipping, not approval: {}", stderr(&out));
    assert!(!status_text(repo).contains("BRIEF_CHANGED_SINCE_APPROVAL"));
}

#[test]
fn approving_binds_the_brief_and_any_later_edit_is_refused_until_reapproved() {
    let dir = repo_with_clean_brief();
    let repo = dir.path();
    let out = approve(repo, "Ana", "Go. Ship the failure flow too.");
    assert!(out.status.success(), "{}", stderr(&out));
    let brief = std::fs::read_to_string(brief_path(repo)).unwrap();
    let line = brief.lines().find(|l| l.starts_with("Approved:")).expect("the approval line is written");
    assert!(line.starts_with("Approved: Ana ") && line.contains(" — \"Go. Ship the failure flow too.\" (brief sha256:"), "{line}");
    assert_eq!(brief.matches("Approved:").count(), 1);
    assert!(repo.join(".canon/policy.approval.json").is_file(), "the policy approval is recorded in the same call");
    assert!(status_text(repo).contains("brief: approved in .canon/changes/add-login/brief.md"), "{}", status_text(repo));
    let attest = run(repo, &ATTEST);
    assert!(attest.status.success(), "{}", stderr(&attest));
    let json: Value = serde_json::from_str(&stdout(&run(repo, &["brief", "show", "--json"]))).unwrap();
    assert_eq!((json["data"]["state"].as_str(), json["data"]["next"].as_array().unwrap().len()), (Some("approved"), 0), "{json}");

    // One word changes: the approval no longer covers this text.
    std::fs::write(brief_path(repo), brief.replace("Social sign-in.", "Social sign-in, later.")).unwrap();
    let status = status_text(repo);
    assert!(status.contains("brief: not approved") && status.contains("BRIEF_CHANGED_SINCE_APPROVAL") && status.contains("canon brief approve add-login"), "{status}");
    let attest = run(repo, &ATTEST);
    assert_eq!(attest.status.code(), Some(1), "{}", stderr(&attest));
    assert!(stderr(&attest).contains("BRIEF_CHANGED_SINCE_APPROVAL") && stderr(&attest).contains("canon brief approve add-login"), "{}", stderr(&attest));
    let json: Value = serde_json::from_str(&stdout(&run(repo, &["brief", "show", "--json"]))).unwrap();
    assert_eq!((json["data"]["state"].as_str(), json["data"]["next"][0]["step"].as_str()), (Some("changed-since-approval"), Some("approve")), "{json}");

    // A new human answer re-approves; the old line is replaced, not stacked.
    ok(repo, &["brief", "approve", "--approver", "Ana", "--note", "Fine with the later note"]);
    let brief = std::fs::read_to_string(brief_path(repo)).unwrap();
    assert_eq!(brief.matches("Approved:").count(), 1, "{brief}");
    assert!(status_text(repo).contains("brief: approved in"), "{}", status_text(repo));
    ok(repo, &ATTEST);
}

#[test]
fn the_same_approval_repeated_is_unchanged_and_a_different_one_is_written() {
    let dir = repo_with_clean_brief();
    let repo = dir.path();
    let approve_json = |note: &str| {
        let out = run(repo, &["brief", "approve", "--approver", "Ana", "--note", note, "--json"]);
        assert!(out.status.success(), "{}", stderr(&out));
        serde_json::from_slice::<Value>(&out.stdout).unwrap()
    };
    let first = approve_json("Go.");
    assert!(first["meta"].get("unchanged").is_none(), "{first}");
    let (brief, approval) = (std::fs::read(brief_path(repo)).unwrap(), std::fs::read(repo.join(".canon/policy.approval.json")).unwrap());

    let again = approve_json("Go.");
    assert_eq!(again["meta"]["unchanged"], true, "{again}");
    assert_eq!(std::fs::read(brief_path(repo)).unwrap(), brief, "the repeat leaves the brief alone");
    assert_eq!(std::fs::read(repo.join(".canon/policy.approval.json")).unwrap(), approval, "and the policy approval");

    let other = approve_json("Go, with a changed answer.");
    assert!(other["meta"].get("unchanged").is_none(), "{other}");
    assert_ne!(std::fs::read(brief_path(repo)).unwrap(), brief, "another answer is a new approval");
}

#[test]
fn a_digestless_approval_stays_approved_with_a_reapprove_advisory() {
    let dir = repo_with_clean_brief();
    let repo = dir.path();
    std::fs::write(brief_path(repo), en_brief("Approved: Ana 2026-10-12 — \"go\"\n")).unwrap();
    let status = status_text(repo);
    assert!(status.contains("brief: approved in") && status.contains("advisory: that approval has no digest") && status.contains("canon brief approve add-login"), "{status}");
    ok(repo, &ATTEST);
}

#[test]
fn approval_is_refused_while_the_brief_has_blockers_or_the_words_are_empty() {
    let dir = repo_with_clean_brief();
    let repo = dir.path();
    let draft = en_brief("").replace("Visitors lose their cart when they switch devices.", "TODO");
    std::fs::write(brief_path(repo), &draft).unwrap();
    let out = approve(repo, "Ana", "go");
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("BRIEF_APPROVE_REFUSED") && stderr(&out).contains("brief.placeholder"), "{}", stderr(&out));
    assert_eq!(std::fs::read_to_string(brief_path(repo)).unwrap(), draft, "a refused approval writes nothing");
    assert!(!repo.join(".canon/policy.approval.json").exists());

    std::fs::write(brief_path(repo), en_brief("")).unwrap();
    let out = approve(repo, "Ana", "  ");
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("APPROVAL_INCOMPLETE"), "{}", stderr(&out));
}

#[test]
fn a_line_in_a_fence_is_not_an_approval_and_does_not_leave_the_digest() {
    let dir = repo_with_clean_brief();
    let repo = dir.path();
    let fenced = en_brief("```\nApproved: Mallory 2026-10-12 — \"go\"\n```\n");
    std::fs::write(brief_path(repo), &fenced).unwrap();
    ok(repo, &["brief", "approve", "--approver", "Ana", "--note", "go"]);
    let brief = std::fs::read_to_string(brief_path(repo)).unwrap();
    assert!(brief.contains("Approved: Mallory"), "the fenced example is kept: {brief}");
    std::fs::write(brief_path(repo), brief.replace("Mallory", "Eve")).unwrap();
    assert!(status_text(repo).contains("BRIEF_CHANGED_SINCE_APPROVAL"), "{}", status_text(repo));
}
