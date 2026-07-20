//! [`ReportError`]: the one error type every fallible step in this
//! crate returns — `report()`/`check()`'s callers (the future
//! `canon-cli` `canon report` arm, part2) match on this, never a
//! per-module error type.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to build parquet seed file: {0}")]
    Seed(String),

    #[error("`duckdb` binary not found on PATH — install DuckDB to run `canon report` (https://duckdb.org)")]
    DuckDbMissing,

    #[error("duckdb exited non-zero running the view layer: {stderr}")]
    QueryFailed { stderr: String },

    #[error("duckdb -json output was not valid JSON: {0}")]
    MalformedJson(#[from] serde_json::Error),

    #[error("report path {path:?} MISSING — run `canon report` first, then `--check` verifies freshness at the same inputs")]
    ReportMissing { path: PathBuf },

    #[error("failed to serialize `manifest.json`: {0}")]
    ManifestJson(String),
}
