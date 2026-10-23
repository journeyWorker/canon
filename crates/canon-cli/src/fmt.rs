//! `canon fmt --check <corpus-root>` (S11 task 2.1): the CLI surface
//! over `canon_fmt::check` — no logic lives here beyond formatting and
//! the exit-code decision (nonzero on any violation, mirroring a
//! linter's own `--check` convention).

use std::path::Path;

use canon_fmt::FmtReport;
use schemars::JsonSchema;
use serde::Serialize;

pub fn run(root: &Path) -> FmtReport {
    canon_fmt::check(root)
}

pub fn format_human(report: &FmtReport) -> String {
    report.format_human()
}

/// One format violation as `--json` shows it.
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ViolationEntry<'a> {
    /// The failure class (`missing-provenance`, …).
    pub class: &'static str,
    pub path: String,
    pub message: &'a str,
}

impl<'a> ViolationEntry<'a> {
    pub fn of(violation: &'a canon_fmt::Violation) -> Self {
        Self { class: violation.class.as_str(), path: violation.path.display().to_string(), message: &violation.detail }
    }
}

/// `canon format`'s report (`FormatReport`).
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FormatReport<'a> {
    pub clean: bool,
    pub files_checked: usize,
    pub violations: Vec<ViolationEntry<'a>>,
    /// Present (true) when nothing was examined, so a clean result is not a pass.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub unverifiable: bool,
}

/// The `--json` data for a [`FmtReport`]; `unverifiable` when nothing was examined.
pub fn format_json(report: &FmtReport, unverifiable: bool) -> FormatReport<'_> {
    FormatReport { clean: report.is_clean() && !unverifiable, files_checked: report.files_checked, violations: report.violations.iter().map(ViolationEntry::of).collect(), unverifiable }
}

impl crate::json_out::Report for FormatReport<'_> {}
