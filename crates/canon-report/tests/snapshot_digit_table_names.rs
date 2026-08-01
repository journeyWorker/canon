//! Regression test for design D3's cited donor bug: `EXPORT DATABASE`
//! mis-escapes digit-containing table names. `crate::snapshot`'s
//! export mechanism never uses `EXPORT DATABASE` — one explicit
//! `COPY "<table>" TO '<table>.parquet' (FORMAT parquet)` per table
//! instead (tasks.md 3.4). This test exercises the EXACT statement
//! `snapshot::copy_statement` builds — the same function `snapshot()`
//! itself calls, so there is no second spelling of the shape to drift
//! from — against a digit-containing table name, proving the resulting
//! filename is byte-identical to the table name, never an
//! escaped/altered variant.

mod support;

use canon_report::query::run_command;
use canon_report::snapshot::copy_statement;

#[test]
fn a_digit_containing_table_name_exports_with_a_byte_identical_filename() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let roots = support::corpus::build(dir.path());
    let out_dir = dir.path().join("snapshot-out");
    std::fs::create_dir_all(&out_dir).unwrap();

    // A table name containing digits — the exact shape D3 calls out
    // (`EXPORT DATABASE` historically mis-escaped these). Created +
    // exported in ONE `duckdb` session (each `run_command` invocation
    // is a fresh in-memory database), the `COPY` half built by the
    // production function rather than restated here.
    let table = "mart_2fa_test";
    let dest = out_dir.join(format!("{table}.parquet"));
    let sql = format!("CREATE OR REPLACE TABLE \"{table}\" AS SELECT 1 AS x; {}", copy_statement(table, &dest));

    run_command(&roots, &sql).unwrap();

    assert!(dest.is_file(), "expected byte-identical filename {}, none found — dir contents: {:?}", dest.display(), std::fs::read_dir(&out_dir).unwrap().map(|e| e.unwrap().file_name()).collect::<Vec<_>>());
    // No escaped variant (e.g. a URL-encoded or otherwise altered
    // name) landed alongside it.
    let files: Vec<_> = std::fs::read_dir(&out_dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(files, vec![format!("{table}.parquet")], "no other file should have been written: {files:?}");
}
