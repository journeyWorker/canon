//! `canon fmt --check <corpus-root>` (S11 task 2.1): the CLI surface
//! over `canon_fmt::check` — no logic lives here beyond formatting and
//! the exit-code decision (nonzero on any violation, mirroring a
//! linter's own `--check` convention).

use std::path::Path;

use canon_fmt::FmtReport;

pub fn run(root: &Path) -> FmtReport {
    canon_fmt::check(root)
}

pub fn format_human(report: &FmtReport) -> String {
    report.format_human()
}

/// The `--json` data for a [`FmtReport`]: the violations are
/// `{class, path, message}`.
pub fn format_json(report: &FmtReport) -> serde_json::Value {
    let violations: Vec<serde_json::Value> = report
        .violations
        .iter()
        .map(|v| serde_json::json!({ "class": v.class.as_str(), "path": v.path.display().to_string(), "message": v.detail }))
        .collect();
    serde_json::json!({ "clean": report.is_clean(), "filesChecked": report.files_checked, "violations": violations })
}
