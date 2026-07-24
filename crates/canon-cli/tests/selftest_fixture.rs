//! Wires S9's fixture snapshot into a reproducible selftest
//! (`s9-unified-surface` tasks.md 7.2, design.md §8's "fixture corpora
//! with rebindable roots + an EXPECTED-output diff; `canon selftest`
//! runs all fixtures and diffs" testing strategy, applied to S9).
//!
//! S9's three fixture corpora already exist and are already exercised
//! by `cargo test -p canon-report` / `bun test` in `packages/dashboard`
//! — the report-marts corpus (`crates/canon-report/fixtures/corpus.rs`,
//! consumed by `tests/{marts,byte_stability,check_gate}.rs`, task 2.6),
//! the snapshot manifest fixture (`packages/dashboard/fixtures/
//! snapshot/`, consumed by `test/smoke.test.ts`, task 5.6), and the
//! rollup-endpoint stub (a hand-rolled `TcpListener` server in
//! canon-report's tests, task 4.4). What was NOT yet
//! automated — task 7.2's own "not a one-off manual check" — is the
//! ONE cross-check task 5.3's own evidence recorded as a MANUAL step:
//! "Cross-verified against S9bRust's real `canon report --snapshot`
//! output ... schemas byte-diffed identical ... manual browser
//! verification". [`fresh_snapshot_matches_the_committed_dashboard_fixture_contract`]
//! below is that exact cross-check, now a `cargo test -p canon-cli`
//! assertion: it regenerates a REAL `canon report --snapshot` and
//! diffs its manifest table list/order plus every table's own Parquet
//! column set against the committed dashboard fixture — the SHARED
//! SNAPSHOT CONTRACT task 3.3's evidence describes as "locked over IRC
//! with the `packages/dashboard` sibling change".
//!
//! [`report_and_snapshot_share_a_stable_digest_across_independent_runs`]
//! covers task 7.2's other named half — "`canon report --check`'s
//! byte-stability ... covered by the standard selftest run" — from a
//! angle no existing test exercises: `canon report`'s drift-checked
//! markdown header and `canon report --snapshot`'s `manifest.json`
//! `source_digest` are computed from the exact same three input
//! digests ([`canon_report::digest::DigestHeader::combined_digest`]);
//! this proves that shared digest is itself reproducible/stable across
//! independent invocations against unchanged input — precisely the
//! property `--check`'s byte-diff gate depends on.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use canon_model::{Actor, Envelope, EvidenceRecord, EvidenceVerdict, RecordKind, RoleId, TaskId};
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use chrono::Utc;
use serde::Deserialize;

fn run_canon(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(cwd).output().expect("spawn canon binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn duckdb_available() -> bool {
    Command::new("duckdb").arg("--version").output().is_ok()
}

/// `crates/canon-cli` -> `crates` -> the real canon repo root, whose
/// committed `packages/dashboard/fixtures/snapshot` is this file's
/// EXPECTED oracle (module doc).
fn canon_repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
}

#[derive(Debug, Deserialize)]
struct ManifestTable {
    table: String,
    file: String,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    source_digest: String,
    tables: Vec<ManifestTable>,
}

fn read_manifest(dir: &Path) -> Manifest {
    let text = std::fs::read_to_string(dir.join("manifest.json")).unwrap_or_else(|e| panic!("read {}: {e}", dir.join("manifest.json").display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", dir.join("manifest.json").display()))
}

/// Every declared table's own Parquet column names, in order — shells
/// out to `duckdb -json -c "DESCRIBE SELECT * FROM read_parquet(...)"`,
/// the same real `duckdb` CLI boundary every other query in this
/// workspace goes through (never a second, untested Parquet-reading
/// dependency — mirrors `canon_report::query`'s own module doc
/// reasoning).
fn parquet_columns(file: &Path) -> Vec<String> {
    let output = Command::new("duckdb")
        .args(["-json", "-c", &format!("DESCRIBE SELECT * FROM read_parquet('{}')", file.display())])
        .output()
        .unwrap_or_else(|e| panic!("spawn duckdb DESCRIBE over {}: {e}", file.display()));
    assert!(output.status.success(), "duckdb DESCRIBE {} failed: {}", file.display(), String::from_utf8_lossy(&output.stderr));
    let rows: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("duckdb -json DESCRIBE output for {}: {e}", file.display()));
    rows.iter().map(|row| row["column_name"].as_str().expect("column_name field").to_string()).collect()
}

/// Task 7.2's core selftest (module doc): a FRESH `canon report
/// --snapshot` must match the committed dashboard fixture's own SHARED
/// SNAPSHOT CONTRACT — same table set, same order, same per-table
/// column names (name-for-name, in order). A `crates/canon-store/sql/
/// views.sql` change that drifts a mart's column list — or any future
/// S9bRust/S9bDash contract disagreement — fails HERE, automatically,
/// instead of only being caught by a one-off manual cross-check.
///
/// Generated over a completely FRESH, EMPTY repo (`tests/report.rs`'s
/// own "every test runs against a completely fresh, empty repo"
/// discipline) — the exported Parquet SCHEMA comes from each view's
/// `SELECT` list, unaffected by row count, so an empty corpus proves
/// the contract exactly as well as a populated one while staying fully
/// isolated from this shared worktree's own `canon/` scratch state
/// (other sibling agents may be writing to the real repo's `canon/
/// ledger`/`.canon/learn`/`.canon/r2` concurrently).
#[test]
fn fresh_snapshot_matches_the_committed_dashboard_fixture_contract() {
    if !duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let repo = canon_repo_root();
    let fixture_dir = repo.join("packages/dashboard/fixtures/snapshot");
    let fixture_manifest = read_manifest(&fixture_dir);
    assert_eq!(fixture_manifest.tables.len(), 7, "the committed dashboard fixture itself must declare exactly 7 tables (s36 added mart_subjects)");

    let fresh_repo = tempfile::tempdir().unwrap();
    let snapshot_dir = fresh_repo.path().join("snap-out");
    let output = run_canon(&["report", "--repo", ".", "--snapshot", snapshot_dir.to_str().unwrap()], fresh_repo.path());
    assert!(output.status.success(), "canon report --snapshot must succeed over a fresh empty repo; stderr: {}", stderr(&output));

    let fresh_manifest = read_manifest(&snapshot_dir);

    let fixture_tables: Vec<&str> = fixture_manifest.tables.iter().map(|t| t.table.as_str()).collect();
    let fresh_tables: Vec<&str> = fresh_manifest.tables.iter().map(|t| t.table.as_str()).collect();
    assert_eq!(fresh_tables, fixture_tables, "a fresh snapshot's table list/order must match the committed dashboard fixture's SHARED SNAPSHOT CONTRACT exactly");

    for (fixture_table, fresh_table) in fixture_manifest.tables.iter().zip(fresh_manifest.tables.iter()) {
        let fixture_columns = parquet_columns(&fixture_dir.join(&fixture_table.file));
        let fresh_columns = parquet_columns(&snapshot_dir.join(&fresh_table.file));
        assert_eq!(
            fresh_columns, fixture_columns,
            "`{}`'s column set must match the committed dashboard fixture's contract name-for-name, in order — a views.sql drift must fail here, not only via a manual dashboard cross-check",
            fixture_table.table
        );
    }
}

fn write_evidence(ledger_root: &Path, task_id: &str) {
    let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), Actor::new("selftest-agent", RoleId::parse("implementer").unwrap()));
    let record = EvidenceRecord::new(envelope, Some(TaskId::parse(task_id).unwrap()), None, None, EvidenceVerdict::Faithful);
    GitTier::new(ledger_root).write(&record).expect("write evidence record");
}

/// Task 7.2's other named half (module doc): `canon report`'s
/// drift-checked markdown header and `canon report --snapshot`'s
/// `manifest.json` `source_digest` are the SAME combined digest
/// (`DigestHeader::combined_digest`, `crates/canon-report/src/
/// digest.rs`) over the SAME three inputs — proves that shared digest
/// is reproducible across two independent `canon report --snapshot`
/// invocations against UNCHANGED input, which is exactly the property
/// `canon report --check`'s byte-diff gate relies on (a rendering is
/// byte-stable iff its digest inputs are stable). Seeds one real
/// evidence record + `.canon/policy.yaml` first so the digest is a real
/// hash, not the empty-corpus `—` placeholder — a stronger proof than
/// the trivially-stable empty case above.
#[test]
fn report_and_snapshot_share_a_stable_digest_across_independent_runs() {
    if !duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".canon")).unwrap();
    std::fs::write(dir.path().join(".canon/policy.yaml"), "risk_routing:\n  reviewer: true\n").unwrap();
    write_evidence(&dir.path().join(".canon/ledger"), "selftest-change#1");

    // `canon report --check` is byte-stable immediately after a write
    // (design D2) — the half of task 7.2 already covered end-to-end by
    // `tests/report.rs`, re-asserted here over the SAME seeded repo the
    // digest cross-check below also uses, so both halves of task 7.2's
    // acceptance text run over one shared fixture.
    let write = run_canon(&["report", "--repo", "."], dir.path());
    assert!(write.status.success(), "stderr: {}", stderr(&write));
    let check = run_canon(&["report", "--repo", ".", "--check"], dir.path());
    assert_eq!(check.status.code(), Some(0), "no-drift must exit 0 immediately after a write; stderr: {}", stderr(&check));

    // Two independent --snapshot runs against the SAME unchanged input.
    let snap_a = dir.path().join("snap-a");
    let out_a = run_canon(&["report", "--repo", ".", "--snapshot", snap_a.to_str().unwrap()], dir.path());
    assert!(out_a.status.success(), "{}", stdout(&out_a));
    let manifest_a = read_manifest(&snap_a);

    let snap_b = dir.path().join("snap-b");
    let out_b = run_canon(&["report", "--repo", ".", "--snapshot", snap_b.to_str().unwrap()], dir.path());
    assert!(out_b.status.success(), "{}", stdout(&out_b));
    let manifest_b = read_manifest(&snap_b);

    assert_ne!(manifest_a.source_digest, "—", "a repo with real evidence + policy must never digest to the empty-corpus placeholder");
    assert_eq!(manifest_a.source_digest, manifest_b.source_digest, "source_digest must be byte-stable across two independent --snapshot runs over unchanged input — the same property canon report --check's byte-diff gate depends on");
}
