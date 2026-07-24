//! Acceptance (s25 `report-pg-tier-boundary` spec.md, s27
//! `tier-role-backend-split` design D2, s28 `rung-backend-capability`
//! design D2/D3): `canon report`'s stderr `WARN` line for kinds
//! routed to a rung whose backend is not read directly by the
//! report, invoking the actually-built `canon` binary — matching
//! `tests/report.rs`'s own discipline. Every fixture here is a bare,
//! empty repo (report generation over an empty corpus already
//! succeeds per `canon-report`'s own `fresh_repo.rs` test); this file
//! only adds a `canon.yaml` to exercise the backend-capability-derived
//! WARN/note.

use std::path::Path;
use std::process::{Command, Output};

fn run_canon(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(cwd).output().expect("spawn canon binary")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn duckdb_available() -> bool {
    std::process::Command::new("duckdb").arg("--version").output().is_ok()
}

fn write_canon_yaml(dir: &Path, text: &str) {
    std::fs::write(dir.join("canon.yaml"), text).unwrap();
}

#[test]
fn multi_tier_repo_emits_a_warn_line_matching_the_reports_own_note() {
    if !duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n  task: hot\n  session: hot\n  event: hot\n  change: local\n",
    );

    let output = run_canon(&["report", "--repo", "."], dir.path());

    assert!(output.status.success(), "canon report must still exit 0; stderr: {}", stderr(&output));
    let err = stderr(&output);
    assert!(err.contains("canon report: WARN"), "{err}");
    for kind in ["task", "session", "event"] {
        assert!(err.contains(kind), "WARN line must name `{kind}`: {err}");
    }
    assert!(!err.contains("change"), "WARN must never name a git-routed kind: {err}");

    let report_path = dir.path().join(".canon/REPORT.md");
    let content = std::fs::read_to_string(&report_path).unwrap();
    assert!(content.contains("## Kinds not read directly"), "{content}");
    for kind in ["task", "session", "event"] {
        assert!(content.contains(&format!("`{kind}`")), "REPORT.md must name `{kind}`:\n{content}");
    }
}

#[test]
fn git_only_repo_emits_no_warn_line() {
    if !duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    // No canon.yaml at all — the existing fixture-corpus default every
    // other test in this crate already uses.

    let output = run_canon(&["report", "--repo", "."], dir.path());

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(!stderr(&output).contains("WARN"), "a git-only repo must emit no WARN line: {}", stderr(&output));
}
