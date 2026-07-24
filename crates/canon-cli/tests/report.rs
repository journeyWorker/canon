//! Integration tests for `canon report [--repo <dir>] [--check]
//! [--snapshot <dir>]` (S9 part2, `s9-unified-surface`, tasks.md 3.1),
//! invoking the actually-built `canon` binary
//! (`env!("CARGO_BIN_EXE_canon")`) — matching `tests/retrieve.rs`/
//! `tests/context.rs`/`tests/gate.rs`'s own discipline: pure logic
//! (`--repo`/`Roots` resolution) is already unit-tested inside
//! `src/report.rs` itself; this file covers the real-process boundary
//! — exit codes and stdout/stderr against a repo the binary itself
//! writes to and re-reads.
//!
//! Every test runs against a completely fresh, empty repo (no
//! `canon.yaml`, no git/r2/learn roots pre-seeded) — `canon-report`'s
//! own `fresh_repo.rs` test already proves report generation over a
//! corpus this empty succeeds with all-empty panels, so this file
//! never needs to build a fixture corpus of its own to exercise the
//! CLI-arm wiring (repo resolution, exit codes, file placement).

use std::path::Path;
use std::process::{Command, Output};

fn run_canon(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(cwd).output().expect("spawn canon binary")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn duckdb_available() -> bool {
    std::process::Command::new("duckdb").arg("--version").output().is_ok()
}

#[test]
fn report_writes_canon_report_md_under_the_resolved_repo_root() {
    if !duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();

    let output = run_canon(&["report", "--repo", "."], dir.path());

    assert!(output.status.success(), "canon report must exit 0; stderr: {}", stderr(&output));
    assert!(stdout(&output).contains(".canon/REPORT.md"), "{}", stdout(&output));
    let report_path = dir.path().join(".canon/REPORT.md");
    assert!(report_path.is_file());
    let content = std::fs::read_to_string(&report_path).unwrap();
    assert!(content.starts_with("# canon report\n"));
}

#[test]
fn check_exit_codes_track_missing_no_drift_and_drift_across_the_full_lifecycle() {
    if !duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();

    // MISSING — no report has been written yet.
    let missing = run_canon(&["report", "--repo", ".", "--check"], dir.path());
    assert_eq!(missing.status.code(), Some(1), "MISSING must exit 1; stderr: {}", stderr(&missing));
    assert!(stderr(&missing).contains("MISSING"), "{}", stderr(&missing));

    // Write it.
    let write = run_canon(&["report", "--repo", "."], dir.path());
    assert!(write.status.success(), "stderr: {}", stderr(&write));

    // NoDrift — immediately after a write, over an unchanged corpus.
    let no_drift = run_canon(&["report", "--repo", ".", "--check"], dir.path());
    assert_eq!(no_drift.status.code(), Some(0), "no-drift must exit 0; stderr: {}", stderr(&no_drift));
    assert!(stderr(&no_drift).contains("no drift"), "{}", stderr(&no_drift));

    // Drift — hand-edit the committed report.
    let report_path = dir.path().join(".canon/REPORT.md");
    let mut content = std::fs::read_to_string(&report_path).unwrap();
    content.push_str("\nhand-edited, never generated\n");
    std::fs::write(&report_path, content).unwrap();

    let drift = run_canon(&["report", "--repo", ".", "--check"], dir.path());
    assert_eq!(drift.status.code(), Some(1), "DRIFT must exit 1; stderr: {}", stderr(&drift));
    assert!(stderr(&drift).contains("DRIFT"), "{}", stderr(&drift));
}

#[test]
fn snapshot_writes_seven_parquet_files_and_a_manifest_under_the_given_dir() {
    if !duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let snapshot_dir = dir.path().join("snap-out");

    let output = run_canon(&["report", "--repo", ".", "--snapshot", snapshot_dir.to_str().unwrap()], dir.path());

    assert!(output.status.success(), "canon report --snapshot must exit 0; stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("7 table(s)"), "{}", stdout(&output));

    // s36 `subject-domain-loop`: `mart_subjects` is exported LAST,
    // after `mart_scope_status` (SubjectSurface's `SNAPSHOT_TABLES`
    // order).
    for table in ["mart_trust_matrix", "mart_session_costs", "mart_role_memory", "mart_flywheel_funnel", "mart_review_burndown", "mart_scope_status", "mart_subjects"] {
        assert!(snapshot_dir.join(format!("{table}.parquet")).is_file(), "missing {table}.parquet");
    }
    let manifest_path = snapshot_dir.join("manifest.json");
    assert!(manifest_path.is_file());
    let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    assert_eq!(manifest["tables"].as_array().unwrap().len(), 7);

    // `--snapshot` never wrote/touched `.canon/REPORT.md` — it is a
    // distinct action from the default write mode.
    assert!(!dir.path().join(".canon/REPORT.md").exists());
}

#[test]
fn report_help_smoke() {
    let output = run_canon(&["report", "--help"], Path::new("."));
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let text = stdout(&output);
    assert!(text.contains("--repo"));
    assert!(text.contains("--check"));
    assert!(text.contains("--snapshot"));
}
