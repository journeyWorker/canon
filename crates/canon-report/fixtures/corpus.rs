//! The S9/S24 fixture corpus (task 2.6, extended by s24 task 5.1): one
//! deterministic corpus covering all six marts with KNOWN expected
//! real write APIs production code uses (`GitTier::write`,
//! `ParquetStrategyStore::append`, `ParquetTrajectoryStore::append`) —
//! never hand-authored JSON/binary files, so this fixture can never
//! silently drift from an actual on-disk shape (a hand-computed
//! digest-suffixed git-tier filename, or a hand-crafted parquet byte
//! layout, would be exactly that risk — `crates/canon-store/src/
//! partition.rs`'s own module doc: a record's git-tier path is
//! content-derived, not caller-chosen). `#[path = "../fixtures/
//! corpus.rs"]`-included from `tests/support.rs` (physically living
//! under `crates/canon-report/fixtures/` per task 2.6, reachable from
//! every `tests/*.rs` integration test binary).
//!
//! Every constant below is the mart row(s) `crates/canon-report/tests/
//! marts.rs` asserts against — this module is BOTH the corpus builder
//! and the single source of truth for what a correct render of it must
//! contain (design D5, "one fixture snapshot" requirement).

// Shared across three separate `tests/*.rs` binaries; each only
// exercises a subset of these documented expected-value constants —
// never truly dead, just per-binary partially unused.
#![allow(dead_code)]


use std::path::Path;

use canon_ingest::verdict::{Becomes, Polarity, VerdictRow};
use canon_model::evidence::RawRecord;
use canon_learn::store::{ParquetStrategyStore, ParquetTrajectoryStore, StrategyStore, TrajectoryStore};
use canon_learn::strategy::{DemotionEvidence, StrategyItem as LearnStrategyItem};
use canon_learn::trajectory::Trajectory as LearnTrajectory;
use canon_learn::verdict_outcome::{TrajectoryVerdict, VerdictOutcome};
use canon_model::envelope::{Actor, Envelope, RecordKind};
use canon_model::ids::{ProjectId, RegimeKey, RoleId, RunId, ScenarioId, Sha, SessionId, SubjectId, TaskId, TotalOrder};
use canon_model::records::{
    Divergence, DivergenceStatus, Event, EvidenceRecord, EvidenceVerdict, Review, ProvenanceRef, Run, RunStatus, Session, StrategyRef, Subject, SubjectStatus, Task, TaskStatus,
};
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use canon_report::roots::Roots;
use chrono::{DateTime, TimeZone, Utc};
use serde_json::json;

fn at(y: i32, m: u32, d: u32, h: u32) -> DateTime<Utc> {
    at_min(y, m, d, h, 0)
}

fn at_min(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, m, d, h, min, 0).single().expect("fixed fixture timestamp is valid")
}

fn actor(agent_id: &str, role: &str) -> Actor {
    Actor::new(agent_id, RoleId::parse(role).expect("fixture role is a valid kebab-slug"))
}

fn regime(role: &str) -> RegimeKey {
    RegimeKey::parse(canon_model::ids::regime_key(role, "acme", "auth", "abc123")).expect("fixture regime_key is well-formed")
}

fn project_id() -> ProjectId {
    ProjectId::parse("root").expect("fixture project_id is a valid ProjectId")
}

/// `mart_trust_matrix`'s expected rows (this fixture builds exactly
/// three subjects under the SAME `s9-fixture` change_id): task 1 is
/// covered+green (a `faithful` evidence record), task 2 is covered but
/// NOT green (only a `divergent` evidence record), task 3 has a `Task`
/// record but no evidence at all (not covered).
pub mod trust_matrix {
    pub const CHANGE_ID: &str = "s9-fixture";
    pub const TASK_1_COVERED_GREEN: (&str, bool, bool, &str) = ("s9-fixture#1", true, true, "agentA");
    pub const TASK_2_COVERED_NOT_GREEN: (&str, bool, bool, &str) = ("s9-fixture#2", true, false, "agentB");
    pub const TASK_3_NOT_COVERED: &str = "s9-fixture#3";
}

/// `mart_session_costs`'s expected single row: one session, one run,
/// two `token_usage` events summing to `0.05` cost / `120` tokens.
pub mod session_costs {
    pub const SESSION_ID: &str = "s9-fixture-session";
    pub const CLIENT: &str = "claude-code";
    pub const ROLE: &str = "dev";
    pub const WORKSPACE_LABEL: &str = "acme";
    pub const RUN_COUNT: i64 = 1;
    pub const TOTAL_COST: f64 = 0.05;
    pub const TOTAL_TOKENS: i64 = 120;
}

/// `mart_role_memory`'s expected four rows: `dev/acme/auth/abc123` has
/// two strategies (one active, one demoted → `hit_rate` `0.5`);
/// `content`, `reviewer` and `fixer` (same repo/area/hash) each have one
/// active strategy (`hit_rate` `1.0`). `reviewer` and `fixer` exist for
/// the funnel's attribution cases below and are incidental here — they
/// are asserted anyway, so adding a role can never silently change this
/// panel's shape unnoticed.
pub mod role_memory {
    pub const DEV_STRATEGY_COUNT: i64 = 2;
    pub const DEV_ACTIVE_COUNT: i64 = 1;
    pub const DEV_DEMOTED_COUNT: i64 = 1;
    pub const DEV_HIT_RATE: f64 = 0.5;
    pub const CONTENT_STRATEGY_COUNT: i64 = 1;
    pub const CONTENT_HIT_RATE: f64 = 1.0;
    pub const REVIEWER_STRATEGY_COUNT: i64 = 1;
    pub const REVIEWER_HIT_RATE: f64 = 1.0;
    pub const FIXER_STRATEGY_COUNT: i64 = 1;
    pub const FIXER_HIT_RATE: f64 = 1.0;
}

/// `mart_flywheel_funnel`'s expected four rows. `dev` carries 3 verdict
/// rows across two trajectories and 2 distilled strategies; `content`,
/// `reviewer` and `fixer` each carry 1 verdict row and 1 distilled
/// strategy. All four roles show `retrieved = 1`: each has exactly one
/// strategy cited by at least one run's `injected_guidance`.
///
/// The four roles are one case per RULE the `applied` stage admits, plus
/// their boundary and their precedence:
///
/// - `dev` — the s40 PROXY. Its strategy went into the `Succeeded`
///   session run, and no trajectory names that run, so it counts under
///   `applied_proxy` (`DEV_APPLIED_ATTRIBUTED = 0`).
/// - `content` — NEITHER rule. Its strategy went into a run still
///   `Running` (a dispatched run nobody closed) and no trajectory names
///   that run either, so `applied = 0`. `content`'s trajectory is
///   deliberately marked RESOLVED: under the pre-s40 definition
///   (`applied` = resolved trajectories, no reference to retrieval)
///   `content` would read `applied = 1`, so `CONTENT_APPLIED = 0` is
///   exactly the assertion a regression to that definition trips. It is
///   ALSO unstamped on purpose, so it cannot pass by attribution either
///   — this row is what keeps `applied` from becoming "any resolved
///   trajectory" again by either route.
/// - `reviewer` — ATTRIBUTION, s42 (`close-the-open-loops`) task 3.3,
///   closing s40 task 3.1's original wording. Its recipient run is ALSO
///   still `Running`, so the proxy alone would score it `0` exactly like
///   `content`; it reads `applied = 1` solely because a RESOLVED
///   `reviewer` trajectory carries that run's own `run_id`. The pairing
///   with `content` is the whole point: the two rows differ only in
///   whether the trajectory was stamped, so `REVIEWER_APPLIED = 1`
///   cannot be produced by the proxy and `CONTENT_APPLIED = 0` cannot be
///   produced by attribution.
/// - `fixer` — BOTH rules at once, which is what makes the split's
///   partition property testable rather than vacuous. Its ONE strategy
///   is cited by TWO runs: one still `Running` with a stamped resolved
///   `fixer` trajectory, and one `Succeeded` with none. Attribution wins,
///   so the strategy is counted ONCE, under `applied_attributed`
///   (`FIXER_APPLIED_PROXY = 0`). Two independently-filtered
///   `count(DISTINCT strategy_id)`s would report `1 + 1` here and make
///   the parts sum above `applied` — the widening
///   `flywheel_funnel_never_widens` exists to catch.
///
/// `dev`'s second trajectory stays pending so the corpus still carries a
/// mixed-outcome role.
pub mod flywheel_funnel {
    pub const DEV_VERDICTS: i64 = 3;
    pub const DEV_DISTILLED: i64 = 2;
    pub const DEV_RETRIEVED: i64 = 1;
    pub const DEV_APPLIED: i64 = 1;
    pub const DEV_APPLIED_ATTRIBUTED: i64 = 0;
    pub const DEV_APPLIED_PROXY: i64 = 1;
    pub const CONTENT_VERDICTS: i64 = 1;
    pub const CONTENT_DISTILLED: i64 = 1;
    pub const CONTENT_RETRIEVED: i64 = 1;
    pub const CONTENT_APPLIED: i64 = 0;
    pub const CONTENT_APPLIED_ATTRIBUTED: i64 = 0;
    pub const CONTENT_APPLIED_PROXY: i64 = 0;
    pub const REVIEWER_VERDICTS: i64 = 1;
    pub const REVIEWER_DISTILLED: i64 = 1;
    pub const REVIEWER_RETRIEVED: i64 = 1;
    pub const REVIEWER_APPLIED: i64 = 1;
    pub const REVIEWER_APPLIED_ATTRIBUTED: i64 = 1;
    pub const REVIEWER_APPLIED_PROXY: i64 = 0;
    pub const FIXER_VERDICTS: i64 = 1;
    pub const FIXER_DISTILLED: i64 = 1;
    pub const FIXER_RETRIEVED: i64 = 1;
    pub const FIXER_APPLIED: i64 = 1;
    pub const FIXER_APPLIED_ATTRIBUTED: i64 = 1;
    pub const FIXER_APPLIED_PROXY: i64 = 0;
}

/// `mart_review_burndown`'s expected running total: one `divergence`
/// opened on day 1, one resolved on day 3 → the running total is `1`
/// on day 1, `1` on day 2 (evidence-only day, no divergence rows —
/// absent from the `GROUP BY day` result entirely), `0` on day 3.
pub mod review_burndown {
    pub const DAY_1_OPENED: i64 = 1;
    pub const DAY_3_RESOLVED: i64 = 1;
}

/// `mart_scope_status`'s expected two rows (s24 task 5.1): task 1
/// (already `done` + `Faithful` evidence in `trust_matrix`, above)
/// declares a scenario ref that ALSO has a `porting.coverage` overlay
/// row -> a fully-known, non-NULL row (`done`, `true`, `true`, `root`,
/// `true`). Task 2 (already `done` + `Divergent` evidence -> covered
/// but not green) declares a DIFFERENT scenario ref with NO
/// `porting.coverage` overlay at all -> `spec_project_id` AND
/// `spec_covered` are both an honest NULL, never a dropped row or an
/// invented `false`. Task 3 (no evidence at all) deliberately keeps its
/// default empty `scenario_refs` -> contributes NO row to
/// `mart_scope_status`, proving the view's additive-only,
/// declared-refs-only posture holds end-to-end through the Rust fetch
/// (tasks.md 5.6).
///
/// This corpus writes exactly ONE covering project, so it pins the
/// single-project shape of `spec_project_id`. The MULTI-project fan-out
/// (two spec roots authoring one scenario id, one row each) is pinned
/// separately by `tests/multi_version_fold.rs`, which needs its own
/// corpus for it.
pub mod scope_status {
    pub const FULLY_GREEN_TASK_ID: &str = "s9-fixture#1";
    pub const FULLY_GREEN_SCENARIO_ID: &str = "s9.fixture.03";
    pub const FULLY_GREEN_TASK_STATUS: &str = "done";
    pub const FULLY_GREEN_EVIDENCE_COVERED: bool = true;
    pub const FULLY_GREEN_GREEN: bool = true;
    /// The `project_id` of the ONE `porting.coverage` overlay row this
    /// corpus authors — `porting.coverage`'s join key is
    /// `(project_id, scenario_id)`, so the mart names whose coverage
    /// each row reports.
    pub const FULLY_GREEN_SPEC_PROJECT_ID: &str = "root";
    pub const FULLY_GREEN_SPEC_COVERED: bool = true;

    pub const UNAUTHORED_TASK_ID: &str = "s9-fixture#2";
    pub const UNAUTHORED_SCENARIO_ID: &str = "s9.fixture.04";
    pub const UNAUTHORED_TASK_STATUS: &str = "done";
    pub const UNAUTHORED_EVIDENCE_COVERED: bool = true;
    pub const UNAUTHORED_GREEN: bool = false;

    /// A task with no declared `scenario_refs` — `trust_matrix::
    /// TASK_3_NOT_COVERED` — must contribute zero rows here.
    pub const NO_SCENARIO_REFS_TASK_ID: &str = super::trust_matrix::TASK_3_NOT_COVERED;
}

/// `mart_subjects`'s expected single row (s36 `subject-domain-loop`):
/// one `dev`-domain subject in status `building`, linking two
/// scenarios — one carrying a `Faithful` evidence record keyed by its
/// `scenario_id` (covered), one with no evidence at all (uncovered) —
/// so `scenario_count = 2`, `covered_scenarios = 1`. Proves the panel
/// joins subject `scenario_ids` against the scenario-keyed evidence
/// ledger with the latest-non-Divergent fold, never a "some rows came
/// back" smoke check.
pub mod subjects {
    pub const DOMAIN: &str = "dev";
    pub const SUBJECT_ID: &str = "s9-fixture-subject";
    pub const TITLE: &str = "s9 fixture subject";
    pub const STATUS: &str = "building";
    pub const SCENARIO_COUNT: i64 = 2;
    pub const COVERED_SCENARIOS: i64 = 1;
    pub const COVERED_SCENARIO_ID: &str = "s9.subject.01";
    pub const UNCOVERED_SCENARIO_ID: &str = "s9.subject.02";
}

/// Builds the full fixture corpus (git tier + `canon-learn` parquet
/// stores) under `dir`, returning the [`Roots`] a [`canon_report::
/// ReportInputs`] can be constructed from directly.
pub fn build(dir: &Path) -> Roots {
    let git_root = dir.join("ledger");
    let learn_root = dir.join("learn");
    let r2_root = dir.join("r2"); // deliberately left empty — proves `Roots::ensure_seeded` handles it.

    build_git_tier(&git_root);
    build_learn_store(&learn_root);

    Roots::new(git_root, r2_root, learn_root)
}

fn build_git_tier(git_root: &Path) {
    let tier = GitTier::new(git_root);

    // ── trust matrix: 3 tasks, 2 evidence records ──────────────────
    tier.write(
        &Task::new(
            Envelope::current(RecordKind::Task, at(2026, 1, 1, 9), Actor::new_unattributed("fixture")),
            TaskId::parse("s9-fixture#1").unwrap(),
            "task one",
            TaskStatus::Done,
            Some("faithful evidence recorded".into()),
        )
        .with_scenario_refs(vec![ScenarioId::parse(scope_status::FULLY_GREEN_SCENARIO_ID).unwrap()]),
    )
    .unwrap();
    tier.write(
        &Task::new(
            Envelope::current(RecordKind::Task, at(2026, 1, 1, 9), Actor::new_unattributed("fixture")),
            TaskId::parse("s9-fixture#2").unwrap(),
            "task two",
            TaskStatus::Done,
            Some("divergent evidence recorded".into()),
        )
        .with_scenario_refs(vec![ScenarioId::parse(scope_status::UNAUTHORED_SCENARIO_ID).unwrap()]),
    )
    .unwrap();
    tier.write(&Task::new(
        Envelope::current(RecordKind::Task, at(2026, 1, 1, 9), Actor::new_unattributed("fixture")),
        TaskId::parse("s9-fixture#3").unwrap(),
        "task three",
        TaskStatus::Open,
        None,
    ))
    .unwrap();

    tier.write(&EvidenceRecord::new(
        Envelope::new(1, RecordKind::EvidenceRecord, at(2026, 1, 2, 10), actor("agentA", "dev")),
        Some(TaskId::parse("s9-fixture#1").unwrap()),
        None,
        None,
        EvidenceVerdict::Faithful,
    ))
    .unwrap();
    tier.write(&EvidenceRecord::new(
        Envelope::new(1, RecordKind::EvidenceRecord, at(2026, 1, 2, 11), actor("agentB", "dev")),
        Some(TaskId::parse("s9-fixture#2").unwrap()),
        None,
        None,
        EvidenceVerdict::Divergent,
    ))
    .unwrap();

    // ── mart_scope_status: task 1's declared scenario ref gets a
    // `porting.coverage` overlay (a fully-known, non-NULL row); task
    // 2's declared scenario ref gets NONE (an honest NULL
    // `spec_covered`); task 3 keeps its default empty `scenario_refs`
    // and so is absent from `mart_scope_status` entirely.
    tier.write_namespaced(
        "porting.coverage",
        &format!("root__{}", scope_status::FULLY_GREEN_SCENARIO_ID),
        RawRecord(json!({
            "schema": 1,
            "kind": "porting.coverage",
            "at": at(2026, 1, 2, 12).to_rfc3339(),
            "actor": {"agent_id": "porting-sync", "role": "implementer"},
            "project_id": "root",
            "scenario_id": scope_status::FULLY_GREEN_SCENARIO_ID,
            "covered": true,
        })),
    )
    .unwrap();

    // ── review burn-down: 1 divergence opened day 1, 1 resolved day 3 ──
    tier.write(&Divergence::new(
        Envelope::new(1, RecordKind::Divergence, at(2026, 1, 1, 12), actor("reviewer1", "reviewer")),
        project_id(),
        ScenarioId::parse("s9.fixture.01").unwrap(),
        Sha::parse("a".repeat(40)).unwrap(),
        DivergenceStatus::Open,
        TotalOrder::new(1),
        1,
        "reviewer1",
        "opened for fixture",
    ))
    .unwrap();
    tier.write(&Divergence::new(
        Envelope::new(1, RecordKind::Divergence, at(2026, 1, 3, 12), actor("reviewer1", "reviewer")),
        project_id(),
        ScenarioId::parse("s9.fixture.02").unwrap(),
        Sha::parse("b".repeat(40)).unwrap(),
        DivergenceStatus::Resolved,
        TotalOrder::new(1),
        1,
        "reviewer1",
        "resolved for fixture",
    ))
    .unwrap();

    tier.write(&Review::new(
        Envelope::new(1, RecordKind::Review, at(2026, 1, 2, 12), actor("reviewer1", "reviewer")),
        project_id(),
        ScenarioId::parse("s9.fixture.01").unwrap(),
        "reviewer1",
        "a".repeat(12),
        ProvenanceRef::UpstreamRef("s9-fixture-upstream-ref".to_string()),
    ))
    .unwrap();

    // ── session costs: 1 session, 1 run (carrying the `dev` strategy
    // as retrieved guidance), 2 token_usage events ─────────────────
    let session_id = SessionId::parse(session_costs::SESSION_ID).unwrap();
    tier.write(&Session::new(
        Envelope::new(1, RecordKind::Session, at(2026, 1, 4, 9), actor("fixture-session-actor", session_costs::ROLE)),
        session_id.clone(),
        session_costs::CLIENT,
        at(2026, 1, 4, 9),
        Some(at(2026, 1, 4, 10)),
    ))
    .unwrap();

    let run_id = RunId::new();
    let mut run = Run::new(
        Envelope::new(1, RecordKind::Run, at(2026, 1, 4, 10), Actor::new_unattributed(session_costs::CLIENT)),
        run_id,
        Some(session_id),
        None,
        RunStatus::Succeeded,
        at(2026, 1, 4, 9),
        Some(at(2026, 1, 4, 10)),
    );
    run.injected_guidance = vec![StrategyRef::new(dev_strategy_active_id(), "dev strategy", "content")];
    tier.write(&run).unwrap();

    // ── flywheel funnel's negative case (s40 `plan-vs-actual-diff`,
    // task 3.1): a SECOND run carrying the `content` strategy as
    // retrieved guidance and left `Running` — a dispatched run nobody
    // ever closed, which is the state every guidance-carrying run in a
    // real repo sits in until `canon dispatch end` runs. `retrieved`
    // counts it, `applied` must not. Deliberately session-less and
    // event-less: `mart_session_costs` inner-joins runs to their
    // `token_usage` events through a `Session`, so this run cannot
    // perturb that panel's expected single row.
    let mut unfinished_run = Run::new(
        Envelope::new(1, RecordKind::Run, at(2026, 1, 4, 11), Actor::new_unattributed(session_costs::CLIENT)),
        RunId::new(),
        None,
        None,
        RunStatus::Running,
        at(2026, 1, 4, 11),
        None,
    );
    unfinished_run.injected_guidance = vec![StrategyRef::new(content_strategy_id(), "content strategy", "content")];
    tier.write(&unfinished_run).unwrap();

    // ── flywheel funnel's ATTRIBUTION case (s42
    // (`close-the-open-loops`) task 3.3, closing s40 task 3.1): a THIRD
    // run, byte-for-byte the same shape as `unfinished_run` above —
    // guidance carried, still `Running`, session-less and event-less —
    // differing ONLY in that a resolved `reviewer` trajectory in the
    // learn store carries this run's `run_id` (`reviewer_run_id`, fixed
    // so both halves of the corpus can name it). The proxy scores this
    // run 0 exactly like `content`; `applied = 1` for `reviewer` is
    // therefore attributable to nothing but the stamped trajectory.
    let mut attributed_run = Run::new(
        Envelope::new(1, RecordKind::Run, at(2026, 1, 4, 12), Actor::new_unattributed(session_costs::CLIENT)),
        reviewer_run_id(),
        None,
        None,
        RunStatus::Running,
        at(2026, 1, 4, 12),
        None,
    );
    attributed_run.injected_guidance = vec![StrategyRef::new(reviewer_strategy_id(), "reviewer strategy", "content")];
    tier.write(&attributed_run).unwrap();

    // ── flywheel funnel's PRECEDENCE case (s42
    // (`close-the-open-loops`) task 3.3/3.4): the ONE `fixer` strategy is
    // cited by TWO runs, one admitted by each rule — a `Running` run with
    // a stamped resolved `fixer` trajectory, and a `Succeeded` run with
    // none. Attribution must WIN so the strategy is counted once, which is
    // what makes `applied == applied_attributed + applied_proxy` a real
    // assertion rather than a vacuous one on this corpus. Both are
    // session-less and event-less, so neither perturbs
    // `mart_session_costs`.
    let mut fixer_attributed_run = Run::new(
        Envelope::new(1, RecordKind::Run, at(2026, 1, 4, 13), Actor::new_unattributed(session_costs::CLIENT)),
        fixer_run_id(),
        None,
        None,
        RunStatus::Running,
        at(2026, 1, 4, 13),
        None,
    );
    fixer_attributed_run.injected_guidance = vec![StrategyRef::new(fixer_strategy_id(), "fixer strategy", "content")];
    tier.write(&fixer_attributed_run).unwrap();

    let mut fixer_terminal_run = Run::new(
        Envelope::new(1, RecordKind::Run, at(2026, 1, 4, 14), Actor::new_unattributed(session_costs::CLIENT)),
        RunId::new(),
        None,
        None,
        RunStatus::Succeeded,
        at(2026, 1, 4, 14),
        Some(at(2026, 1, 4, 15)),
    );
    fixer_terminal_run.injected_guidance = vec![StrategyRef::new(fixer_strategy_id(), "fixer strategy", "content")];
    tier.write(&fixer_terminal_run).unwrap();

    tier.write(&Event::new(
        Envelope::new(1, RecordKind::Event, at_min(2026, 1, 4, 9, 30), Actor::new_unattributed(session_costs::CLIENT)),
        run_id,
        1,
        "token_usage",
        json!({
            "provider_id": "anthropic",
            "workspace_key": "acme",
            "workspace_label": session_costs::WORKSPACE_LABEL,
            "tokens": {"input": 60, "output": 20, "cache_read": 0, "cache_write": 0, "reasoning": 0, "total": 80},
            "cost": 0.03,
            "cost_source": "api",
        }),
    ))
    .unwrap();
    tier.write(&Event::new(
        Envelope::new(1, RecordKind::Event, at_min(2026, 1, 4, 9, 45), Actor::new_unattributed(session_costs::CLIENT)),
        run_id,
        2,
        "token_usage",
        json!({
            "provider_id": "anthropic",
            "workspace_key": "acme",
            "workspace_label": session_costs::WORKSPACE_LABEL,
            "tokens": {"input": 30, "output": 10, "cache_read": 0, "cache_write": 0, "reasoning": 0, "total": 40},
            "cost": 0.02,
            "cost_source": "api",
        }),
    ))
    .unwrap();

    // ── mart_subjects: one `dev`-domain subject (status `building`)
    // linking two scenarios. The first carries a scenario-keyed
    // `Faithful` evidence record (covered); the second has none
    // (uncovered) -> scenario_count 2, covered_scenarios 1. Both
    // evidence writes are keyed by `scenario_id` with `task_id` None,
    // so `int_task_evidence` (which filters `task_id IS NOT NULL`)
    // excludes them -> `mart_trust_matrix`'s three-task shape is
    // unchanged. Dated 2026-01-02 (an already-present review-burndown
    // day) so the burn-down's last row stays 2026-01-03.
    tier.write(&EvidenceRecord::new(
        Envelope::new(1, RecordKind::EvidenceRecord, at(2026, 1, 2, 13), actor("agentA", "dev")),
        None,
        Some(ScenarioId::parse(subjects::COVERED_SCENARIO_ID).unwrap()),
        None,
        EvidenceVerdict::Faithful,
    ))
    .unwrap();
    tier.write(
        &Subject::new(
            Envelope::new(1, RecordKind::Subject, at(2026, 1, 5, 9), actor("planner1", "planner")),
            SubjectId::parse(subjects::SUBJECT_ID).unwrap(),
            subjects::TITLE,
            "fixture product unit",
            subjects::DOMAIN,
            SubjectStatus::Building,
            RoleId::parse("dev").unwrap(),
        )
        .with_links(
            vec![],
            vec![
                ScenarioId::parse(subjects::COVERED_SCENARIO_ID).unwrap(),
                ScenarioId::parse(subjects::UNCOVERED_SCENARIO_ID).unwrap(),
            ],
        ),
    )
    .unwrap();
}

// Fixed strategy/trajectory ids so `session_costs`'s `injected_guidance`
// (built before the learn store, above) can cite them by value — both
// sides of the corpus reference the SAME ids, exactly like a real
// `Run::injected_guidance` snapshot would.
fn dev_strategy_active_id() -> canon_learn::ids::StrategyId {
    canon_learn::ids::StrategyId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAV").unwrap()
}
fn dev_strategy_demoted_id() -> canon_learn::ids::StrategyId {
    canon_learn::ids::StrategyId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAW").unwrap()
}
fn content_strategy_id() -> canon_learn::ids::StrategyId {
    canon_learn::ids::StrategyId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAX").unwrap()
}
fn dev_trajectory_1_id() -> canon_learn::ids::TrajectoryId {
    canon_learn::ids::TrajectoryId::parse("01ARZ3NDEKTSV4RRFFQ69G5FB0").unwrap()
}
fn dev_trajectory_2_id() -> canon_learn::ids::TrajectoryId {
    canon_learn::ids::TrajectoryId::parse("01ARZ3NDEKTSV4RRFFQ69G5FB1").unwrap()
}
fn content_trajectory_id() -> canon_learn::ids::TrajectoryId {
    canon_learn::ids::TrajectoryId::parse("01ARZ3NDEKTSV4RRFFQ69G5FB2").unwrap()
}
fn reviewer_strategy_id() -> canon_learn::ids::StrategyId {
    canon_learn::ids::StrategyId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAY").unwrap()
}
fn reviewer_trajectory_id() -> canon_learn::ids::TrajectoryId {
    canon_learn::ids::TrajectoryId::parse("01ARZ3NDEKTSV4RRFFQ69G5FB3").unwrap()
}
fn fixer_strategy_id() -> canon_learn::ids::StrategyId {
    canon_learn::ids::StrategyId::parse("01ARZ3NDEKTSV4RRFFQ69G5FAZ").unwrap()
}
fn fixer_trajectory_id() -> canon_learn::ids::TrajectoryId {
    canon_learn::ids::TrajectoryId::parse("01ARZ3NDEKTSV4RRFFQ69G5FB4").unwrap()
}
/// The only two RUN ids this corpus fixes, for the same reason the ids
/// above are fixed and no other run needs to be: the funnel's attribution
/// rule (s42 (`close-the-open-loops`) task 3.3) is a JOIN between a `Run`
/// the git-tier half writes and a `Trajectory` the learn-store half
/// writes, so the two halves must name the identical value. Every other
/// fixture run is referenced only within `build_git_tier`, where a local
/// `RunId::new()` binding suffices.
fn reviewer_run_id() -> RunId {
    RunId::parse("01ARZ3NDEKTSV4RRFFQ69G5FC0").unwrap()
}
fn fixer_run_id() -> RunId {
    RunId::parse("01ARZ3NDEKTSV4RRFFQ69G5FC1").unwrap()
}

fn build_learn_store(learn_root: &Path) {
    let strategy_store = ParquetStrategyStore::open(learn_root.join("strategies"));
    let trajectory_store = ParquetTrajectoryStore::open(learn_root.join("trajectories"));

    // ── role memory: dev (1 active + 1 demoted), content (1 active) ──
    strategy_store
        .append(&LearnStrategyItem::new(
            dev_strategy_active_id(),
            regime("dev"),
            RoleId::parse("dev").unwrap(),
            "dev strategy (active)",
            "d",
            "content",
            vec![dev_trajectory_1_id()],
            at(2026, 1, 3, 9),
        ))
        .unwrap();
    strategy_store
        .append(
            &LearnStrategyItem::new(
                dev_strategy_demoted_id(),
                regime("dev"),
                RoleId::parse("dev").unwrap(),
                "dev strategy (demoted)",
                "d",
                "content",
                vec![dev_trajectory_2_id()],
                at(2026, 1, 3, 10),
            )
            .with_demotion(DemotionEvidence::new(dev_trajectory_2_id(), "contradicted by a later trajectory", at(2026, 1, 3, 11))),
        )
        .unwrap();
    strategy_store
        .append(&LearnStrategyItem::new(
            content_strategy_id(),
            regime("content"),
            RoleId::parse("content").unwrap(),
            "content strategy (active)",
            "d",
            "content",
            vec![content_trajectory_id()],
            at(2026, 1, 3, 9),
        ))
        .unwrap();

    // ── flywheel funnel: dev (2 trajectories / 3 verdict rows), content
    // (1 trajectory / 1 verdict row). A trajectory's own outcome no
    // longer feeds `applied` at all (s40 `plan-vs-actual-diff`, task
    // 3.1 — the recipient run's terminal status does), so the outcomes
    // below exist to make a regression to the old definition VISIBLE,
    // not to drive the expected counts. ──
    trajectory_store
        .append(
            &LearnTrajectory::new(
                dev_trajectory_1_id(),
                regime("dev"),
                "fix the bug",
                "context",
                vec![VerdictRow { role: RoleId::parse("dev").unwrap(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate }],
                at(2026, 1, 3, 8),
                vec![],
            )
            .unwrap()
            .with_verdict_record(TrajectoryVerdict::new(VerdictOutcome::Success, 0.9)),
        )
        .unwrap();
    trajectory_store
        .append(
            &LearnTrajectory::new(
                dev_trajectory_2_id(),
                regime("dev"),
                "attempt that regressed",
                "context",
                vec![
                    VerdictRow { role: RoleId::parse("dev").unwrap(), polarity: Polarity::Failure, becomes: Becomes::GuardrailCandidate },
                    VerdictRow { role: RoleId::parse("dev").unwrap(), polarity: Polarity::Corrective, becomes: Becomes::GuardrailWhatTheSampleCaught },
                ],
                at(2026, 1, 3, 8),
                vec![],
            )
            .unwrap(),
            // stays pending — the corpus keeps one unresolved sample.
        )
        .unwrap();
    trajectory_store
        .append(
            &LearnTrajectory::new(
                content_trajectory_id(),
                regime("content"),
                "copy edit",
                "context",
                vec![VerdictRow { role: RoleId::parse("content").unwrap(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate }],
                at(2026, 1, 3, 8),
                vec![],
            )
            .unwrap()
            // Resolved on purpose, and deliberately NOT stamped with a
            // run: the pre-s40 `applied` counted resolved trajectories,
            // so that definition would render `content` as
            // `applied = 1`, and s42's attribution rule would too if
            // this row named `unfinished_run`. `CONTENT_APPLIED = 0` is
            // the assertion both regressions trip.
            .with_verdict_record(TrajectoryVerdict::new(VerdictOutcome::Success, 0.8)),
        )
        .unwrap();

    // ── flywheel funnel's ATTRIBUTION case (s42
    // (`close-the-open-loops`) task 3.3): one `reviewer` strategy, cited
    // by `attributed_run` (still `Running`), plus one RESOLVED `reviewer`
    // trajectory stamped with that run's own id. This is the pair s40
    // task 3.1 asked for and could not express, because
    // `canon_learn::Trajectory` carried no run id at all.
    strategy_store
        .append(&LearnStrategyItem::new(
            reviewer_strategy_id(),
            regime("reviewer"),
            RoleId::parse("reviewer").unwrap(),
            "reviewer strategy (active)",
            "d",
            "content",
            vec![reviewer_trajectory_id()],
            at(2026, 1, 3, 9),
        ))
        .unwrap();
    trajectory_store
        .append(
            &LearnTrajectory::new(
                reviewer_trajectory_id(),
                regime("reviewer"),
                "review the change",
                "context",
                vec![VerdictRow { role: RoleId::parse("reviewer").unwrap(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate }],
                at(2026, 1, 3, 8),
                vec![],
            )
            .unwrap()
            .with_verdict_record(TrajectoryVerdict::new(VerdictOutcome::Success, 0.85))
            .with_run_id(Some(reviewer_run_id())),
        )
        .unwrap();

    // ── flywheel funnel's PRECEDENCE case (s42
    // (`close-the-open-loops`) task 3.3/3.4): one `fixer` strategy cited
    // by BOTH `fixer_attributed_run` (still `Running`, stamped below) and
    // a `Succeeded` run with no trajectory of its own. Attribution wins,
    // so this strategy is counted once — see `flywheel_funnel`'s own doc.
    strategy_store
        .append(&LearnStrategyItem::new(
            fixer_strategy_id(),
            regime("fixer"),
            RoleId::parse("fixer").unwrap(),
            "fixer strategy (active)",
            "d",
            "content",
            vec![fixer_trajectory_id()],
            at(2026, 1, 3, 9),
        ))
        .unwrap();
    trajectory_store
        .append(
            &LearnTrajectory::new(
                fixer_trajectory_id(),
                regime("fixer"),
                "remediate the finding",
                "context",
                vec![VerdictRow { role: RoleId::parse("fixer").unwrap(), polarity: Polarity::Corrective, becomes: Becomes::GuardrailWhatTheSampleCaught }],
                at(2026, 1, 3, 8),
                vec![],
            )
            .unwrap()
            .with_verdict_record(TrajectoryVerdict::new(VerdictOutcome::Success, 0.7))
            .with_run_id(Some(fixer_run_id())),
        )
        .unwrap();
}
