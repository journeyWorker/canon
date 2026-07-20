//! Acceptance: "a --check MISSING/no-drift/DRIFT test over a fixture"
//! (parity.py `cmd_report` shape, design D2, tasks.md 3.2).

mod support;

use canon_report::{check_report, write_report, CheckOutcome, ReportInputs};

fn inputs(dir: &std::path::Path) -> ReportInputs {
    let roots = support::corpus::build(dir);
    ReportInputs::new(dir, roots)
}

#[test]
fn check_reports_missing_when_the_report_was_never_generated() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let report_path = dir.path().join("REPORT.md");

    let outcome = check_report(&inputs(dir.path()), &report_path).unwrap();
    assert_eq!(outcome, CheckOutcome::Missing);
    assert_eq!(outcome.exit_code(), 1);
    assert!(outcome.message(&report_path).contains("MISSING"));
}

#[test]
fn check_reports_no_drift_immediately_after_a_write() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let report_path = dir.path().join("REPORT.md");
    let in_ = inputs(dir.path());

    write_report(&in_, &report_path).unwrap();
    let outcome = check_report(&in_, &report_path).unwrap();

    assert_eq!(outcome, CheckOutcome::NoDrift);
    assert_eq!(outcome.exit_code(), 0);
    assert_eq!(outcome.message(&report_path), "canon report --check: no drift");
}

#[test]
fn check_reports_drift_when_the_committed_file_has_been_hand_edited() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let report_path = dir.path().join("REPORT.md");
    let in_ = inputs(dir.path());

    write_report(&in_, &report_path).unwrap();
    std::fs::write(&report_path, "# stale hand edit\n").unwrap();

    let outcome = check_report(&in_, &report_path).unwrap();
    assert_eq!(outcome, CheckOutcome::Drift);
    assert_eq!(outcome.exit_code(), 1);
    assert!(outcome.message(&report_path).contains("DRIFT"));
}

#[test]
fn check_reports_drift_when_new_evidence_lands_after_the_committed_write() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let report_path = dir.path().join("REPORT.md");
    let in_ = inputs(dir.path());
    write_report(&in_, &report_path).unwrap();

    // Simulate the corpus changing underneath an already-committed
    // report (a new evidence record lands) without re-running `canon
    // report` — the drift gate must catch this, not just a hand edit.
    use canon_model::envelope::{Actor, Envelope, RecordKind};
    use canon_model::ids::{RoleId, TaskId};
    use canon_model::records::{EvidenceRecord, EvidenceVerdict};
    use canon_store::git_tier::GitTier;
    use canon_store::tier::Tier;
    let tier = GitTier::new(in_.roots.git_root.clone());
    tier.write(&EvidenceRecord::new(
        Envelope::new(1, RecordKind::EvidenceRecord, chrono::Utc::now(), Actor::new("late-agent", RoleId::parse("dev").unwrap())),
        Some(TaskId::parse("s9-fixture#3").unwrap()),
        None,
        None,
        EvidenceVerdict::Faithful,
    ))
    .unwrap();

    let outcome = check_report(&in_, &report_path).unwrap();
    assert_eq!(outcome, CheckOutcome::Drift);
}

#[test]
fn check_reports_no_drift_after_the_commit_that_adds_the_report_itself_moves_head() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let report_path = dir.path().join("REPORT.md");
    let in_ = inputs(dir.path());

    // The P1 regression this guards: a committed `canon/REPORT.md` can
    // never contain the hash of the commit that adds it (that commit's
    // hash is a function of the report's own bytes). If the digest
    // header ever regressed to embedding `git rev-parse HEAD`, this
    // test would fail — HEAD moves the moment the report is committed,
    // permanently drifting every subsequent `--check` at unchanged
    // inputs.
    let git = |args: &[&str]| {
        let status = std::process::Command::new("git").args(args).current_dir(dir.path()).status().unwrap();
        assert!(status.success(), "git {args:?} failed");
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "s9-fixture@example.com"]);
    git(&["config", "user.name", "S9 Fixture"]);

    write_report(&in_, &report_path).unwrap();

    // Commit EVERYTHING, including the just-written report — HEAD
    // moves from "unborn" to a real sha derived from the report's own
    // bytes.
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "add generated report"]);

    let outcome = check_report(&in_, &report_path).unwrap();
    assert_eq!(
        outcome,
        CheckOutcome::NoDrift,
        "a committed report must show no drift on unchanged inputs, even though HEAD moved when the report itself was committed"
    );
}
