//! The design doc's own S2 acceptance bar, verbatim: "write/read/age
//! round-trip across all three tiers in fixtures; layout violations
//! detected; DuckDB views open against a fixture corpus" — task 5.2's
//! `canon selftest`-shaped fixture corpus with rebindable roots.
//!
//! Exercises a REAL local Postgres (via `tests/support::LocalPg` — a
//! genuinely local, zero-network, unix-socket-only, ephemeral cluster;
//! see that module's doc comment for why this is not the same thing as
//! the `live-pg` Cargo feature's hosted-Postgres-specific gating) alongside `GitTier`
//! and `R2Tier`, then opens `sql/views.sql` via the real `duckdb` CLI
//! against the same fixture roots. BOTH external dependencies
//! (`initdb`/`pg_ctl` and the `duckdb` binary) are detected and this
//! test skips cleanly — never fails — when either is absent, so
//! `cargo test --workspace` stays green on a machine lacking them.

mod support;

use canon_model::envelope::{Actor, Envelope, RecordKind};
use canon_model::handoff::{DomainId, HandoffBody};
use canon_model::ids::{ChangeId, HandoffId, RoleId, RunId};
use canon_model::records::{Change, ChangeStatus, Trajectory};
use canon_model::Handoff;
use canon_store::git_tier::GitTier;
use canon_store::pg_tier::PgTier;
use canon_store::policy::TierPolicy;
use canon_store::r2_tier::R2Tier;
use canon_store::registry::TierRegistry;
use canon_store::tier::TierQuery;
use chrono::Utc;
use support::LocalPg;

fn actor() -> Actor {
    Actor::new("e2e-test", RoleId::parse("implementer").unwrap())
}

fn duckdb_available() -> bool {
    std::process::Command::new("duckdb").arg("--version").output().is_ok()
}

/// Seeds a zero-row placeholder parquet file (matching
/// `canon-learn`'s own `ParquetStrategyStore`/`ParquetTrajectoryStore`
/// 5-column encoding, verified against `crates/canon-learn/src/store/
/// parquet_{strategy,trajectory}.rs`, 2026-07-11) under `<learn_root>/
/// {strategies,trajectories}` so `sql/views.sql`'s S9-added
/// `stg_strategy_items`/`stg_trajectories` (`read_parquet`-backed,
/// which — unlike `read_text` — hard-errors on a zero-file glob) never
/// aborts THIS test's `mart_records_by_kind` query, which does not
/// exercise those views at all. This test has no `canon-learn`
/// dependency; it builds the empty parquet file directly via `arrow`/
/// `parquet` (already this crate's own dependencies) rather than
/// adding one just for a placeholder.
fn seed_empty_learn_root(learn_root: &std::path::Path) {
    use arrow::array::{ArrayRef, StringArray};
    use arrow::datatypes::{DataType, Field, Schema};
    use arrow::record_batch::RecordBatch;
    use parquet::arrow::ArrowWriter;
    use std::sync::Arc;

    let schema = Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("regime_key", DataType::Utf8, false),
        Field::new("role", DataType::Utf8, false),
        Field::new("recorded_at", DataType::Utf8, false),
        Field::new("body", DataType::Utf8, false),
    ]));
    for sub in ["strategies", "trajectories"] {
        let path = learn_root.join(sub).join("_seed").join("_seed").join("_seed").join("_seed").join("_seed.parquet");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let empty: ArrayRef = Arc::new(StringArray::from(Vec::<&str>::new()));
        let batch = RecordBatch::try_new(schema.clone(), vec![empty.clone(), empty.clone(), empty.clone(), empty.clone(), empty]).unwrap();
        let file = std::fs::File::create(&path).unwrap();
        let mut writer = ArrowWriter::try_new(file, schema.clone(), None).unwrap();
        writer.write(&batch).unwrap();
        writer.close().unwrap();
    }
}

#[test]
fn write_age_query_and_duckdb_views_round_trip_across_all_three_tiers() {
    let Some(pg) = LocalPg::try_start() else {
        eprintln!("skipping: initdb/pg_ctl not found on PATH — no local Postgres available for this E2E test");
        return;
    };
    if !duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }

    let git_dir = tempfile::tempdir().unwrap();
    let r2_dir = tempfile::tempdir().unwrap();

    // A policy mirroring the shipped `canon.yaml` shape closely enough
    // to be representative: `change` -> local (git), `handoff` ->
    // hot (postgres, ages to cold after 30d), `trajectory` -> cold
    // (s3) directly.
    let yaml = r#"
tiers:
  local: { backend: git, root: .canon/ledger }
  hot:       { backend: postgres, dsn_env: CANON_PG_DSN_E2E, schema: canon_v1_e2e }
  cold:      { backend: s3, bucket_env: CANON_R2_BUCKET_E2E, prefix: "canon/" }
routing:
  change: local
  handoff: hot
  trajectory: cold
aging:
  handoff: { after: 30d, to: cold }
"#;
    let policy = TierPolicy::from_yaml(yaml).unwrap();
    let git = GitTier::new(git_dir.path());
    let pg_tier = PgTier::connect(&pg.dsn(), "canon_v1_e2e").expect("connect to the local ephemeral Postgres");
    let r2 = R2Tier::local(r2_dir.path(), "canon/").unwrap();
    let registry = TierRegistry::new(policy, Some(git), Some(pg_tier), Some(r2), None);

    // 1. One git-tier kind.
    let change = Change::new(
        Envelope::new(1, RecordKind::Change, Utc::now(), actor()),
        ChangeId::parse("s2-tiered-storage").unwrap(),
        "S2",
        "tiered storage",
        ChangeStatus::InProgress,
    );
    registry.persist(&change).expect("persist change (git)");

    // 2. One pg-tier kind, timestamped 60 days in the past so it
    // qualifies for `aging.handoff.after: 30d`.
    let handoff = Handoff::new(
        Envelope::new(1, RecordKind::Handoff, Utc::now() - chrono::Duration::days(60), actor()),
        HandoffId::parse("20260510-1200-e2e-fixture-abcd").unwrap(),
        uuid::Uuid::new_v4(),
        None,
        1,
        "E2E fixture handoff",
        None,
        HandoffBody { domain: DomainId::parse("기획").unwrap(), template_version: 1, fields: serde_json::json!({}) },
    );
    registry.persist(&handoff).expect("persist handoff (pg)");

    // 3. One r2-tier kind.
    let trajectory =
        Trajectory::new(Envelope::new(1, RecordKind::Trajectory, Utc::now(), actor()), RunId::new(), None, None, None, None, Some(0.8));
    registry.persist(&trajectory).expect("persist trajectory (r2)");

    // Before aging: the handoff is readable (from pg).
    let before = registry.query(&TierQuery::kind(RecordKind::Handoff)).unwrap();
    assert_eq!(before.records.len(), 1);

    // Age: the 60-day-old handoff crosses the 30d threshold and moves
    // pg -> r2.
    let reports = registry.age_all().unwrap();
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].kind, RecordKind::Handoff);
    assert_eq!(reports[0].moved, 1);

    // Idempotence: re-running ages nothing new (already gone from pg).
    let reports2 = registry.age_all().unwrap();
    assert_eq!(reports2[0].moved, 0);
    assert_eq!(reports2[0].already_aged, 0, "nothing left in pg to re-select");

    // After aging: still readable via the SAME `canon query` call —
    // merged transparently from its new tier (unified-query spec).
    let after = registry.query(&TierQuery::kind(RecordKind::Handoff)).unwrap();
    assert_eq!(after.records.len(), 1, "the aged handoff must still be readable post-move");
    assert!(after.violations.is_empty());

    let changes = registry.query(&TierQuery::kind(RecordKind::Change)).unwrap();
    assert_eq!(changes.records.len(), 1);
    let trajectories = registry.query(&TierQuery::kind(RecordKind::Trajectory)).unwrap();
    assert_eq!(trajectories.records.len(), 1);

    // Open the DuckDB views against the SAME fixture roots and confirm
    // `mart_records_by_kind` matches the corpus's known content: one
    // `change` in git, one `handoff` now in r2 (post-aging), one
    // `trajectory` in r2.
    let views_sql = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("sql/views.sql");
    let learn_dir = tempfile::tempdir().unwrap();
    seed_empty_learn_root(learn_dir.path());
    let query = "SELECT kind, source_tier, n FROM mart_records_by_kind ORDER BY kind, source_tier;";
    let output = std::process::Command::new("duckdb")
        .arg("-init")
        .arg(&views_sql)
        .arg("-csv")
        .arg("-c")
        .arg(query)
        .env("CANON_GIT_ROOT", git_dir.path())
        .env("CANON_R2_ROOT", r2_dir.path().join("canon"))
        .env("CANON_LEARN_ROOT", learn_dir.path())
        .output()
        .expect("run duckdb -init sql/views.sql");

    assert!(output.status.success(), "duckdb exited non-zero: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("change,git,1"), "expected change/git/1 row in mart_records_by_kind, got:\n{stdout}");
    assert!(stdout.contains("handoff,r2,1"), "expected handoff/r2/1 (post-aging) row, got:\n{stdout}");
    assert!(stdout.contains("trajectory,r2,1"), "expected trajectory/r2/1 row, got:\n{stdout}");
}
