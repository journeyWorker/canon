//! `canon report --snapshot <dir>` (design D3, tasks.md 3.3; s24
//! extended this to 6 marts, s36 to 7): asserts the SHARED SNAPSHOT
//! CONTRACT both `canon-report` (the writer) and `packages/dashboard`
//! (the reader) build to — locked over IRC with the dashboard's own
//! fixture-owning sibling change, verified fresh against
//! `crates/canon-store/sql/views.sql` by both sides independently:
//! exactly 7 parquet files (filenames byte-identical to table names) +
//! one `manifest.json` declaring exactly those `{table, file}` pairs in
//! the report's own panel order, and each parquet file's own column set
//! matches the mart's `views.sql` `SELECT` list name-for-name, in
//! order. A schema drift between the writer and the dashboard's fixture
//! must fail HERE, not silently diverge.
mod support;

use canon_report::{snapshot, ReportInputs};
use parquet::file::reader::{FileReader, SerializedFileReader};

/// `(table, [declared columns in views.sql SELECT order])` — the
/// locked contract (Main's directive: "same column set per mart" is a
/// TEST FAILURE, not a local-only pass).
const EXPECTED_CONTRACT: &[(&str, &[&str])] = &[
    ("mart_trust_matrix", &["task_id", "change_id", "title", "task_status", "covered", "green", "who", "evidence_count", "latest_at"]),
    (
        "mart_session_costs",
        &["session_id", "client", "role", "workspace_label", "run_count", "total_cost", "total_tokens", "first_event_at", "last_event_at"],
    ),
    (
        "mart_role_memory",
        &["role", "regime_key", "strategy_count", "active_count", "demoted_count", "hit_rate", "avg_source_trajectories", "latest_recorded_at"],
    ),
    ("mart_flywheel_funnel", &["role", "verdicts", "distilled", "retrieved", "applied"]),
    (
        "mart_review_burndown",
        &["day", "evidence_faithful", "evidence_divergent", "evidence_not_applicable", "divergence_opened", "divergence_resolved", "divergence_open_running_total"],
    ),
    ("mart_scope_status", &["task_id", "scenario_id", "task_status", "evidence_covered", "green", "spec_covered"]),
    ("mart_subjects", &["domain", "subject_id", "title", "status", "scenario_count", "covered_scenarios"]),
];

fn parquet_columns(path: &std::path::Path) -> Vec<String> {
    let file = std::fs::File::open(path).unwrap();
    let reader = SerializedFileReader::new(file).unwrap();
    reader.metadata().file_metadata().schema().get_fields().iter().map(|f| f.name().to_string()).collect()
}

#[test]
fn snapshot_writes_seven_parquet_files_and_a_manifest_listing_exactly_them() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let roots = support::corpus::build(dir.path());
    let inputs = ReportInputs::new(dir.path(), roots);
    let out_dir = dir.path().join("snapshot-out");

    let manifest = snapshot(&inputs, &out_dir).unwrap();

    // Exactly the 7 contracted tables, in the report's declared order
    // — never a superset/subset, never reordered.
    assert_eq!(manifest.tables.len(), 7, "manifest.tables must list exactly 7 marts, got {:?}", manifest.tables);
    for (entry, (table, _columns)) in manifest.tables.iter().zip(EXPECTED_CONTRACT) {
        assert_eq!(entry.table, *table);
        assert_eq!(entry.file, format!("{table}.parquet"), "filename must be byte-identical to the table name (design D3)");
    }

    // "exactly them": no stray parquet files in the output dir beyond
    // the 7 declared ones.
    let parquet_files: Vec<_> = std::fs::read_dir(&out_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".parquet"))
        .collect();
    assert_eq!(parquet_files.len(), 7, "expected exactly 7 parquet files, found {parquet_files:?}");

    // manifest.json on disk round-trips to the identical declared shape.
    let manifest_path = out_dir.join("manifest.json");
    assert!(manifest_path.is_file());
    let text = std::fs::read_to_string(&manifest_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(parsed["generated_at"].is_string());
    assert!(parsed["source_git_sha"].is_string());
    assert!(parsed["source_digest"].is_string());
    let tables = parsed["tables"].as_array().unwrap();
    assert_eq!(tables.len(), 7);

    // The CONTRACT: per-mart column set (name + order) matches
    // views.sql's own SELECT list exactly — a writer/reader schema
    // drift is a test failure here, not a silent divergence discovered
    // only in the dashboard.
    for (table, expected_columns) in EXPECTED_CONTRACT {
        let path = out_dir.join(format!("{table}.parquet"));
        assert!(path.is_file(), "missing {table}.parquet");
        let columns = parquet_columns(&path);
        assert_eq!(&columns, expected_columns, "{table}'s exported parquet columns must match views.sql exactly (name + order)");
    }
}

/// Regression test for a `COPY "<table>" TO '<path>' (FORMAT parquet)`
/// SQL-injection-shaped bug: an unescaped single quote INSIDE the
/// destination path would terminate the `TO '<path>'` string literal
/// early and corrupt the statement. `snapshot::export_view` now
/// SQL-escapes the path (doubling every `'`) before embedding it —
/// this proves the full `--snapshot` run still succeeds, writing all
/// 7 parquet files + manifest.json, when the destination directory
/// itself contains an apostrophe.
#[test]
fn snapshot_into_a_directory_whose_path_contains_an_apostrophe_succeeds() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let roots = support::corpus::build(dir.path());
    let inputs = ReportInputs::new(dir.path(), roots);
    // The apostrophe-bearing path segment IS the point of this test.
    let out_dir = dir.path().join("s9'snap");

    let manifest = snapshot(&inputs, &out_dir).unwrap();

    assert_eq!(manifest.tables.len(), 7, "manifest.tables must list exactly 7 marts, got {:?}", manifest.tables);
    for (entry, (table, _columns)) in manifest.tables.iter().zip(EXPECTED_CONTRACT) {
        assert_eq!(entry.table, *table);
        let parquet_path = out_dir.join(&entry.file);
        assert!(parquet_path.is_file(), "missing {} at {}", entry.file, parquet_path.display());
    }
    let parquet_files: Vec<_> = std::fs::read_dir(&out_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".parquet"))
        .collect();
    assert_eq!(parquet_files.len(), 7, "expected exactly 7 parquet files, found {parquet_files:?}");
    assert!(out_dir.join("manifest.json").is_file());
}
