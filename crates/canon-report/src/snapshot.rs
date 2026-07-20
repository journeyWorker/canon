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

use std::path::{Path, PathBuf};

use crate::digest::DigestHeader;
use crate::error::ReportError;
use crate::manifest::{git_head_sha, Manifest, ManifestTable};
use crate::query;
use crate::ReportInputs;

/// The S9/S24/S36-owned marts, in the order the report declares them —
/// [`crate::render::ReportMarts`]'s own field order, duplicated here as
/// a bare name list (this module never renders markdown, so it does not
/// depend on [`crate::render`]). `mart_scope_status` (s20
/// `task-scenario-join`, surfaced by s24 `scope-status-report`) then
/// `mart_subjects` (s36 `subject-domain-loop`) are appended LAST, after
/// the original five.
pub const SNAPSHOT_TABLES: &[&str] = &[
    "mart_trust_matrix",
    "mart_session_costs",
    "mart_role_memory",
    "mart_flywheel_funnel",
    "mart_review_burndown",
    "mart_scope_status",
    "mart_subjects",
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

/// Exports one DuckDB table/view to `<dest_dir>/<view>.parquet` via a
/// single quoted-identifier `COPY` statement (D3) — returns the
/// written file's path. `dest_dir` is created first (idempotent). The
/// destination path is SQL-escaped ([`sql_quote_literal`]) before
/// embedding in the `TO '<path>'` literal, so a path containing an
/// apostrophe never corrupts the statement (covered by the
/// apostrophe-path integration test in `tests/snapshot.rs`, and by
/// this module's own `sql_quote_literal` unit tests below).
pub fn export_view(roots: &crate::Roots, view: &str, dest_dir: &Path) -> Result<PathBuf, ReportError> {
    std::fs::create_dir_all(dest_dir)?;
    let dest = dest_dir.join(format!("{view}.parquet"));
    let sql = format!("COPY \"{view}\" TO '{}' (FORMAT parquet);", sql_quote_literal(&dest));
    query::run_command(roots, &sql)?;
    Ok(dest)
}

/// `canon report --snapshot <dir>`'s full run: exports every
/// [`SNAPSHOT_TABLES`] view to `<dir>/<table>.parquet`, then writes
/// `<dir>/manifest.json` declaring exactly those `{table, file}` pairs
/// plus `generated_at`/`source_git_sha`/`source_digest` (design D3).
/// Returns the written [`Manifest`].
pub fn snapshot(inputs: &ReportInputs, dir: &Path) -> Result<Manifest, ReportError> {
    std::fs::create_dir_all(dir)?;

    let mut tables = Vec::with_capacity(SNAPSHOT_TABLES.len());
    for view in SNAPSHOT_TABLES {
        export_view(&inputs.roots, view, dir)?;
        tables.push(ManifestTable { table: (*view).to_string(), file: format!("{view}.parquet") });
    }

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
