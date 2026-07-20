//! Session⋈Run⋈Handoff reachability (data-stores Pattern 2 — "Second-
//! hop join keys (session ↔ run, session ↔ handoff) must be minted,
//! typed, and reachable — they are NOT currently wired anywhere",
//! the donor data-stores adoption brief §Pattern 2). `sql/views.sql`'s
//! `mart_session_costs` already proves Session⋈Run
//! resolves in one call; this test proves the FULL Session⋈Run⋈Handoff
//! triple resolves in one call too (`mart_session_run_handoff`) —
//! exactly the donor's own unclosed gap this crate closes.
//!
//! Mirrors `tests/e2e_write_age_query_duckdb.rs`'s established shape
//! (write via a real `canon-store` tier, then open `sql/views.sql`
//! through the real `duckdb` CLI against the same fixture roots) but
//! keeps every record on the git tier — no local Postgres needed —
//! plus one throwaway r2-tier record purely so `stg_r2_records`'
//! `read_parquet` glob (which, unlike `read_text`, hard-errors on a
//! zero-file match) has at least one file to find. Skips cleanly
//! (never fails) when the `duckdb` CLI is absent, matching that same
//! established convention.

use canon_model::envelope::{Actor, Envelope, RecordKind};
use canon_model::handoff::{DomainId, HandoffBody};
use canon_model::ids::{ChangeId, HandoffId, RoleId, RunId, SessionId};
use canon_model::records::{Change, ChangeStatus, Run, RunStatus, Session};
use canon_model::Handoff;
use canon_store::git_tier::GitTier;
use canon_store::r2_tier::R2Tier;
use canon_store::tier::Tier;
use chrono::Utc;

fn actor() -> Actor {
    Actor::new("e2e-test", RoleId::parse("implementer").unwrap())
}

fn duckdb_available() -> bool {
    std::process::Command::new("duckdb").arg("--version").output().is_ok()
}

/// Seeds a zero-row placeholder parquet file under `<learn_root>/
/// {strategies,trajectories}` so `sql/views.sql`'s `stg_strategy_items`/
/// `stg_trajectories` (`read_parquet`-backed, which hard-errors on a
/// zero-file glob — DuckDB resolves each view's output schema at
/// `-init` bind time, before this test's own query even runs) never
/// aborts loading the init script, even though this test's own query
/// never touches those two views. Duplicated from
/// `tests/e2e_write_age_query_duckdb.rs::seed_empty_learn_root`
/// (private to that file, each integration-test file its own crate) —
/// same 5-column `id`/`regime_key`/`role`/`recorded_at`/`body` encoding
/// `crates/canon-learn/src/store/{parquet_strategy,parquet_trajectory}.rs`
/// use.
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
fn session_run_handoff_resolve_together_through_mart_session_run_handoff() {
    if !duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }

    let git_dir = tempfile::tempdir().unwrap();
    let r2_dir = tempfile::tempdir().unwrap();

    let git = GitTier::new(git_dir.path());
    let session_id = SessionId::parse("e2e-join-session-0001").unwrap();

    // 1. A Session record — the `sessions` CTE's own `session_id`.
    let session = Session::new(Envelope::new(1, RecordKind::Session, Utc::now(), actor()), session_id.clone(), "claude-code", Utc::now(), None);
    git.write(&session).expect("persist session");

    // 2. A Run record sharing the SAME session_id — the `mart_session_costs`
    // half this view reuses verbatim.
    let run_id = RunId::new();
    let run = Run::new(Envelope::new(1, RecordKind::Run, Utc::now(), actor()), run_id, Some(session_id.clone()), None, RunStatus::Succeeded, Utc::now(), Some(Utc::now()));
    git.write(&run).expect("persist run");

    // 3. A Handoff record whose ENVELOPE actor carries the same
    // session_id (view header comment: `Handoff` has no dedicated
    // `session_id` field of its own today — `actor.session_id` is the
    // one currently-available, honest join key).
    let handoff_id = HandoffId::parse("20260710-1200-e2e-join-fixture-a1b2").unwrap();
    let handoff_actor = actor().with_session(session_id.clone());
    let handoff = Handoff::new(
        Envelope::new(1, RecordKind::Handoff, Utc::now(), handoff_actor),
        handoff_id.clone(),
        uuid::Uuid::new_v4(),
        None,
        1,
        "E2E join-view fixture handoff",
        None,
        HandoffBody { domain: DomainId::parse("기획").unwrap(), template_version: 1, fields: serde_json::json!({}) },
    );
    git.write(&handoff).expect("persist handoff");

    // Throwaway r2-tier record: NOT part of the join under test, exists
    // only so `stg_r2_records`' `read_parquet('.../kind=*/**/*.parquet')`
    // glob has at least one real file to find (module doc).
    let r2 = R2Tier::local(r2_dir.path(), "canon/").unwrap();
    let filler = Change::new(Envelope::new(1, RecordKind::Change, Utc::now(), actor()), ChangeId::parse("e2e-join-filler").unwrap(), "S2", "r2 glob filler", ChangeStatus::InProgress);
    r2.write(&filler).expect("persist r2 filler record");

    let learn_dir = tempfile::tempdir().unwrap();
    seed_empty_learn_root(learn_dir.path());

    let views_sql = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("sql/views.sql");
    let query = "SELECT session_id, client, run_id, run_status, handoff_id, handoff_state, handoff_title FROM mart_session_run_handoff ORDER BY session_id;";
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

    let expected_row = format!(
        "{},claude-code,{},succeeded,{},pending,E2E join-view fixture handoff",
        session_id.as_str(),
        run_id,
        handoff_id.as_str()
    );
    assert!(stdout.contains(&expected_row), "expected Session⋈Run⋈Handoff joined row `{expected_row}`, got:\n{stdout}");
}
