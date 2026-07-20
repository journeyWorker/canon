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

/// `mart_role_memory`'s expected two rows: `dev/acme/auth/abc123` has
/// two strategies (one active, one demoted → `hit_rate` `0.5`);
/// `content/acme/auth/abc123` has one active strategy (`hit_rate`
/// `1.0`).
pub mod role_memory {
    pub const DEV_STRATEGY_COUNT: i64 = 2;
    pub const DEV_ACTIVE_COUNT: i64 = 1;
    pub const DEV_DEMOTED_COUNT: i64 = 1;
    pub const DEV_HIT_RATE: f64 = 0.5;
    pub const CONTENT_STRATEGY_COUNT: i64 = 1;
    pub const CONTENT_HIT_RATE: f64 = 1.0;
}

/// `mart_flywheel_funnel`'s expected two rows: `dev` carries 3 verdict
/// rows across two trajectories (2 distilled strategies, 1 applied —
/// `t1`'s `success` outcome), `content` carries 1 verdict row (1
/// distilled strategy, 0 applied — `t3` stays `pending`). Both roles
/// show `retrieved = 1` (one run's `injected_guidance` cites one
/// strategy from each role).
pub mod flywheel_funnel {
    pub const DEV_VERDICTS: i64 = 3;
    pub const DEV_DISTILLED: i64 = 2;
    pub const DEV_RETRIEVED: i64 = 1;
    pub const DEV_APPLIED: i64 = 1;
    pub const CONTENT_VERDICTS: i64 = 1;
    pub const CONTENT_DISTILLED: i64 = 1;
    pub const CONTENT_RETRIEVED: i64 = 1;
    pub const CONTENT_APPLIED: i64 = 0;
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
/// row -> a fully-known, non-NULL row (`done`, `true`, `true`,
/// `true`). Task 2 (already `done` + `Divergent` evidence -> covered
/// but not green) declares a DIFFERENT scenario ref with NO
/// `porting.coverage` overlay at all -> `spec_covered` is an honest
/// NULL, never a dropped row or an invented `false`. Task 3 (no
/// evidence at all) deliberately keeps its default empty
/// `scenario_refs` -> contributes NO row to `mart_scope_status`,
/// proving the view's additive-only, declared-refs-only posture holds
/// end-to-end through the Rust fetch (tasks.md 5.6).
pub mod scope_status {
    pub const FULLY_GREEN_TASK_ID: &str = "s9-fixture#1";
    pub const FULLY_GREEN_SCENARIO_ID: &str = "s9.fixture.03";
    pub const FULLY_GREEN_TASK_STATUS: &str = "done";
    pub const FULLY_GREEN_EVIDENCE_COVERED: bool = true;
    pub const FULLY_GREEN_GREEN: bool = true;
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
            Envelope::new(1, RecordKind::Task, at(2026, 1, 1, 9), Actor::new_unattributed("fixture")),
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
            Envelope::new(1, RecordKind::Task, at(2026, 1, 1, 9), Actor::new_unattributed("fixture")),
            TaskId::parse("s9-fixture#2").unwrap(),
            "task two",
            TaskStatus::Done,
            Some("divergent evidence recorded".into()),
        )
        .with_scenario_refs(vec![ScenarioId::parse(scope_status::UNAUTHORED_SCENARIO_ID).unwrap()]),
    )
    .unwrap();
    tier.write(&Task::new(
        Envelope::new(1, RecordKind::Task, at(2026, 1, 1, 9), Actor::new_unattributed("fixture")),
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

    // ── session costs: 1 session, 1 run (with retrieved guidance
    // pointing at both role-memory strategies below), 2 token_usage
    // events ──────────────────────────────────────────────────────
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
    run.injected_guidance = vec![
        StrategyRef::new(dev_strategy_active_id(), "dev strategy", "content"),
        StrategyRef::new(content_strategy_id(), "content strategy", "content"),
    ];
    tier.write(&run).unwrap();

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

    // ── flywheel funnel: dev (2 trajectories / 3 verdict rows, 1
    // applied), content (1 trajectory / 1 verdict row, still pending) ──
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
            // stays pending — never marked, proving `applied` excludes it.
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
            .unwrap(),
        )
        .unwrap();
}
