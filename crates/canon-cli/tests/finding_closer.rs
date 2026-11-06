//! `canon-0-38-finding-closer`, scenarios `finding.closer.01-12`: a finding is closed `rejected` or `deferred` only by a reviewer, the closing
//! record carries the role the call declared (none when it declared none), and `canon finding list` names who closed each closed finding.
//! Against the built `canon` binary. Tests are named by scenario id (`finding_closer_03_…` proves `finding.closer.03`).

use std::path::Path;
use std::process::{Command, Output};

use canon_model::{Actor, ChangeId, Envelope, Finding, FindingSeverity, RecordKind, RoleId, Sha, Subject, SubjectId, SubjectStatus};
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use chrono::Utc;
use serde_json::Value;
use tempfile::TempDir;

#[path = "support/hermetic.rs"]
mod hermetic;

const CHANGE: &str = "c-demo";

const CANON_YAML: &str = "\
tiers:
  local: { backend: git, root: .canon/ledger }
routing:
  subject: local
  change: local
  scenario: local
  evidence: local
  finding: local
";

fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .env("GIT_AUTHOR_NAME", "a")
        .env("GIT_AUTHOR_EMAIL", "a@a")
        .env("GIT_COMMITTER_NAME", "a")
        .env("GIT_COMMITTER_EMAIL", "a@a")
        .output()
        .expect("git on PATH");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn repo() -> TempDir {
    let dir = TempDir::new().expect("a temp dir");
    std::fs::write(dir.path().join("canon.yaml"), CANON_YAML).expect("canon.yaml");
    git(dir.path(), &["init", "-q", "-b", "main"]);
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "initial"]);
    dir
}

/// `canon` in `repo`, with the declared role of the call: `Some` sets `CANON_ROLE`, `None` removes it.
fn canon(repo: &Path, role: Option<&str>, args: &[&str]) -> Output {
    let mut command = hermetic::canon();
    command.args(args).current_dir(repo);
    if let Some(role) = role {
        command.env("CANON_ROLE", role);
    }
    command.output().expect("spawn canon")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn json(out: &Output) -> Value {
    serde_json::from_str(&stdout(out)).unwrap_or_else(|e| panic!("not JSON ({e}): {}{}", stdout(out), stderr(out)))
}

/// Raise round 1 seq `seq` (a blocker) and commit it.
fn open_finding(repo: &Path, seq: &str) {
    let added = canon(
        repo,
        None,
        &["finding", "add", "--change", CHANGE, "--round", "1", "--seq", seq, "--severity", "blocker", "--introduced-unknown", "--actor", "rev-a", "--summary", "needs a guard", "--role", "reviewer"],
    );
    assert!(added.status.success(), "{}", stderr(&added));
    let promoted = canon(repo, None, &["gate", "promote"]);
    assert!(promoted.status.success(), "{}", stderr(&promoted));
}

fn close(repo: &Path, role: Option<&str>, seq: &str, disposition: &str, extra: &[&str]) -> Output {
    let mut args = vec!["finding", "close", CHANGE, "--round", "1", "--seq", seq, "--disposition", disposition, "--actor", "closer-1", "--json"];
    if disposition != "fixed" {
        args.extend(["--reason", "because"]);
    }
    args.extend_from_slice(extra);
    canon(repo, role, &args)
}

fn listed(repo: &Path, extra: &[&str]) -> Vec<Value> {
    let mut args = vec!["finding", "list", "--change", CHANGE, "--json"];
    args.extend_from_slice(extra);
    let out = canon(repo, None, &args);
    assert!(out.status.success(), "{}", stderr(&out));
    json(&out)["data"]["findings"].as_array().expect("findings").clone()
}

/// Every committed version of every finding, the latest by `at` per seq.
fn versions(repo: &Path) -> Vec<Value> {
    fn walk(dir: &Path, out: &mut Vec<Value>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(value) = serde_json::from_str::<Value>(&text) {
                    out.push(value);
                }
            }
        }
    }
    let mut out = Vec::new();
    for root in [".canon/ledger/kind=finding", ".canon/archive/kind=finding"] {
        walk(&repo.join(root), &mut out);
    }
    out.sort_by_key(|v| v["at"].as_str().unwrap_or_default().to_string());
    out
}

fn staged(repo: &Path) -> usize {
    let mut out = Vec::new();
    fn count(dir: &Path, out: &mut Vec<()>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.filter_map(Result::ok) {
            if entry.path().is_dir() {
                count(&entry.path(), out);
            } else {
                out.push(());
            }
        }
    }
    count(&repo.join(".canon/ledger/_staging/kind=finding"), &mut out);
    out.len()
}

fn latest_of(repo: &Path, seq: u64) -> Value {
    versions(repo).into_iter().rfind(|v| v["seq"] == seq).expect("a version of the finding")
}

/// finding.closer.01
#[test]
fn finding_closer_01_an_implementer_or_orchestrator_closes_fixed_and_the_record_carries_the_role() {
    let dir = repo();
    open_finding(dir.path(), "1");
    open_finding(dir.path(), "2");
    for (seq, role) in [("1", "implementer"), ("2", "orchestrator")] {
        let out = close(dir.path(), Some(role), seq, "fixed", &[]);
        assert!(out.status.success(), "{role}: {}", stderr(&out));
        assert!(canon(dir.path(), None, &["gate", "promote"]).status.success());
        let closed = latest_of(dir.path(), seq.parse().unwrap());
        assert_eq!(closed["disposition"], "fixed");
        assert_eq!(closed["actor"]["role"], role, "{closed}");
        assert_eq!(closed["actor"]["agent_id"], "closer-1", "{closed}");
        assert_eq!(closed["reviewer"], "rev-a", "the reviewer who raised it is unchanged: {closed}");
    }
    // `--as` declares the role as well as the environment.
    open_finding(dir.path(), "3");
    let out = close(dir.path(), None, "3", "fixed", &["--as", "planner"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(canon(dir.path(), None, &["gate", "promote"]).status.success());
    assert_eq!(latest_of(dir.path(), 3)["actor"]["role"], "planner");
}

/// finding.closer.02
#[test]
fn finding_closer_02_a_declared_role_and_a_different_role_option_is_refused_and_writes_nothing() {
    let dir = repo();
    open_finding(dir.path(), "1");
    let before = versions(dir.path()).len();
    for role in [Some("implementer"), None] {
        let mut extra = vec!["--role", "reviewer"];
        if role.is_none() {
            extra.extend(["--as", "implementer"]);
        }
        let out = close(dir.path(), role, "1", "fixed", &extra);
        assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
        let error = json(&out)["error"].clone();
        assert_eq!(error["code"], "FINDING_ARGUMENT_INVALID", "{error}");
        assert_eq!(error["field"], "--role", "{error}");
    }
    assert_eq!(staged(dir.path()), 0);
    assert_eq!(versions(dir.path()).len(), before);
    assert_eq!(listed(dir.path(), &["--open"]).len(), 1, "the finding stays open");
    // The same role named twice is accepted.
    let ok = close(dir.path(), Some("implementer"), "1", "fixed", &["--role", "implementer"]);
    assert!(ok.status.success(), "{}", stderr(&ok));
}

/// finding.closer.03
#[test]
fn finding_closer_03_a_role_other_than_reviewer_cannot_close_rejected_or_deferred() {
    let dir = repo();
    open_finding(dir.path(), "1");
    let before = versions(dir.path()).len();
    for role in ["implementer", "orchestrator", "planner"] {
        for disposition in ["rejected", "deferred"] {
            let out = close(dir.path(), Some(role), "1", disposition, &[]);
            assert_eq!(out.status.code(), Some(2), "{role} {disposition}: {}", stderr(&out));
            let error = json(&out)["error"].clone();
            assert_eq!(error["code"], "FINDING_ROLE_DENIED", "{error}");
            assert_eq!(error["retryable"], false, "{error}");
            let hint = error["hint"].as_str().unwrap_or_default();
            assert!(hint.contains("reviewer") && hint.contains("--as reviewer"), "{error}");
        }
    }
    // `--role` alone, with nothing declared, is the role of the call.
    let out = close(dir.path(), None, "1", "rejected", &["--role", "implementer"]);
    assert_eq!(json(&out)["error"]["code"], "FINDING_ROLE_DENIED");
    assert_eq!(staged(dir.path()), 0);
    assert_eq!(versions(dir.path()).len(), before);
}

/// finding.closer.04
#[test]
fn finding_closer_04_after_the_refusal_the_finding_is_open_and_its_blocker_still_blocks_the_gate() {
    let dir = repo();
    let repo = dir.path();
    std::fs::create_dir_all(repo.join(".canon")).unwrap();
    std::fs::write(repo.join(".canon/policy.yaml"), "spec_coverage:\n  require_evidence: true\n  require_review: {}\n").unwrap();
    let envelope = Envelope::new(1, RecordKind::Subject, Utc::now(), Actor::new("canon", RoleId::parse("implementer").unwrap()));
    let subject = Subject::new(envelope, SubjectId::parse("demo-subject").unwrap(), "Seeded", "s", "dev", SubjectStatus::Verifying, RoleId::parse("implementer").unwrap())
        .with_change_ids(vec![ChangeId::parse(CHANGE).unwrap()]);
    GitTier::new(repo.join(".canon/ledger")).write(&subject).unwrap();
    open_finding(repo, "1");

    let gate_before = canon(repo, None, &["gate", "check"]);
    assert_eq!(gate_before.status.code(), Some(1), "{}", stdout(&gate_before));
    assert!(stdout(&gate_before).contains("open-blocker (1)"), "{}", stdout(&gate_before));

    let refused = close(repo, Some("implementer"), "1", "rejected", &[]);
    assert_eq!(refused.status.code(), Some(2));
    assert_eq!(staged(repo), 0);
    let rows = listed(repo, &["--open"]);
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0]["disposition"], "open");
    let gate_after = canon(repo, None, &["gate", "check"]);
    assert_eq!(gate_after.status.code(), Some(1), "{}", stdout(&gate_after));
    assert!(stdout(&gate_after).contains("open-blocker (1)"), "{}", stdout(&gate_after));
}

/// finding.closer.05
#[test]
fn finding_closer_05_a_reviewer_closes_with_any_disposition_and_the_record_says_reviewer() {
    let dir = repo();
    for (seq, disposition) in [("1", "fixed"), ("2", "rejected"), ("3", "deferred")] {
        open_finding(dir.path(), seq);
        let out = close(dir.path(), Some("reviewer"), seq, disposition, &[]);
        assert!(out.status.success(), "{disposition}: {}", stderr(&out));
        assert!(canon(dir.path(), None, &["gate", "promote"]).status.success());
        let closed = latest_of(dir.path(), seq.parse().unwrap());
        assert_eq!(closed["disposition"], disposition);
        assert_eq!(closed["actor"]["role"], "reviewer", "{closed}");
    }
    // `--role reviewer` with nothing declared is the reviewer's close too.
    open_finding(dir.path(), "4");
    assert!(close(dir.path(), None, "4", "rejected", &["--role", "reviewer"]).status.success());
    assert!(canon(dir.path(), None, &["gate", "promote"]).status.success());
    assert_eq!(latest_of(dir.path(), 4)["actor"]["role"], "reviewer");
}

/// finding.closer.06
#[test]
fn finding_closer_06_a_reviewer_closing_rejected_without_a_reason_is_refused() {
    let dir = repo();
    open_finding(dir.path(), "1");
    let out = canon(dir.path(), Some("reviewer"), &["finding", "close", CHANGE, "--round", "1", "--seq", "1", "--disposition", "rejected", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert_eq!(json(&out)["error"]["code"], "FINDING_REASON_REQUIRED");
    assert_eq!(staged(dir.path()), 0);
    assert_eq!(listed(dir.path(), &["--open"]).len(), 1);
}

/// finding.closer.07
#[test]
fn finding_closer_07_a_call_with_no_role_closes_and_the_record_carries_no_role() {
    let dir = repo();
    for (seq, disposition) in [("1", "fixed"), ("2", "rejected"), ("3", "deferred")] {
        open_finding(dir.path(), seq);
        let out = close(dir.path(), None, seq, disposition, &[]);
        assert!(out.status.success(), "{disposition}: {}", stderr(&out));
        assert!(canon(dir.path(), None, &["gate", "promote"]).status.success());
        let closed = latest_of(dir.path(), seq.parse().unwrap());
        assert_eq!(closed["disposition"], disposition);
        assert!(closed["actor"].get("role").is_none(), "no role is recorded, reviewer least of all: {closed}");
        assert_eq!(closed["actor"]["agent_id"], "closer-1");
    }
}

/// finding.closer.08
#[test]
fn finding_closer_08_before_a_finding_is_closed_the_list_names_no_closer() {
    let dir = repo();
    open_finding(dir.path(), "1");
    let rows = listed(dir.path(), &[]);
    assert!(rows[0].get("closedBy").is_none_or(Value::is_null), "{}", rows[0]);
    assert!(rows[0].get("closedRole").is_none_or(Value::is_null), "{}", rows[0]);
    let table = stdout(&canon(dir.path(), None, &["finding", "list", "--change", CHANGE]));
    let row = table.lines().find(|l| l.contains("needs a guard")).expect("the row");
    assert!(row.contains("  -  "), "an open finding shows no closer: {table}");
}

/// finding.closer.09
#[test]
fn finding_closer_09_the_list_names_who_closed_each_closed_finding_and_under_which_role() {
    let dir = repo();
    for seq in ["1", "2", "3", "4"] {
        open_finding(dir.path(), seq);
    }
    assert!(close(dir.path(), Some("reviewer"), "1", "rejected", &[]).status.success());
    assert!(close(dir.path(), Some("implementer"), "2", "fixed", &[]).status.success());
    assert!(close(dir.path(), None, "3", "deferred", &[]).status.success());
    assert!(canon(dir.path(), None, &["gate", "promote"]).status.success());

    let rows = listed(dir.path(), &[]);
    let by_seq = |seq: u64| rows.iter().find(|r| r["seq"] == seq).expect("a row").clone();
    assert_eq!((by_seq(1)["closedBy"].clone(), by_seq(1)["closedRole"].clone()), ("closer-1".into(), "reviewer".into()));
    assert_eq!((by_seq(2)["closedBy"].clone(), by_seq(2)["closedRole"].clone()), ("closer-1".into(), "implementer".into()));
    assert_eq!(by_seq(3)["closedBy"], "closer-1");
    assert!(by_seq(3)["closedRole"].is_null(), "{}", by_seq(3));
    assert!(by_seq(4)["closedBy"].is_null() && by_seq(4)["closedRole"].is_null(), "an open finding names no closer: {}", by_seq(4));

    let table = stdout(&canon(dir.path(), None, &["finding", "list", "--change", CHANGE]));
    assert!(table.contains("closed by"), "{table}");
    assert!(table.contains("closer-1 (reviewer)") && table.contains("closer-1 (implementer)"), "{table}");
    let only = listed(dir.path(), &["--fields", "closedBy,closedRole"]);
    assert_eq!(only[0]["closedBy"], "closer-1", "{only:?}");
}

/// finding.closer.10
#[test]
fn finding_closer_10_a_finding_closed_by_0_37_lists_without_an_error_as_written() {
    let dir = repo();
    open_finding(dir.path(), "1");
    // 0.37 wrote the closing version with the actor `canon` and the default role `reviewer`.
    let mut envelope = Envelope::new(1, RecordKind::Finding, Utc::now() + chrono::Duration::seconds(5), Actor::new("canon", RoleId::parse("reviewer").unwrap()));
    envelope.at = Utc::now() + chrono::Duration::seconds(5);
    let open = Finding::new(envelope.clone(), ChangeId::parse(CHANGE).unwrap(), 1, 1, FindingSeverity::Blocker, "rev-a", "needs a guard");
    let closed = open.fixed_by(Sha::parse("c".repeat(40)).unwrap());
    let ledger = GitTier::new(dir.path().join(".canon/ledger"));
    ledger.write(&closed).unwrap();
    let before = versions(dir.path());

    let out = canon(dir.path(), None, &["finding", "list", "--change", CHANGE, "--json"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let row = json(&out)["data"]["findings"][0].clone();
    assert_eq!((row["closedBy"].clone(), row["closedRole"].clone()), ("canon".into(), "reviewer".into()), "{row}");
    assert_eq!(versions(dir.path()), before, "reading rewrites nothing");
}

fn changelog() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CHANGELOG.md")).expect("CHANGELOG.md")
}

/// What the Unreleased section of `changelog` lacks for this change: the Breaking entry, the Changed entry and the line on the guides.
fn changelog_problems(changelog: &str) -> Vec<String> {
    let unreleased = changelog.split("\n## ").find(|section| section.starts_with("Unreleased")).unwrap_or_default();
    let subsection = |name: &str| -> String {
        let marker = format!("### {name}\n");
        let Some(start) = unreleased.find(&marker) else { return String::new() };
        let rest = &unreleased[start + marker.len()..];
        rest.split("\n### ").next().unwrap_or_default().to_string()
    };
    let mut problems = Vec::new();
    let breaking = subsection("Breaking");
    if !(breaking.contains("canon-0-38-finding-closer") && breaking.contains("FINDING_ROLE_DENIED") && breaking.contains("rejected") && breaking.contains("deferred")) {
        problems.push("Breaking names no refusal of rejected and deferred (canon-0-38-finding-closer, FINDING_ROLE_DENIED)".to_string());
    }
    let changed = subsection("Changed");
    if !(changed.contains("canon-0-38-finding-closer") && changed.contains("closedBy") && changed.contains("closedRole")) {
        problems.push("Changed names no recorded role of a close (canon-0-38-finding-closer, closedBy, closedRole)".to_string());
    }
    if !(unreleased.contains("role-implementer") && unreleased.contains("role-reviewer")) {
        problems.push("no line says the implementer guide has no `finding close` and the reviewer guide has".to_string());
    }
    problems
}

/// finding.closer.11
#[test]
fn finding_closer_11_the_changelog_names_the_refusal_the_recorded_role_and_the_guides() {
    let problems = changelog_problems(&changelog());
    assert!(problems.is_empty(), "{problems:#?}");
}

/// finding.closer.12 (failure case): the check finds each missing entry, and a real entry passes it.
#[test]
fn finding_closer_12_a_changelog_without_the_entry_fails_the_release_check() {
    assert_eq!(changelog_problems("# Changelog\n\n## Unreleased\n\n## 0.37.0\n").len(), 3);
    assert_eq!(changelog_problems("# Changelog\n\n## Unreleased\n\n### Breaking\n\nonly canon-0-38-finding-closer FINDING_ROLE_DENIED rejected deferred\n\n### Changed\n\nnothing\n").len(), 2);
    let whole = "# Changelog\n\n## Unreleased\n\n### Breaking\n\ncanon-0-38-finding-closer FINDING_ROLE_DENIED rejected deferred\n\n### Changed\n\ncanon-0-38-finding-closer closedBy closedRole role-implementer role-reviewer\n";
    assert!(changelog_problems(whole).is_empty());
}
