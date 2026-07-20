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
