//! `canon report --snapshot <dir>` (design D3, tasks.md 3.3/3.4): one
//! explicit `COPY "<table>" TO '<table>.parquet' (FORMAT parquet)` per
//! exported mart — never `EXPORT DATABASE`, which mis-escapes
//! digit-containing table names (D3's own cited donor bug) — plus a
//! single `manifest.json` ([`crate::manifest::Manifest`]). This is a
//! straight `COPY` of the DuckDB views `crates/canon-store/sql/
//! views.sql` already computed (design D1: no second aggregation
//! layer) — the exported columns are exactly each view's own `SELECT`
//! list, never a Rust-side projection (unlike [`crate::marts`]'s
//! curated markdown-rendering column subset).
//!
//! All ten `COPY` statements run in ONE `duckdb` process over ONE
//! pinned read of the corpus ([`crate::query::run_pinned_command`]),
//! so the exported files are ten views of a single input. A snapshot
//! is what `packages/dashboard` renders, and the dashboard repeats the
//! report's claim that a per-change total and the per-round rows it
//! totals cannot disagree — which would be false of ten files
//! exported by ten processes against a ledger still being written.

use std::path::Path;

use crate::digest::DigestHeader;
use crate::error::ReportError;
use crate::manifest::{git_head_sha, Manifest, ManifestTable};
use crate::query;
use crate::ReportInputs;

/// The ten S9/S24/S36/s43-owned marts, in the order the report declares
/// them — [`crate::marts::REPORT_MARTS`]'s own order, duplicated here
/// as a bare name list (`packages/dashboard/test/panel-copy.test.ts`
/// parses this literal out of the source, so it stays a literal;
/// `marts`' own `report_marts_are_the_snapshot_tables` pins the two
/// together). `mart_scope_status`, `mart_subjects`, `mart_review_rounds`
/// and `mart_review_totals` remain appended in their historical order;
/// `mart_run_observability` is appended last so existing consumers'
/// table order never moves.
pub const SNAPSHOT_TABLES: &[&str] = &[
    "mart_trust_matrix",
    "mart_session_costs",
    "mart_role_memory",
    "mart_flywheel_funnel",
    "mart_review_burndown",
    "mart_scope_status",
    "mart_subjects",
    "mart_review_rounds",
    "mart_review_totals",
    "mart_run_observability",
];

/// Escapes a path for embedding inside a single-quoted DuckDB SQL
/// string literal: doubles every `'` (`'` -> `''`), DuckDB's own
/// SQL-standard string-literal escape — verified against a real
/// `duckdb` run, 2026-07-11. Without this, a destination directory
/// containing an apostrophe (e.g. `s9'snap/`) would terminate the
/// `TO '<path>'` literal early and corrupt the `COPY` statement.
fn sql_quote_literal(path: &Path) -> String {
    path.display().to_string().replace('\'', "''")
}

/// The `COPY` statement that exports one DuckDB table/view to
/// `dest` — the single declaration of that statement's shape (D3),
/// used by [`snapshot`] and asserted directly by
/// `tests/snapshot_digit_table_names.rs`. The table name is a QUOTED
/// IDENTIFIER, so a digit-containing name round-trips byte-identically
/// instead of being mangled the way `EXPORT DATABASE` mangles it; the
/// destination is SQL-escaped ([`sql_quote_literal`]), so a path
/// containing an apostrophe never terminates the `TO '<path>'` literal
/// early.
pub fn copy_statement(view: &str, dest: &Path) -> String {
    format!("COPY \"{view}\" TO '{}' (FORMAT parquet);", sql_quote_literal(dest))
}

/// `canon report --snapshot <dir>`'s full run: exports every
/// [`SNAPSHOT_TABLES`] view to `<dir>/<table>.parquet`, then writes
/// `<dir>/manifest.json` declaring exactly those `{table, file}` pairs
/// plus `generated_at`/`source_git_sha`/`source_digest` (design D3).
/// Returns the written [`Manifest`].
///
/// The ten `COPY`s are ONE pinned batch, not ten processes: a
/// snapshot is a set of files a consumer joins across, so two of them
/// disagreeing about the same corpus would be the same defect as two
/// report panels disagreeing (module doc above). `manifest.json`'s
/// `source_digest`/`source_git_sha` are computed after the export,
/// from the repo's own files, and are not part of that pin.
pub fn snapshot(inputs: &ReportInputs, dir: &Path) -> Result<Manifest, ReportError> {
    std::fs::create_dir_all(dir)?;

    let mut tables = Vec::with_capacity(SNAPSHOT_TABLES.len());
    let mut script = String::new();
    for view in SNAPSHOT_TABLES {
        let file = format!("{view}.parquet");
        script.push_str(&copy_statement(view, &dir.join(&file)));
        script.push('\n');
        tables.push(ManifestTable { table: (*view).to_string(), file });
    }
    query::run_pinned_command(&inputs.roots, &script)?;

    let digest = DigestHeader::compute(&inputs.repo_root, &inputs.roots.git_root)?;
    let manifest = Manifest {
        generated_at: chrono::Utc::now(),
        source_git_sha: git_head_sha(&inputs.repo_root),
        source_digest: digest.combined_digest(),
        tables,
    };

    let manifest_json = serde_json::to_string_pretty(&manifest).map_err(|e| ReportError::ManifestJson(e.to_string()))?;
    std::fs::write(dir.join("manifest.json"), manifest_json)?;

    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sql_quote_literal_doubles_every_single_quote() {
        assert_eq!(sql_quote_literal(Path::new("/tmp/s9'snap")), "/tmp/s9''snap");
        assert_eq!(sql_quote_literal(Path::new("/tmp/it's/a'test'")), "/tmp/it''s/a''test''");
    }

    #[test]
    fn sql_quote_literal_is_identity_when_no_quotes_present() {
        assert_eq!(sql_quote_literal(Path::new("/tmp/plain-dir")), "/tmp/plain-dir");
    }
}
