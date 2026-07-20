//! `canon report --check`'s drift gate (design D2, tasks.md 3.2) —
//! lifted near-verbatim from the donor parity harness's `cmd_report`
//! (verified against the donor source directly, 2026-07-11): regenerate in memory,
//! byte-diff against the existing file. No existing file → `Missing`
//! (exit 1, "generate first"). Byte match → `NoDrift` (exit 0). Byte
//! mismatch → `Drift` (exit 1, "regenerate with `canon report`").

use std::path::Path;

use crate::error::ReportError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckOutcome {
    /// `report_path` does not exist yet — parity.py's own MISSING case.
    Missing,
    /// The freshly regenerated content byte-matches the existing file.
    NoDrift,
    /// The freshly regenerated content differs from the existing file.
    Drift,
}

impl CheckOutcome {
    /// `0` only for [`CheckOutcome::NoDrift`] — both `Missing` and
    /// `Drift` are failing exit codes (parity.py `cmd_report`: both
    /// branches `return 1`).
    pub fn exit_code(self) -> i32 {
        match self {
            CheckOutcome::NoDrift => 0,
            CheckOutcome::Missing | CheckOutcome::Drift => 1,
        }
    }

    /// The stderr-shaped message a CLI caller prints — parity.py's own
    /// three literal messages, adapted to `canon report`'s naming.
    pub fn message(self, report_path: &Path) -> String {
        let path = report_path.display();
        match self {
            CheckOutcome::Missing => {
                format!("canon report --check: {path} MISSING — run `canon report` first, then `--check` verifies freshness at the same inputs")
            }
            CheckOutcome::NoDrift => "canon report --check: no drift".to_string(),
            CheckOutcome::Drift => format!("canon report --check: DRIFT — {path} is stale; regenerate with `canon report`"),
        }
    }
}

/// Byte-diffs `generated` (a freshly rendered report) against whatever
/// currently exists at `report_path`. Pure I/O + comparison — the
/// caller ([`crate::check`] via [`crate::check_report`]) is responsible
/// for calling [`crate::report`] first; this function never re-derives
/// anything itself.
pub fn check(report_path: &Path, generated: &str) -> Result<CheckOutcome, ReportError> {
    if !report_path.is_file() {
        return Ok(CheckOutcome::Missing);
    }
    let existing = std::fs::read_to_string(report_path)?;
    Ok(if existing == generated { CheckOutcome::NoDrift } else { CheckOutcome::Drift })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_when_report_path_does_not_exist() {
        let dir = tempfile::tempdir().unwrap();
        let outcome = check(&dir.path().join("REPORT.md"), "content").unwrap();
        assert_eq!(outcome, CheckOutcome::Missing);
        assert_eq!(outcome.exit_code(), 1);
    }

    #[test]
    fn no_drift_on_byte_identical_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("REPORT.md");
        std::fs::write(&path, "content\n").unwrap();
        let outcome = check(&path, "content\n").unwrap();
        assert_eq!(outcome, CheckOutcome::NoDrift);
        assert_eq!(outcome.exit_code(), 0);
    }

    #[test]
    fn drift_on_any_byte_difference() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("REPORT.md");
        std::fs::write(&path, "old content\n").unwrap();
        let outcome = check(&path, "new content\n").unwrap();
        assert_eq!(outcome, CheckOutcome::Drift);
        assert_eq!(outcome.exit_code(), 1);
    }
}
