//! Acceptance: "a record stored as N versions at one natural key
//! contributes to a mart exactly ONCE" — the regression guard for
//! `crates/canon-store/sql/views.sql`'s "Multi-version records" header
//! section, and specifically for the cost double-count the s42
//! re-review found.
//!
//! Every canon tier is APPEND-ONLY at a natural key
//! (`canon_store::partition::resolve_partition`), so a state
//! transition is a SECOND physical row at the same key, not an
//! in-place update. `crate::query`'s Rust fold collapses those
//! versions; `views.sql` does not inherit that fold — `stg_records` is
//! a plain `UNION ALL` — so every mart that joins or aggregates has to
//! fold its own key. Before that fix, `canon dispatch begin`/`end`
//! storing two `Run` versions per dispatch multiplied
//! `mart_session_costs`' `total_cost` AND `total_tokens` by the
//! version count, silently: `run_count` is `count(DISTINCT run_id)`
//! and stayed correct, so a doubled bill read as an expensive run.
//!
//! Deliberately builds standalone per-scenario corpora rather than
//! reusing `fixtures/corpus.rs` — that shared fixture writes exactly
//! ONE version per natural key (which is why it never caught this),
//! and this crate's precedent (`session_costs_multi_workspace.rs`) is
//! one purpose-built corpus per documented scenario.
//!
//! One test is a NEGATIVE guard: `mart_review_burndown` MUST keep
//! reading the raw version stream, because for a divergence the
//! transition IS the curve. It fails if someone "generalizes" the fold
//! into `stg_records` for every reader.
//!
//! The next two tests guard the two ways a fold can be WRONG rather
//! than absent, both of which the s42 gate re-review found in the
//! original fix:
//!
//! - Wrong KEY. `scope_status_reports_each_projects_own_coverage_…`
//!   folds `porting.coverage` at `(project_id, scenario_id)`, its own
//!   declared `join_key`. Keyed `scenario_id` alone, two spec roots
//!   authoring one scenario id MERGE into one row whose `spec_covered`
//!   is an arbitrary pick — one project's coverage reported under
//!   another's name, which is worse than the double-count the fold
//!   replaced.
//! - Wrong ORDER. `trust_matrix_breaks_an_equal_at_task_tie_…` pins
//!   the fold's winner to the one `canon_store::fold_latest_by_key`
//!   picks — greatest `(at, schema, digest)`, not greatest `at`. Equal
//!   `at` is routine, not exotic: a plan-derived record stamps `at`
//!   from its source document's mtime, and `RecordKind::Task`'s
//!   `schema_version` is `2` precisely because a parser change
//!   produced a fresh record tied on `at` with the stale one it
//!   supersedes. That test computes its expectation by CALLING the
//!   Rust fold rather than hardcoding a winner, so it fails on any
//!   future divergence between the two implementations, in either
//!   direction.
//!
//! The last five go rung by rung through the `(at, schema, digest)`
//! triple `version_rank` materializes, on the premise that a rung
//! agreeing by coincidence is a rung that will stop agreeing:
//!
//! - Three of them pin the `at` rung, which is
//!   `canon_store::tier::raw_record_at` — an RFC3339 STRING parsed to
//!   a UTC instant at nanosecond precision — and NOT the `"at"`
//!   column's naive-microsecond `TIMESTAMP` cast of that same text,
//!   which `version_rank` used to compare and which is lossy in three
//!   separate ways. `scope_status_ranks_one_instant_written_in_…`
//!   pins the DISCARDED OFFSET;
//!   `scope_status_ranks_a_one_nanosecond_version_bump_…` pins the
//!   TRUNCATED sub-microsecond precision, at exactly the delta
//!   `canon-cli::dispatch::close_version_at` writes, so it is canon's
//!   own dispatch path rather than an exotic corpus; and
//!   `scope_status_ranks_a_lowercase_rfc3339_stamp_…` pins the PARSE
//!   SURFACE, which is chrono's and is strictly wider than a DuckDB
//!   cast's.
//! - `scope_status_picks_the_schema_version_canon_query_picks_…` pins
//!   the `schema` rung's DOMAIN. `raw_record_schema` narrows through
//!   `u64` to `u32` and floors the rest to `0`; DuckDB's `BIGINT` does
//!   neither, so `schema: 4294967296` — legal for a plugin overlay,
//!   `validate_envelope_shape` admits any JSON integer — used to rank
//!   LAST in Rust and FIRST in SQL.
//! - `both_roots_fold_a_digest_the_rust_reader_refuses_to_return` pins
//!   the `digest` rung's known RESIDUAL, on both roots. It asserts a
//!   divergence rather than absence of one: narrowing either residual
//!   is legitimate, but it has to land together with `views.sql`'s
//!   header note, and this test is how that gets noticed.

mod support;

use canon_model::envelope::{Actor, Envelope, RecordKind};
use canon_model::evidence::RawRecord;
use canon_model::ids::{ChangeId, ProjectId, RoleId, RunId, ScenarioId, Sha, SessionId, SubjectId, TaskId, TotalOrder};
use canon_model::records::{
    Divergence, DivergenceStatus, Event, EvidenceRecord, EvidenceVerdict, Finding, FindingSeverity, Run, RunStatus, Session, Subject, SubjectStatus, Task, TaskStatus,
};
use canon_report::marts;
use canon_report::query;
use canon_report::roots::Roots;
use canon_store::fold_latest_by_key;
use canon_store::git_tier::GitTier;
use canon_store::partition::{content_digest12, resolve_partition};
use canon_store::r2_tier::R2Tier;
use canon_store::tier::{raw_record_at, raw_record_schema, Tier, TierQuery};
use chrono::{DateTime, TimeDelta, TimeZone, Utc};
use serde_json::json;

fn at(y: i32, m: u32, d: u32, h: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, m, d, h, 0, 0).single().unwrap()
}

fn actor(role: &str) -> Actor {
    Actor::new("fold-fixture", RoleId::parse(role).unwrap())
}

/// One session, and ONE run persisted as the two versions `canon
/// dispatch begin`/`end` actually write: `Running` at begin, then
/// `Succeeded` re-stamped at a strictly greater `envelope.at`
/// (`canon-cli`'s `close_version_at`). Both land in the git tier as
/// distinct digest-suffixed objects, which is precisely the shape
/// `stg_records` returns twice.
fn dispatched_run_corpus(git_root: &std::path::Path) {
    let tier = GitTier::new(git_root);
    let session_id = SessionId::parse("fold-session").unwrap();
    tier.write(&Session::new(
        Envelope::new(1, RecordKind::Session, at(2026, 5, 1, 9), actor("dev")),
        session_id.clone(),
        "claude-code",
        at(2026, 5, 1, 9),
        Some(at(2026, 5, 1, 12)),
    ))
    .unwrap();

    let run_id = RunId::new();
    tier.write(&Run::new(
        Envelope::new(1, RecordKind::Run, at(2026, 5, 1, 10), Actor::new_unattributed("claude-code")),
        run_id,
        Some(session_id.clone()),
        None,
        RunStatus::Running,
        at(2026, 5, 1, 10),
        None,
    ))
    .unwrap();
    tier.write(&Run::new(
        Envelope::new(1, RecordKind::Run, at(2026, 5, 1, 11), Actor::new_unattributed("claude-code")),
        run_id,
        Some(session_id.clone()),
        None,
        RunStatus::Succeeded,
        at(2026, 5, 1, 10),
        Some(at(2026, 5, 1, 11)),
    ))
    .unwrap();

    // Exactly ONE `token_usage` event: $0.01 and 15 tokens, spent once.
    tier.write(&Event::new(
        Envelope::new(1, RecordKind::Event, at(2026, 5, 1, 10), Actor::new_unattributed("claude-code")),
        run_id,
        1,
        "token_usage",
        json!({
            "provider_id": "anthropic",
            "workspace_key": "canon",
            "workspace_label": "canon",
            "tokens": {"input": 10, "output": 5, "cache_read": 0, "cache_write": 0, "reasoning": 0, "total": 15},
            "cost": 0.01,
            "cost_source": "api",
        }),
    ))
    .unwrap();
}

#[test]
fn session_costs_charges_a_run_stored_as_two_versions_exactly_once() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let git_root = dir.path().join("ledger");
    dispatched_run_corpus(&git_root);

    let roots = Roots::new(git_root, dir.path().join("r2"), dir.path().join("learn"));
    let result = marts::fetch_session_costs(&roots).unwrap();

    assert_eq!(result.rows.len(), 1, "one session in one workspace must yield exactly one row, got {:?}", result.rows);
    let row = &result.rows[0];

    // `run_count` was ALWAYS right — `count(DISTINCT run_id)` absorbs
    // the duplicate. Asserted anyway so a future fold regression can
    // never be mistaken for a run_count regression.
    assert_eq!(row["run_count"], 1, "one run, however many versions it is stored as");

    // These two are the actual blocker. Unfolded, the `runs` CTE
    // yielded one row per stored VERSION, the `token_usage` join fanned
    // out, and both sums doubled to 0.02 / 30.
    let cost = row["total_cost"].as_f64().unwrap();
    assert!((cost - 0.01).abs() < 1e-9, "a run stored as two versions must be billed ONCE: expected 0.01, got {cost}");
    assert_eq!(row["total_tokens"], 15, "a run stored as two versions must have its tokens counted ONCE");
}

#[test]
fn session_run_handoff_reports_only_the_runs_latest_status() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let git_root = dir.path().join("ledger");
    dispatched_run_corpus(&git_root);

    let roots = Roots::new(git_root, dir.path().join("r2"), dir.path().join("learn"));
    // No `fetch_*` wrapper exists for this mart (it is not a report
    // panel) — read it the way `canon-report` reads every view.
    let rows = query::run_query(&roots, "SELECT session_id, run_id, run_status FROM mart_session_run_handoff ORDER BY run_id;").unwrap();

    // Unfolded this emitted TWO rows for one run, one claiming
    // `running` and one `succeeded` — a view whose whole purpose is
    // answering "what is this triple's state" reporting both answers
    // at once, with no column saying which is current.
    assert_eq!(rows.len(), 1, "one session/run pair must yield exactly one row, got {rows:?}");
    assert_eq!(rows[0]["run_status"], "succeeded", "the surviving row must be the LATEST version, never the superseded `running` one");
}

#[test]
fn subjects_counts_scenarios_once_across_a_status_lifecycle() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let git_root = dir.path().join("ledger");
    let tier = GitTier::new(&git_root);

    let subject_id = SubjectId::parse("fold-subject").unwrap();
    let covered = ScenarioId::parse("fold.subject.01").unwrap();
    let uncovered = ScenarioId::parse("fold.subject.02").unwrap();

    // The full `canon subject status` walk: FIVE versions at one
    // `subject_id`, which is the ordinary end state of any shipped
    // subject, not an exotic corpus.
    for (hour, status) in [
        (9, SubjectStatus::Proposed),
        (10, SubjectStatus::Specced),
        (11, SubjectStatus::Building),
        (12, SubjectStatus::Verifying),
        (13, SubjectStatus::Shipped),
    ] {
        tier.write(
            &Subject::new(
                Envelope::new(1, RecordKind::Subject, at(2026, 5, 2, hour), actor("planner")),
                subject_id.clone(),
                "folded subject",
                "one product unit, five stored versions",
                "dev",
                status,
                RoleId::parse("dev").unwrap(),
            )
            .with_links(vec![], vec![covered.clone(), uncovered.clone()]),
        )
        .unwrap();
    }

    tier.write(&EvidenceRecord::new(
        Envelope::new(1, RecordKind::EvidenceRecord, at(2026, 5, 2, 14), actor("dev")),
        None,
        Some(covered.clone()),
        None,
        EvidenceVerdict::Faithful,
    ))
    .unwrap();

    let roots = Roots::new(git_root, dir.path().join("r2"), dir.path().join("learn"));
    let result = marts::fetch_subjects(&roots).unwrap();

    assert_eq!(result.rows.len(), 1, "one subject must yield exactly one row however many statuses it walked, got {:?}", result.rows);
    let row = &result.rows[0];
    assert_eq!(row["status"], "shipped", "the row must carry the LATEST status, not an arbitrary superseded one");
    // Unfolded, `subject_scenarios` fanned out five-fold: 2 linked
    // scenarios read as 10, and the 1 covered one read as 5.
    assert_eq!(row["scenario_count"], 2, "the subject links two scenarios, not two-per-stored-version");
    assert_eq!(row["covered_scenarios"], 1, "exactly one of them carries a non-divergent verdict");
}

#[test]
fn trust_matrix_and_scope_status_yield_one_row_per_task_after_a_checkbox_flip() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let git_root = dir.path().join("ledger");
    let tier = GitTier::new(&git_root);

    let task_id = TaskId::parse("fold-change#1").unwrap();
    let scenario_id = ScenarioId::parse("fold.task.01").unwrap();

    // `canon gate task` flipping a checkbox appends a second `task`
    // version at the same `task_id`.
    for (hour, status) in [(9, TaskStatus::Open), (10, TaskStatus::Done)] {
        tier.write(
            &Task::new(
                Envelope::new(1, RecordKind::Task, at(2026, 5, 3, hour), actor("planner")),
                task_id.clone(),
                "folded task",
                status,
                None,
            )
            .with_scenario_refs(vec![scenario_id.clone()]),
        )
        .unwrap();
    }

    tier.write(&EvidenceRecord::new(
        Envelope::new(1, RecordKind::EvidenceRecord, at(2026, 5, 3, 11), actor("dev")),
        Some(task_id.clone()),
        None,
        None,
        EvidenceVerdict::Faithful,
    ))
    .unwrap();

    let roots = Roots::new(git_root, dir.path().join("r2"), dir.path().join("learn"));

    let trust = marts::fetch_trust_matrix(&roots).unwrap();
    assert_eq!(trust.rows.len(), 1, "one task must yield exactly one trust-matrix row, got {:?}", trust.rows);
    assert_eq!(trust.rows[0]["task_status"], "done", "the row must carry the flipped status, not the superseded `open` one");

    // `mart_scope_status` joined TWO unfolded task relations
    // (`int_task_scenario_refs` and `mart_trust_matrix`), so it was
    // QUADRATIC in versions: one declared pair rendered as four rows,
    // two of them claiming a `task_status` the plan no longer holds.
    let scope = marts::fetch_scope_status(&roots).unwrap();
    assert_eq!(scope.rows.len(), 1, "one declared (task_id, scenario_id) pair must yield exactly one row, got {:?}", scope.rows);
    assert_eq!(scope.rows[0]["task_status"], "done");
}

#[test]
fn review_burndown_still_reads_every_divergence_version_as_its_own_event() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let git_root = dir.path().join("ledger");
    let tier = GitTier::new(&git_root);

    // ONE divergence, opened day 1 and resolved day 3 — two versions at
    // the same natural key (`project__scenario__run_seq__round`).
    for (day, status) in [(1, DivergenceStatus::Open), (3, DivergenceStatus::Resolved)] {
        tier.write(&Divergence::new(
            Envelope::new(1, RecordKind::Divergence, at(2026, 5, day, 12), actor("reviewer")),
            ProjectId::parse("root").unwrap(),
            ScenarioId::parse("fold.diverge.01").unwrap(),
            Sha::parse("c".repeat(40)).unwrap(),
            status,
            TotalOrder::new(1),
            1,
            "reviewer1",
            "one divergence, opened then resolved",
        ))
        .unwrap();
    }

    let roots = Roots::new(git_root, dir.path().join("r2"), dir.path().join("learn"));
    let result = marts::fetch_review_burndown(&roots).unwrap();

    // This is the case a single global `stg_records` fold would break:
    // folding to the latest version drops the `+1` and leaves only the
    // `-1`, driving the running total NEGATIVE. The burn-down is on
    // `views.sql`'s RAW-STREAM list for exactly this reason.
    assert_eq!(result.rows.len(), 2, "the open day and the resolve day are two distinct points on the curve, got {:?}", result.rows);
    assert_eq!(result.rows[0]["divergence_opened"], 1);
    assert_eq!(result.rows[0]["divergence_resolved"], 0);
    assert_eq!(result.rows[0]["divergence_open_running_total"], 1, "opened, not yet resolved");
    assert_eq!(result.rows[1]["divergence_opened"], 0);
    assert_eq!(result.rows[1]["divergence_resolved"], 1);
    assert_eq!(result.rows[1]["divergence_open_running_total"], 0, "burnt down to zero — never negative");
}

/// The one scenario id TWO different spec roots both author coverage
/// for. `porting.coverage`'s `join_key` is `(project_id, scenario_id)`
/// (`.canon/plugins/porting/plugin.yaml`), so the two overlay rows
/// below are DISTINCT records at distinct natural keys — never two
/// versions of one record.
const SHARED_SCENARIO: &str = "fold.cov.01";

/// A well-formed `porting.coverage` overlay body — hand-built JSON
/// rather than a typed record, exactly as `fixtures/corpus.rs` does it,
/// because an overlay kind has no `canon-model` type at all.
fn coverage_overlay(project_id: &str, scenario_id: &str, covered: bool) -> RawRecord {
    coverage_overlay_at_schema(project_id, scenario_id, covered, 1)
}

/// [`coverage_overlay`] with the envelope `schema` left OPEN. An
/// overlay kind has no `RecordKind::schema_version()` narrowing the
/// field to a `u32`, and `canon_model::evidence::
/// validate_envelope_shape` admits any `is_u64() || is_i64()` integer,
/// so a value outside `u32` is a legal stored record here — which is
/// exactly the domain the fold rungs have to agree about.
fn coverage_overlay_at_schema(project_id: &str, scenario_id: &str, covered: bool, schema: u64) -> RawRecord {
    coverage_overlay_at_stamp(project_id, scenario_id, covered, schema, &at(2026, 5, 5, 11).to_rfc3339())
}

/// [`coverage_overlay_at_schema`] with the envelope `at` left OPEN, as
/// a raw STRING. A typed record's `at` is a `DateTime<Utc>` and can
/// only ever serialize as `…+00:00`, but `canon_model::evidence::
/// validate_envelope_shape` admits any string
/// `DateTime::parse_from_rfc3339` accepts — a strictly wider surface
/// (non-zero offsets, lowercase `t`/`z`, a space separator,
/// nanoseconds). A hand-authored or imported overlay body is where
/// that width is reachable, and it is exactly the domain the `at` rung
/// has to agree about.
fn coverage_overlay_at_stamp(project_id: &str, scenario_id: &str, covered: bool, schema: u64, stamp: &str) -> RawRecord {
    RawRecord(json!({
        "schema": schema,
        "kind": "porting.coverage",
        "at": stamp,
        "actor": {"agent_id": "porting-sync", "role": "implementer"},
        "project_id": project_id,
        "scenario_id": scenario_id,
        "covered": covered,
        "surface_ref": [],
    }))
}

#[test]
fn scope_status_reports_each_projects_own_coverage_for_a_shared_scenario_id() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let git_root = dir.path().join("ledger");
    let tier = GitTier::new(&git_root);

    let task_id = TaskId::parse("fold-cov#1").unwrap();
    tier.write(
        &Task::new(
            Envelope::new(RecordKind::Task.schema_version(), RecordKind::Task, at(2026, 5, 5, 9), actor("planner")),
            task_id.clone(),
            "coverage fan-out task",
            TaskStatus::Done,
            None,
        )
        .with_scenario_refs(vec![ScenarioId::parse(SHARED_SCENARIO).unwrap()]),
    )
    .unwrap();

    // Two spec roots, ONE version each, disagreeing about coverage. The
    // disagreement is the point: it makes an arbitrary winner visible,
    // where two `true` rows would hide it.
    for (project_id, covered) in [("alpha", true), ("beta", false)] {
        let natural_key = format!("{project_id}__{}", SHARED_SCENARIO);
        tier.write_namespaced("porting.coverage", &natural_key, coverage_overlay(project_id, SHARED_SCENARIO, covered)).unwrap();
    }

    let roots = Roots::new(git_root, dir.path().join("r2"), dir.path().join("learn"));
    let scope = marts::fetch_scope_status(&roots).unwrap();

    // Folded at `scenario_id` alone this was ONE row — the two overlay
    // rows collapsed into a single group and `arg_max` returned
    // whichever the scan reached first, so `spec_covered` reported one
    // project's answer with nothing naming the project it came from.
    assert_eq!(scope.rows.len(), 2, "one row per COVERING PROJECT for the shared scenario id, got {:?}", scope.rows);

    let spec_covered_for = |project_id: &str| {
        scope
            .rows
            .iter()
            .find(|r| r.get("spec_project_id").and_then(|v| v.as_str()) == Some(project_id))
            .unwrap_or_else(|| panic!("no row reports {project_id}'s own coverage: {:?}", scope.rows))["spec_covered"]
            .clone()
    };
    assert_eq!(spec_covered_for("alpha"), json!(true), "alpha authored covered = true");
    assert_eq!(spec_covered_for("beta"), json!(false), "beta authored covered = false, and must not read alpha's answer");

    for row in &scope.rows {
        assert_eq!(row["task_id"], "fold-cov#1");
        assert_eq!(row["scenario_id"], SHARED_SCENARIO);
        assert_eq!(row["task_status"], "done", "the task side is a single version and must be unaffected by the coverage fan-out");
    }
}

#[test]
fn trust_matrix_breaks_an_equal_at_task_tie_the_same_way_canon_query_does() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let git_root = dir.path().join("ledger");
    let tier = GitTier::new(&git_root);

    let task_id = TaskId::parse("fold-tie#1").unwrap();
    let scenario_id = ScenarioId::parse("fold.tie.01").unwrap();
    // The SAME `at` for both versions — a `Task`'s `at` is
    // `file_modified_at(<source plan doc>)` (s20 D7), and a canon PARSER
    // change does not touch that mtime. `Task` is the kind whose
    // `schema_version` was bumped to `2` for exactly this collision, so
    // this is that kind's real history, not a contrived corpus.
    let tied_at = at(2026, 5, 6, 9);

    // Generation 1: pre-s37, no `depends_on`.
    tier.write(
        &Task::new(Envelope::new(1, RecordKind::Task, tied_at, actor("planner")), task_id.clone(), "generation tie task", TaskStatus::Open, None)
            .with_scenario_refs(vec![scenario_id.clone()]),
    )
    .unwrap();
    // Generation 2: the current one, carrying the field the bump exists
    // for, at the IDENTICAL `at`.
    tier.write(
        &Task::new(
            Envelope::new(RecordKind::Task.schema_version(), RecordKind::Task, tied_at, actor("planner")),
            task_id.clone(),
            "generation tie task",
            TaskStatus::Done,
            None,
        )
        .with_scenario_refs(vec![scenario_id.clone()])
        .with_depends_on(vec![TaskId::parse("fold-tie#0").unwrap()]),
    )
    .unwrap();

    // The expectation is COMPUTED by the Rust fold, never hardcoded:
    // this test's whole claim is "the view agrees with
    // `canon_store::fold_latest_by_key`", so hardcoding a winner would
    // let the two drift apart while the test still passed. Same
    // `(key, at, schema, digest)` extraction `canon-cli::query`'s
    // `fold_latest_by_natural_key` performs.
    struct Candidate {
        key: String,
        at: DateTime<Utc>,
        schema: u32,
        digest: String,
        status: String,
    }
    let stored = tier.read(&TierQuery::kind(RecordKind::Task)).unwrap();
    assert!(stored.violations.is_empty(), "both generations must store cleanly: {:?}", stored.violations);
    assert_eq!(stored.records.len(), 2, "two generations at one task_id must be two physical objects, got {:?}", stored.records);

    let candidates: Vec<Candidate> = stored
        .records
        .iter()
        .map(|record| Candidate {
            key: resolve_partition(RecordKind::Task, &record.0).unwrap().natural_key,
            at: raw_record_at(record),
            schema: raw_record_schema(record),
            digest: content_digest12(&record.0),
            status: record.0["status"].as_str().expect("a stored Task always carries a string status").to_string(),
        })
        .collect();

    // The fixture property that makes this test a GUARD rather than a
    // coincidence: the STALE generation's digest sorts first, so it is
    // also the first file a filename-ordered scan reaches. An `at`-only
    // fold therefore retains it and reports `open`. If a future
    // `Task` field flips this ordering, retune the fixture (vary the
    // title) — otherwise the test would still pass while no longer
    // distinguishing the two fold rules.
    let stale = candidates.iter().find(|c| c.schema == 1).expect("generation 1 is stored");
    let fresh = candidates.iter().find(|c| c.schema == RecordKind::Task.schema_version()).expect("generation 2 is stored");
    assert_eq!(stale.at, fresh.at, "the two generations must TIE on `at` — that is the whole scenario");
    assert!(
        stale.digest < fresh.digest,
        "fixture must arrange the STALE body to sort first ({} < {}), so an `at`-only fold demonstrably keeps the wrong one",
        stale.digest,
        fresh.digest
    );

    let expected_status = fold_latest_by_key(candidates, |c| c.key.clone(), |c| c.at, |c| c.schema, |c| c.digest.as_str())
        .into_values()
        .next()
        .expect("one natural key, one winner")
        .status;
    assert_eq!(expected_status, "done", "`fold_latest_by_key` breaks the `at` tie by the greater `schema`, i.e. generation 2");

    let roots = Roots::new(git_root, dir.path().join("r2"), dir.path().join("learn"));

    let trust = marts::fetch_trust_matrix(&roots).unwrap();
    assert_eq!(trust.rows.len(), 1, "one task must yield exactly one trust-matrix row, got {:?}", trust.rows);
    assert_eq!(
        trust.rows[0]["task_status"], expected_status,
        "`mart_trust_matrix` must pick the version `canon query` picks; ordering by `at` alone picked the superseded generation"
    );

    // `int_task_scenario_refs` folds the same key, so the same tie
    // decides which version's declared refs `mart_scope_status` reads.
    let scope = marts::fetch_scope_status(&roots).unwrap();
    assert_eq!(scope.rows.len(), 1, "one declared pair must yield exactly one row, got {:?}", scope.rows);
    assert_eq!(scope.rows[0]["task_status"], expected_status);
}

/// Rung 2, `schema`: the SQL rung's domain must be the domain
/// `canon_store::tier::raw_record_schema` computes, not DuckDB's.
///
/// That function is
/// `get("schema").and_then(as_u64).and_then(u32::try_from).unwrap_or(0)`
/// — it reads the field as a `u64` and then NARROWS to `u32`, flooring
/// anything that does not fit to `0`. `views.sql` used to compare the
/// raw JSON integer as a DuckDB `BIGINT`, which is neither of those
/// domains, so for `schema: 4294967296` the two folds ranked the same
/// pair in OPPOSITE orders: Rust reads it as `0` and keeps the
/// `schema: 1` sibling, DuckDB read it as the LARGEST rank in the
/// corpus and kept the other. Nothing upstream prevents the value: an
/// overlay's `schema` is authored by a plugin, and
/// `validate_envelope_shape` accepts any JSON integer.
#[test]
fn scope_status_picks_the_schema_version_canon_query_picks_when_a_rung_overflows_u32() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let git_root = dir.path().join("ledger");
    let tier = GitTier::new(&git_root);

    const OVERFLOWED_SCHEMA: u64 = u32::MAX as u64 + 1;
    const SCENARIO: &str = "fold.u32.01";
    const PROJECT: &str = "alpha";

    tier.write(
        &Task::new(
            Envelope::new(RecordKind::Task.schema_version(), RecordKind::Task, at(2026, 5, 5, 9), actor("planner")),
            TaskId::parse("fold-u32#1").unwrap(),
            "u32 rung task",
            TaskStatus::Done,
            None,
        )
        .with_scenario_refs(vec![ScenarioId::parse(SCENARIO).unwrap()]),
    )
    .unwrap();

    // Two versions of ONE overlay key (`write_namespaced` appends on a
    // changed body), tied on `at`, disagreeing about `covered` so the
    // winner is observable, and differing on the ONE rung under test.
    let natural_key = format!("{PROJECT}__{SCENARIO}");
    tier.write_namespaced("porting.coverage", &natural_key, coverage_overlay_at_schema(PROJECT, SCENARIO, true, 1)).unwrap();
    tier.write_namespaced("porting.coverage", &natural_key, coverage_overlay_at_schema(PROJECT, SCENARIO, false, OVERFLOWED_SCHEMA)).unwrap();

    let (stored, violations) = tier.scan_namespaced_kind("porting.coverage").unwrap();
    assert!(violations.is_empty(), "an out-of-`u32` `schema` is a legal stored overlay, not a violation: {violations:?}");
    assert_eq!(stored.len(), 2, "two versions at one overlay key must be two physical objects, got {stored:?}");

    // Expectation COMPUTED by the Rust fold, never hardcoded — the same
    // posture `trust_matrix_breaks_an_equal_at_task_tie_…` takes, and
    // the reason this test fails on a divergence in EITHER direction.
    struct Candidate {
        key: String,
        at: DateTime<Utc>,
        schema: u32,
        digest: String,
        raw_schema: u64,
        covered: bool,
    }
    let candidates: Vec<Candidate> = stored
        .iter()
        .map(|(_, record)| Candidate {
            key: format!("{}__{}", record.0["project_id"].as_str().unwrap(), record.0["scenario_id"].as_str().unwrap()),
            at: raw_record_at(record),
            schema: raw_record_schema(record),
            digest: content_digest12(&record.0),
            raw_schema: record.0["schema"].as_u64().expect("both fixture bodies store `schema` as a JSON integer"),
            covered: record.0["covered"].as_bool().expect("a coverage overlay always carries a boolean `covered`"),
        })
        .collect();

    // Fixture properties that make this a GUARD rather than a
    // coincidence: the two versions tie on `at` (so `schema` decides),
    // they disagree on the answer (so the winner is visible), and the
    // RAW integer out-ranks while the FOLD value ranks last.
    let overflowed = candidates.iter().find(|c| c.raw_schema == OVERFLOWED_SCHEMA).expect("the overflowed generation is stored");
    let kept = candidates.iter().find(|c| c.raw_schema == 1).expect("the `schema: 1` generation is stored");
    assert_eq!(overflowed.at, kept.at, "the two versions must TIE on `at` — `schema` is the rung under test");
    assert_eq!(overflowed.schema, 0, "`raw_record_schema` floors a value outside `u32` to 0");
    assert_eq!(kept.schema, 1);
    assert!(overflowed.raw_schema > u64::from(kept.schema), "the RAW integer must out-rank, so a `BIGINT` rung demonstrably picks the other version");
    assert_ne!(overflowed.covered, kept.covered, "the two versions must disagree, or the winner is unobservable");

    let expected_covered = fold_latest_by_key(candidates, |c| c.key.clone(), |c| c.at, |c| c.schema, |c| c.digest.as_str())
        .into_values()
        .next()
        .expect("one overlay key, one winner")
        .covered;
    assert!(expected_covered, "`fold_latest_by_key` keeps the `schema: 1` version — its overflowed sibling ranks 0");

    let roots = Roots::new(git_root, dir.path().join("r2"), dir.path().join("learn"));
    let scope = marts::fetch_scope_status(&roots).unwrap();
    assert_eq!(scope.rows.len(), 1, "one declared pair, one covering project, got {:?}", scope.rows);
    assert_eq!(
        scope.rows[0]["spec_covered"],
        json!(expected_covered),
        "`mart_scope_status` must pick the version `canon query` picks; comparing the raw JSON integer as a `BIGINT` picked the version `raw_record_schema` floors to 0"
    );
}

/// One stored version of the two-version overlay corpus the `at`-rung
/// tests fold, read back through the SAME `(at, schema, digest)`
/// extraction `canon-cli::query`'s `fold_latest_by_natural_key`
/// performs — plus the raw `at` TEXT, which is what those tests
/// select a version by and what the SQL rung parses.
#[derive(Clone)]
struct AtVersion {
    key: String,
    at: DateTime<Utc>,
    at_text: String,
    schema: u32,
    digest: String,
    covered: bool,
}

const AT_RUNG_PROJECT: &str = "alpha";

/// The corpus all three `at`-rung tests fold: one `Task` declaring one
/// scenario (so `mart_scope_status` has a row at all), plus TWO
/// versions of that scenario's `porting.coverage` overlay at ONE
/// natural key, differing in `at`, `schema` and `covered`.
///
/// `porting.coverage` is the vehicle for the same reason
/// `scope_status_picks_the_schema_version_…` uses it — an overlay body
/// is hand-authored JSON, so its `at` can be any string
/// `validate_envelope_shape` admits. `version_rank` is defined ONCE in
/// `stg_records` and inherited by all twelve fold sites, so a rung
/// pinned through this mart is pinned for every one of them.
struct AtRungCorpus {
    dir: tempfile::TempDir,
    git_root: std::path::PathBuf,
    versions: Vec<AtVersion>,
}

impl AtRungCorpus {
    fn build(task_id: &str, scenario: &str, versions: [(&str, u64, bool); 2]) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let git_root = dir.path().join("ledger");
        let tier = GitTier::new(&git_root);

        tier.write(
            &Task::new(
                Envelope::new(RecordKind::Task.schema_version(), RecordKind::Task, at(2026, 5, 7, 8), actor("planner")),
                TaskId::parse(task_id).unwrap(),
                "at rung task",
                TaskStatus::Done,
                None,
            )
            .with_scenario_refs(vec![ScenarioId::parse(scenario).unwrap()]),
        )
        .unwrap();

        let natural_key = format!("{AT_RUNG_PROJECT}__{scenario}");
        for (stamp, schema, covered) in versions {
            tier.write_namespaced("porting.coverage", &natural_key, coverage_overlay_at_stamp(AT_RUNG_PROJECT, scenario, covered, schema, stamp))
                .unwrap();
        }

        let (stored, violations) = tier.scan_namespaced_kind("porting.coverage").unwrap();
        assert!(violations.is_empty(), "every `at` spelling under test is a LEGAL stored overlay, not a violation: {violations:?}");
        assert_eq!(stored.len(), 2, "two versions at one overlay key must be two physical objects, got {stored:?}");

        let versions = stored
            .iter()
            .map(|(_, record)| AtVersion {
                key: format!("{}__{}", record.0["project_id"].as_str().unwrap(), record.0["scenario_id"].as_str().unwrap()),
                at: raw_record_at(record),
                at_text: record.0["at"].as_str().expect("the fixture always stores `at` as a string").to_string(),
                schema: raw_record_schema(record),
                digest: content_digest12(&record.0),
                covered: record.0["covered"].as_bool().expect("a coverage overlay always carries a boolean `covered`"),
            })
            .collect();

        Self { dir, git_root, versions }
    }

    fn stamped(&self, at_text: &str) -> &AtVersion {
        self.versions.iter().find(|v| v.at_text == at_text).unwrap_or_else(|| panic!("no stored version is stamped `{at_text}`"))
    }

    /// The winner `canon query` returns, COMPUTED by the Rust fold and
    /// never hardcoded — the posture every other rung test here takes,
    /// and what makes these fail on a divergence in EITHER direction.
    fn rust_winner_covered(&self) -> bool {
        fold_latest_by_key(self.versions.clone(), |v| v.key.clone(), |v| v.at, |v| v.schema, |v| v.digest.as_str())
            .into_values()
            .next()
            .expect("one overlay key, one winner")
            .covered
    }

    /// The winner `mart_scope_status` reports for that same key.
    fn sql_winner_covered(&self) -> serde_json::Value {
        let roots = Roots::new(self.git_root.clone(), self.dir.path().join("r2"), self.dir.path().join("learn"));
        let scope = marts::fetch_scope_status(&roots).unwrap();
        assert_eq!(scope.rows.len(), 1, "one declared pair, one covering project, got {:?}", scope.rows);
        scope.rows[0]["spec_covered"].clone()
    }
}

/// Rung 1, `at`, the DISCARDED OFFSET. Two versions of one key at the
/// SAME INSTANT, written with different RFC3339 offsets, must TIE on
/// `at` here exactly as they tie in `raw_record_at`, leaving `schema`
/// to decide.
///
/// `version_rank`'s `at` member used to be `CAST(<the `at` text> AS
/// TIMESTAMP)` — a naive wall clock that DISCARDS the offset rather
/// than applying it. It read `12:00:00+05:00` as `12:00` where
/// `raw_record_at` reads `07:00Z`, so this pair ranked in OPPOSITE
/// orders in `canon query` and in every mart. Nothing upstream
/// prevents the stamp: `validate_envelope_shape` asks only that `at`
/// parse as RFC3339, never that it be UTC.
#[test]
fn scope_status_ranks_one_instant_written_in_two_offsets_as_a_tie() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    const OFFSET_STAMP: &str = "2026-05-07T12:00:00+05:00";
    const ZULU_STAMP: &str = "2026-05-07T07:00:00Z";

    // The version a WALL-CLOCK cast prefers (`12:00` beats `07:00`) is
    // deliberately the one the Rust fold demotes, by carrying the
    // lower `schema`. Without that inversion the two rules would agree
    // by luck and this would prove nothing.
    let corpus = AtRungCorpus::build("fold-atoff#1", "fold.atoff.01", [(OFFSET_STAMP, 1, false), (ZULU_STAMP, 2, true)]);

    let offset = corpus.stamped(OFFSET_STAMP);
    let zulu = corpus.stamped(ZULU_STAMP);
    assert_eq!(offset.at, zulu.at, "the two stamps must be the SAME instant — that is the whole scenario");
    assert_ne!(offset.at_text, zulu.at_text, "…written two DIFFERENT ways, or there is no offset to discard");
    assert!(offset.schema < zulu.schema, "the wall-clock winner must be the fold's LOSER, or the two rules are indistinguishable");
    assert_ne!(offset.covered, zulu.covered, "the two versions must disagree, or the winner is unobservable");

    let expected = corpus.rust_winner_covered();
    assert!(expected, "with `at` tied, `fold_latest_by_key` keeps the greater `schema` — the `Z`-stamped version");
    assert_eq!(
        corpus.sql_winner_covered(),
        json!(expected),
        "`mart_scope_status` must pick the version `canon query` picks; casting the `at` TEXT to a naive `TIMESTAMP` dropped the `+05:00` and picked the other"
    );
}

/// Rung 1, `at`, the TRUNCATED PRECISION — at exactly the delta
/// canon's own dispatch path writes.
/// `canon-cli::dispatch::close_version_at` stamps a closing run's
/// version at the later of the observed clock and `recorded + 1ns`,
/// so whenever the host clock is not monotone a dispatched run's
/// begin/close pair differs by ONE NANOSECOND and nothing else.
///
/// `version_rank`'s `at` member used to be a MICROSECOND `TIMESTAMP`,
/// which truncated that delta away entirely: the pair TIED, the fold
/// fell through to `schema` and then to `digest`, and a mart could
/// report the superseded version of a finished run while `canon
/// query` returned the current one.
#[test]
fn scope_status_ranks_a_one_nanosecond_version_bump_as_the_later_version() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    // Built the way `close_version_at` builds it — `recorded` plus
    // chrono's smallest representable step — rather than hardcoded, so
    // the pair stays that function's successor by construction.
    let recorded = at(2026, 5, 7, 9);
    let closed = recorded + TimeDelta::nanoseconds(1);
    let begin_stamp = recorded.to_rfc3339();
    let close_stamp = closed.to_rfc3339();

    // Same inversion device as the offset test: the EARLIER version
    // carries the higher `schema`, so a rung that cannot see the
    // nanosecond demonstrably falls through and keeps the wrong one.
    let corpus = AtRungCorpus::build("fold-atns#1", "fold.atns.01", [(begin_stamp.as_str(), 2, false), (close_stamp.as_str(), 1, true)]);

    let begin = corpus.stamped(&begin_stamp);
    let close = corpus.stamped(&close_stamp);
    assert_eq!(
        close.at.signed_duration_since(begin.at),
        TimeDelta::nanoseconds(1),
        "the pair must be exactly `close_version_at`'s one-nanosecond successor"
    );
    assert_eq!(
        begin.at.timestamp_micros(),
        close.at.timestamp_micros(),
        "…and must be INVISIBLE at microsecond resolution, or the old rung would already have seen it"
    );
    assert!(begin.schema > close.schema, "the earlier version must out-rank on `schema`, or a tie would keep the right one by luck");
    assert_ne!(begin.covered, close.covered, "the two versions must disagree, or the winner is unobservable");

    let expected = corpus.rust_winner_covered();
    assert!(expected, "`fold_latest_by_key` compares nanoseconds, so the CLOSED version wins on `at` before `schema` is consulted");
    assert_eq!(
        corpus.sql_winner_covered(),
        json!(expected),
        "`mart_scope_status` must pick the version `canon query` picks; a microsecond `TIMESTAMP` tied the pair and fell through to `schema`"
    );
}

/// Rung 1, `at`, the PARSE SURFACE. `DateTime::parse_from_rfc3339`
/// accepts lowercase `t`/`z` (chrono 0.4.45
/// `format::parse::parse_rfc3339`: "quoted characters can be in any
/// mixture of lower and upper cases"); a DuckDB `TIMESTAMP` cast
/// returns NULL for them, which the old rung coalesced to
/// `-infinity`. A record `canon query` reads at its real instant
/// therefore sank BELOW every sibling here — a fold decided by a
/// SPELLING, and the failure mode that made "just cast it" the wrong
/// fix for the offset above.
#[test]
fn scope_status_ranks_a_lowercase_rfc3339_stamp_at_its_real_instant() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    const UPPER_STAMP: &str = "2026-05-07T09:00:00Z";
    const LOWER_STAMP: &str = "2026-05-07t10:00:00z";

    // Equal `schema`, so `at` ALONE decides and no lower rung can
    // rescue a floored stamp.
    let corpus = AtRungCorpus::build("fold-atlc#1", "fold.atlc.01", [(UPPER_STAMP, 1, false), (LOWER_STAMP, 1, true)]);

    let upper = corpus.stamped(UPPER_STAMP);
    let lower = corpus.stamped(LOWER_STAMP);
    assert!(
        !LOWER_STAMP.contains('T') && !LOWER_STAMP.contains('Z'),
        "the stamp under test must be the lowercase spelling, or nothing about the parse surface is exercised"
    );
    assert!(lower.at > upper.at, "the lowercase stamp is an hour LATER — `parse_from_rfc3339` reads it at its real instant");
    assert_eq!(lower.schema, upper.schema, "the `schema` rung must be neutral here, or it could mask a floored `at`");
    assert_ne!(lower.covered, upper.covered, "the two versions must disagree, or the winner is unobservable");

    let expected = corpus.rust_winner_covered();
    assert!(expected, "`fold_latest_by_key` keeps the later instant, however its offset is spelled");
    assert_eq!(
        corpus.sql_winner_covered(),
        json!(expected),
        "`mart_scope_status` must pick the version `canon query` picks; a `TIMESTAMP` cast returned NULL for the lowercase stamp and floored it to `-infinity`"
    );
}

/// A 12-hex digest that sorts strictly above every real
/// `content_digest12` output, so a row planted with it wins the third
/// rung outright.
const TAMPERED_DIGEST: &str = "ffffffffffff";

/// Rewrites one r2 object's materialized `digest` COLUMN in place,
/// leaving `body` byte-identical — precisely the row shape
/// `R2Tier::read`'s `validate_row` exists to refuse. Uses the `duckdb`
/// CLI these tests already require rather than standing up a second
/// arrow encoder in the test binary. The scratch file deliberately does
/// NOT end in `.parquet`, so it can never be picked up by
/// `stg_r2_records`' glob even transiently.
fn tamper_r2_digest_column(object: &std::path::Path, digest: &str) {
    let scratch = object.with_file_name("_tampering.tmp");
    let sql = format!(
        "COPY (SELECT kind, natural_key, \"at\", '{digest}' AS digest, body FROM read_parquet('{src}')) TO '{dst}' (FORMAT PARQUET);",
        src = object.display(),
        dst = scratch.display()
    );
    let out = std::process::Command::new("duckdb").arg("-c").arg(&sql).output().expect("the `duckdb` CLI runs");
    assert!(out.status.success(), "duckdb rewrite failed: {}", String::from_utf8_lossy(&out.stderr));
    std::fs::rename(&scratch, object).unwrap();
}

/// Locates the r2 object carrying `digest` by walking the local root,
/// rather than joining `WriteReceipt.location` onto it: that location
/// is an `object_store` key, which percent-encodes characters a
/// `task_id` legally contains, so it is not always the on-disk name.
fn r2_object_with_digest(r2_root: &std::path::Path, digest: &str) -> std::path::PathBuf {
    fn walk(dir: &std::path::Path, needle: &str, found: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, needle, found);
            } else if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(".parquet") && n.contains(needle)) {
                found.push(path);
            }
        }
    }
    let mut found = Vec::new();
    walk(r2_root, digest, &mut found);
    assert_eq!(found.len(), 1, "exactly one r2 object must carry digest `{digest}`, found {found:?}");
    found.pop().expect("length asserted above")
}

fn digest_residual_task(task_id: &TaskId, at: DateTime<Utc>, status: TaskStatus) -> Task {
    Task::new(Envelope::new(RecordKind::Task.schema_version(), RecordKind::Task, at, actor("planner")), task_id.clone(), "digest residual task", status, None)
}

/// Rung 3, `digest`: an EXECUTABLE PIN of the residual
/// `crates/canon-store/sql/views.sql`'s header documents — the one
/// place these views knowingly disagree with the Rust readers, on BOTH
/// roots, for the same structural reason and through two different
/// enforcement points.
///
/// `content_digest12` hashes a canonical serialization DuckDB cannot
/// reproduce, so each staging view reads a STORED copy of that number
/// instead: the git FILENAME's `__{digest12}` suffix, and the r2
/// parquet `digest` COLUMN. Each copy is enforced against the content
/// by that root's own reader — `GitTier::scan_kind_where`'s
/// `expected != relative` layout gate refuses the whole FILE,
/// `R2Tier::read`'s `validate_row` refuses the single ROW — and
/// NEITHER gate runs inside DuckDB. So the corpus each reader refuses
/// is exactly the corpus these views still fold, at whatever digest it
/// claims.
///
/// This test asserts that divergence rather than wishing it away: it
/// plants one refused version per root, at a digest that out-ranks its
/// well-formed sibling, and pins BOTH the Rust exclusion and the SQL
/// inclusion. It is a pin, NOT a statement that the behaviour is
/// desirable. Narrowing either residual is legitimate — but it must
/// land together with the header note, and this test failing is how
/// that gets noticed. In particular, r2's object key carries the same
/// `__{digest12}` suffix the git filename does
/// (`partition::hive_object_key`), and cross-checking it against the
/// column here is deliberately NOT done: `R2Tier::read` never
/// validates the key, so enforcing key == column in SQL would newly
/// demote rows `canon query` reads happily. The header states that
/// trade in full.
#[test]
fn both_roots_fold_a_digest_the_rust_reader_refuses_to_return() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let git_root = dir.path().join("ledger");
    let r2_root = dir.path().join("r2");
    // Equal `at` AND equal `schema` on every pair below, so the third
    // rung is the one that decides — the whole point of the fixture.
    let tied = at(2026, 5, 8, 9);

    // ── git root: the refused version is a file RENAMED so its
    //    `__{digest12}` suffix no longer matches its own body.
    let git_task = TaskId::parse("fold-dig-git#1").unwrap();
    let git_tier = GitTier::new(&git_root);
    let git_kept = git_tier.write(&digest_residual_task(&git_task, tied, TaskStatus::Done)).unwrap();
    let git_refused = git_tier.write(&digest_residual_task(&git_task, tied, TaskStatus::Open)).unwrap();
    assert_ne!(git_kept.digest, git_refused.digest, "two differing bodies must be two distinct objects");
    assert!(
        git_kept.digest.as_str() < TAMPERED_DIGEST && git_refused.digest.as_str() < TAMPERED_DIGEST,
        "the planted digest must out-rank both real ones ({} / {})",
        git_kept.digest,
        git_refused.digest
    );
    std::fs::rename(
        git_root.join(&git_refused.location),
        git_root.join(git_refused.location.replace(&format!("__{}.json", git_refused.digest), &format!("__{TAMPERED_DIGEST}.json"))),
    )
    .unwrap();

    let git_read = git_tier.read(&TierQuery::kind(RecordKind::Task)).unwrap();
    assert_eq!(git_read.records.len(), 1, "the renamed file must be refused, leaving one readable version, got {:?}", git_read.records);
    assert_eq!(git_read.records[0].0["status"], "done", "the readable version is the one whose name still matches its body");
    assert_eq!(git_read.violations.len(), 1, "…and refused LOUDLY: {:?}", git_read.violations);
    assert_eq!(git_read.violations[0].subject, "layout", "git refuses it through the layout gate, got {:?}", git_read.violations[0]);

    // ── r2 root: the refused version is a row whose materialized
    //    `digest` column no longer matches its own body.
    let r2_task = TaskId::parse("fold-dig-r2#1").unwrap();
    let r2_tier = R2Tier::local(&r2_root, "").unwrap();
    let r2_kept = r2_tier.write(&digest_residual_task(&r2_task, tied, TaskStatus::Done)).unwrap();
    let r2_refused = r2_tier.write(&digest_residual_task(&r2_task, tied, TaskStatus::Open)).unwrap();
    assert!(
        r2_kept.digest.as_str() < TAMPERED_DIGEST && r2_refused.digest.as_str() < TAMPERED_DIGEST,
        "the planted digest must out-rank both real ones ({} / {})",
        r2_kept.digest,
        r2_refused.digest
    );
    tamper_r2_digest_column(&r2_object_with_digest(&r2_root, &r2_refused.digest), TAMPERED_DIGEST);

    let r2_read = r2_tier.read(&TierQuery::kind(RecordKind::Task)).unwrap();
    assert_eq!(r2_read.records.len(), 1, "the tampered row must be refused, leaving one readable version, got {:?}", r2_read.records);
    assert_eq!(r2_read.records[0].0["status"], "done", "the readable version is the one whose column still matches its body");
    assert_eq!(r2_read.violations.len(), 1, "…and refused LOUDLY: {:?}", r2_read.violations);
    assert!(
        r2_read.violations[0].detail.contains("digest"),
        "r2 refuses it through `validate_row`'s digest check, got {:?}",
        r2_read.violations[0]
    );

    // Both roots: the mart reports the version its Rust reader REFUSED.
    let roots = Roots::new(git_root, r2_root, dir.path().join("learn"));
    let trust = marts::fetch_trust_matrix(&roots).unwrap();
    let status_of = |task_id: &str| {
        trust
            .rows
            .iter()
            .find(|r| r.get("task_id").and_then(|v| v.as_str()) == Some(task_id))
            .unwrap_or_else(|| panic!("no trust-matrix row for {task_id}: {:?}", trust.rows))["task_status"]
            .clone()
    };
    assert_eq!(
        status_of("fold-dig-git#1"),
        json!("open"),
        "documented git residual: `canon query` drops the renamed file as a layout violation, these views fold it at its filename digest"
    );
    assert_eq!(
        status_of("fold-dig-r2#1"),
        json!("open"),
        "documented r2 residual: `canon query` drops the tampered row via `validate_row`, these views fold it at its column digest"
    );
}

/// s43 (`findings-are-records`): a finding's DISPOSITION is a lifecycle
/// — `canon finding add` raises it `open`, a later write closes it
/// `fixed` — so one finding that got fixed is TWO versions at
/// `{change_id}__{round:04}__{seq:04}`. Unfolded, `mart_review_rounds`
/// would count that finding twice in `findings` and place it in BOTH
/// `disposition_open` and `disposition_fixed`, which is the
/// `mart_subjects` status double-count on the panel whose whole purpose
/// is a defensible count.
///
/// The `introduced_by` half is the sharper failure and is why this test
/// asserts the sourced/unsourced split too: the SUPERSEDED version here
/// carries no `introduced_by`, so an unfolded read inflates
/// `introduced_by_unsourced` — the UNKNOWN bucket the panel reports as
/// a known unknown, and the first of the two UNDER-counts its
/// canonical sentence names. Over-reporting the unknown makes that
/// sentence itself wrong.
#[test]
fn review_rounds_counts_a_refixed_finding_once_at_its_latest_disposition() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let git_root = dir.path().join("ledger");
    let tier = GitTier::new(&git_root);
    let change = ChangeId::parse("fold-finding-change").unwrap();
    let resolution = Sha::parse("d".repeat(40)).unwrap();

    // Round 1 `seq 1`, raised `open` and then closed `fixed_by` at a
    // strictly greater `envelope.at` — the two writes `canon finding
    // add` produces across a round and its fix.
    let raised = Finding::new(
        Envelope::new(1, RecordKind::Finding, at(2026, 6, 1, 9), actor("reviewer")),
        change.clone(),
        1,
        1,
        FindingSeverity::Blocker,
        "reviewer1",
        "raised open, fixed later",
    );
    tier.write(&raised).unwrap();
    tier.write(
        &Finding::new(
            Envelope::new(1, RecordKind::Finding, at(2026, 6, 1, 10), actor("reviewer")),
            change.clone(),
            1,
            1,
            FindingSeverity::Blocker,
            "reviewer1",
            "raised open, fixed later",
        )
        .fixed_by(resolution.clone()),
    )
    .unwrap();

    // Round 2 `seq 1` names that resolution: the fix-of-fix edge, which
    // must be found through the FOLDED winner (the superseded `open`
    // version carries no `resolution_sha` at all).
    tier.write(
        &Finding::new(
            Envelope::new(1, RecordKind::Finding, at(2026, 6, 2, 9), actor("reviewer")),
            change.clone(),
            2,
            1,
            FindingSeverity::ShouldFix,
            "reviewer1",
            "defect in that fix",
        )
        .with_introduced_by(resolution),
    )
    .unwrap();

    let roots = Roots::new(git_root, dir.path().join("r2"), dir.path().join("learn"));
    let result = marts::fetch_review_rounds(&roots).unwrap();
    let row = |round: i64| {
        result
            .rows
            .iter()
            .find(|r| r.get("round").and_then(|v| v.as_i64()) == Some(round))
            .unwrap_or_else(|| panic!("no review-rounds row for round {round}: {:?}", result.rows))
    };

    let round_1 = row(1);
    assert_eq!(round_1["findings"], 1, "two versions of ONE finding are one finding, got {round_1:?}");
    assert_eq!(round_1["disposition_fixed"], 1, "the latest version's disposition is the one reported");
    assert_eq!(round_1["disposition_open"], 0, "the superseded `open` version must not also be counted");
    assert_eq!(round_1["introduced_by_unsourced"], 1, "one finding, one unsourced slot — never one per version");

    // The edge still resolves: `fix_of_fix` reads the FOLDED winner's
    // `resolution_sha`, which only the latest version carries.
    assert_eq!(row(2)["fix_of_fix"], 1, "the fix-of-fix join must see the folded winner's resolution_sha");
}
