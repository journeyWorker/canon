//! `mart_scope_status` (task-scenario-join spec, design D3): one query
//! answers "is this scope DONE (checkbox), VERIFIED (evidence-covered),
//! and SPEC-COVERED (scenario-covered)" for a declared `(task_id,
//! scenario_id)` pair — unifying `mart_trust_matrix`'s evidence-
//! PRESENCE `covered` against `porting.coverage`'s spec-AUTHORSHIP
//! `covered`, joined structurally through `Task.scenario_refs` /
//! `int_task_scenario_refs`.
//!
//! Mirrors `tests/e2e_session_run_handoff_join_duckdb.rs`'s established
//! shape (write via a real `canon-store` `GitTier`, then open
//! `sql/views.sql` through the real `duckdb` CLI against the same
//! fixture root) — no local Postgres needed; one throwaway r2-tier
//! record is still seeded purely so `stg_r2_records`' `read_parquet`
//! glob (which hard-errors on a zero-file match) has at least one file
//! to find. Skips cleanly (never fails) when the `duckdb` CLI is
//! absent, matching that same established convention.

use canon_model::envelope::{Actor, Envelope, RecordKind};
use canon_model::evidence::RawRecord;
use canon_model::ids::{RoleId, ScenarioId, TaskId};
use canon_model::records::{EvidenceRecord, EvidenceVerdict, Task, TaskStatus};
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use chrono::Utc;

fn actor() -> Actor {
    Actor::new("e2e-test", RoleId::parse("implementer").unwrap())
}

fn duckdb_available() -> bool {
    std::process::Command::new("duckdb").arg("--version").output().is_ok()
}

/// A well-formed `porting.coverage`-shaped overlay body — this crate's
/// own tests never depend on `canon-plugin`, so this is a plain
/// hand-built JSON body, exactly mirroring `git_tier.rs`'s own
/// private test helper of the same name/shape.
fn overlay_body(project_id: &str, scenario_id: &str, covered: bool) -> RawRecord {
    RawRecord(serde_json::json!({
        "schema": 1,
        "kind": "porting.coverage",
        "at": Utc::now().to_rfc3339(),
        "actor": {"agent_id": "porting-sync", "role": "implementer"},
        "project_id": project_id,
        "scenario_id": scenario_id,
        "covered": covered,
    }))
}

#[test]
fn a_declared_covered_evidenced_scenario_resolves_fully_green_in_one_query() {
    if !duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }

    let git_dir = tempfile::tempdir().unwrap();
    let git = GitTier::new(git_dir.path());

    // A Task declaring TWO scenario refs: one with both evidence AND a
    // porting.coverage overlay row (fully green), one with NO overlay
    // row at all (the "unauthored scenario" gap case).
    let task_id = TaskId::parse("e2e-join-change#1.1").unwrap();
    let covered_scenario = ScenarioId::parse("e2e.join.01").unwrap();
    let uncovered_scenario = ScenarioId::parse("e2e.join.02").unwrap();
    let task = Task::new(Envelope::new(1, RecordKind::Task, Utc::now(), actor()), task_id.clone(), "wire the join fixture", TaskStatus::Done, None)
        .with_scenario_refs(vec![covered_scenario.clone(), uncovered_scenario.clone()]);
    git.write(&task).expect("persist task");

    // Evidence-VERIFIED half: a Faithful EvidenceRecord for the task_id
    // — the exact `mart_trust_matrix` half this view reuses verbatim.
    let evidence = EvidenceRecord::new(Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), actor()), Some(task_id.clone()), None, None, EvidenceVerdict::Faithful);
    git.write(&evidence).expect("persist evidence");

    // Spec-COVERED half: a `porting.coverage` overlay row for ONLY the
    // covered scenario — the uncovered scenario deliberately has none.
    let overlay = overlay_body("e2e-join-project", covered_scenario.as_str(), true);
    git.write_namespaced("porting.coverage", &format!("e2e-join-project__{}", covered_scenario.as_str()), overlay).expect("persist porting.coverage overlay");

    let views_sql = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("sql/views.sql");
    let r2_dir = tempfile::tempdir().unwrap();
    let learn_dir = tempfile::tempdir().unwrap();
    seed_empty_learn_root(learn_dir.path());
    seed_empty_r2_root(r2_dir.path());

    let run_query = |query: &str| -> String {
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
        String::from_utf8_lossy(&output.stdout).to_string()
    };

    // ── the fully-green row: DONE + VERIFIED + SPEC-COVERED, one query ──
    let covered_row = run_query(&format!(
        "SELECT task_status, evidence_covered, green, spec_covered FROM mart_scope_status WHERE task_id = '{task_id}' AND scenario_id = '{covered_scenario}';"
    ));
    assert!(covered_row.contains("done,true,true,true"), "expected a fully-green scope-status row, got:\n{covered_row}");

    // ── the unauthored-scenario row: spec_covered is honestly NULL, never a dropped row or invented false ──
    let uncovered_present = run_query(&format!("SELECT count(*) FROM mart_scope_status WHERE task_id = '{task_id}' AND scenario_id = '{uncovered_scenario}';"));
    assert!(uncovered_present.trim_end().ends_with('1'), "the declared-but-unauthored scenario must still surface a row, got:\n{uncovered_present}");
    let uncovered_spec_covered = run_query(&format!(
        "SELECT spec_covered IS NULL FROM mart_scope_status WHERE task_id = '{task_id}' AND scenario_id = '{uncovered_scenario}';"
    ));
    assert!(uncovered_spec_covered.contains("true"), "spec_covered must be honestly NULL for an unauthored scenario, got:\n{uncovered_spec_covered}");

    // ── a Task with no scenario_refs is absent from the join mart but unaffected in mart_trust_matrix ──
    let unrelated_task_id = TaskId::parse("e2e-join-change#1.2").unwrap();
    let unrelated_task = Task::new(Envelope::new(1, RecordKind::Task, Utc::now(), actor()), unrelated_task_id.clone(), "an ordinary covers-free task", TaskStatus::Open, None);
    git.write(&unrelated_task).expect("persist covers-free task");
    let join_mart_absent = run_query(&format!("SELECT count(*) FROM mart_scope_status WHERE task_id = '{unrelated_task_id}';"));
    assert!(join_mart_absent.trim_end().ends_with('0'), "a covers-free task must produce zero rows in mart_scope_status, got:\n{join_mart_absent}");
    let trust_matrix_present = run_query(&format!("SELECT count(*) FROM mart_trust_matrix WHERE task_id = '{unrelated_task_id}';"));
    assert!(trust_matrix_present.trim_end().ends_with('1'), "the covers-free task must still be unaffected in mart_trust_matrix, got:\n{trust_matrix_present}");
}

/// Seeds a zero-row placeholder parquet file under `<learn_root>/
/// {strategies,trajectories}` so `sql/views.sql`'s `stg_strategy_items`/
/// `stg_trajectories` (`read_parquet`-backed, which hard-errors on a
/// zero-file glob) never abort loading the init script. Duplicated from
/// `tests/e2e_session_run_handoff_join_duckdb.rs::seed_empty_learn_root`
/// (private to that file, each integration-test file its own crate).
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

/// `stg_r2_records`' `read_parquet('.../kind=*/**/*.parquet')` glob
/// (unlike `read_text`) hard-errors on a zero-file match — seed a
/// throwaway r2-tier record via [`canon_store::r2_tier::R2Tier`] purely
/// so DuckDB has at least one file to bind the view's schema against.
/// This record is never part of the join under test.
fn seed_empty_r2_root(r2_root: &std::path::Path) {
    use canon_model::ids::ChangeId;
    use canon_model::records::{Change, ChangeStatus};
    use canon_store::r2_tier::R2Tier;

    let r2 = R2Tier::local(r2_root, "canon/").unwrap();
    let filler = Change::new(Envelope::new(1, RecordKind::Change, Utc::now(), actor()), ChangeId::parse("e2e-scope-status-filler").unwrap(), "S2", "r2 glob filler", ChangeStatus::InProgress);
    r2.write(&filler).expect("persist r2 filler record");
}
