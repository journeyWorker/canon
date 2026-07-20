//! The eleven non-`Handoff` record kinds (task 1.2). `Handoff` itself
//! lives in [`crate::handoff`] — its state machine and per-domain body
//! template registry (design D4/D5) are large enough to earn their own
//! module.
//!
//! Every type here composes [`Envelope`] via `#[serde(flatten)]` and
//! implements [`CanonRecord`]; every join-spine-key-shaped field uses
//! the matching newtype from [`crate::ids`], never a bare `String`.
//!
//! Field scope note: S1 owns the closed *kind set* and the join-spine
//! keys (design D1/D3) — the exact business-field shape of e.g. a
//! `Divergence` beyond its join keys and fold-ordering fields is
//! intentionally minimal here. Faithfully replicating the donor parity
//! harness's full axis-2 port-conformance system (manifest/review/
//! remediation JSONL) is real migration-mapping work that belongs to
//! S11 ("the donor parity harness is the FIRST migration target"), not
//! S1; these types carry the join keys and
//! fold-ordering fields S11 will need, without claiming to already be
//! that migration.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::envelope::{CanonRecord, Envelope, RecordKind};
use crate::ids::{
    is_kebab_slug, ChangeId, PrNumber, ProjectId, RegimeKey, RoleId, RunId, ScenarioId, Sha, SessionId, SpecDigest, SubjectId,
    TaskId, TotalOrder,
};
use crate::trust::{FlaggedOverlay, TrustLifecycle};

/// A `Change`'s lifecycle state (mirrors an openspec change's own
/// proposal → tasks → archive flow).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChangeStatus {
    Proposed,
    InProgress,
    Completed,
    Archived,
}

/// A change: the top of the join spine's `change_id` row (change ↔
/// tasks ↔ specs). `subject_id` (s36, additive — `#[serde(default,
/// skip_serializing_if = "Option::is_none")]`, so a pre-s36 `Change`
/// is byte-identical on the wire) links an imported plan change to the
/// durable [`Subject`] it was adopted under; it is a plain `pub` field
/// left `None` by [`Change::new`] and stamped on by `canon-cli`'s
/// `subject adopt` at adoption time (mirroring how
/// [`Session::project_key`] is set outside this crate), never derived
/// here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Change {
    #[serde(flatten)]
    pub envelope: Envelope,
    pub change_id: ChangeId,
    pub title: String,
    pub summary: String,
    pub status: ChangeStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_id: Option<SubjectId>,
}

impl Change {
    pub fn new(envelope: Envelope, change_id: ChangeId, title: impl Into<String>, summary: impl Into<String>, status: ChangeStatus) -> Self {
        debug_assert_eq!(envelope.kind, RecordKind::Change);
        Self { envelope, change_id, title: title.into(), summary: summary.into(), status, subject_id: None }
    }
}

impl CanonRecord for Change {
    const KIND: RecordKind = RecordKind::Change;
    fn envelope(&self) -> &Envelope {
        &self.envelope
    }
}

/// A `Subject`'s lifecycle (s36 design D1): the states a product unit
/// moves through, `proposed → specced → building → verifying → shipped
/// → retired`. Transitions are policy-gated (CEL) at the CLI/gate layer
/// (s35 seam); this enum owns only the closed set and its stable
/// snake_case wire spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SubjectStatus {
    Proposed,
    Specced,
    Building,
    Verifying,
    Shipped,
    Retired,
}

/// Validate a [`Subject`]'s `domain` at parse: SHAPE only — a kebab-case
/// slug (s36 design D2). The CLOSED base domain vocabulary (`planning`,
/// `design`, `dev`, `data`, `test`) lives in the `canon/vocab` plugin
/// (S10) and is extended per-repo there; canon-model deliberately does
/// NOT encode which domains a repo activates — it validates the slug
/// shape and nothing more, mirroring exactly how
/// [`crate::handoff::HandoffBody`]'s `domain` keeps its vocabulary out
/// of this crate. A PRESENT-but-malformed domain fails this record's
/// whole `Deserialize` (→ malformed, never silently kept); an absent
/// `domain` is a missing required field, also a hard error.
fn deserialize_domain_slug<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if !is_kebab_slug(&s) {
        return Err(serde::de::Error::custom(format!(
            "subject domain {s:?} is not a kebab-case slug (`[a-z0-9]+(-[a-z0-9]+)*`)"
        )));
    }
    Ok(s)
}

/// A subject (join-spine `subject_id` row: subject ↔ change ↔ scenario)
/// — the durable product/management unit a team plans, designs, builds,
/// and measures across many changes (s36, the reviewed 13th kind). A
/// by-id kind like [`Change`]: flat Hive partition, no mandatory
/// `scenario_id`. `domain` is a validated-shape-only kebab slug (see
/// [`deserialize_domain_slug`] — the closed vocabulary lives in
/// `canon/vocab`, not here); `owner_role` names the accountable role;
/// `change_ids`/`scenario_ids` are the join links accumulated as work
/// is adopted and specced against the subject (both additive-empty by
/// default, so a freshly-authored subject with no links yet is still a
/// valid, minimal record).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Subject {
    #[serde(flatten)]
    pub envelope: Envelope,
    pub subject_id: SubjectId,
    pub title: String,
    pub summary: String,
    #[serde(deserialize_with = "deserialize_domain_slug")]
    pub domain: String,
    pub status: SubjectStatus,
    pub owner_role: RoleId,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub change_ids: Vec<ChangeId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scenario_ids: Vec<ScenarioId>,
}

impl Subject {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        envelope: Envelope,
        subject_id: SubjectId,
        title: impl Into<String>,
        summary: impl Into<String>,
        domain: impl Into<String>,
        status: SubjectStatus,
        owner_role: RoleId,
    ) -> Self {
        debug_assert_eq!(envelope.kind, RecordKind::Subject);
        let domain = domain.into();
        debug_assert!(is_kebab_slug(&domain), "Subject.domain must be a kebab-case slug");
        Self {
            envelope,
            subject_id,
            title: title.into(),
            summary: summary.into(),
            domain,
            status,
            owner_role,
            change_ids: Vec::new(),
            scenario_ids: Vec::new(),
        }
    }

    /// Builder for the join links — `Subject::new`'s own signature stays
    /// unchanged (mirrors [`Task::with_scenario_refs`]).
    pub fn with_links(mut self, change_ids: Vec<ChangeId>, scenario_ids: Vec<ScenarioId>) -> Self {
        self.change_ids = change_ids;
        self.scenario_ids = scenario_ids;
        self
    }
}

impl CanonRecord for Subject {
    const KIND: RecordKind = RecordKind::Subject;
    fn envelope(&self) -> &Envelope {
        &self.envelope
    }
}

/// A `tasks.md` checkbox's canonical state — `Open` is the unchecked
/// `- [ ]`, `Done` the checked `- [x]`. `evidence_note` carries the
/// same "one-line evidence note" discipline this very change's own
/// tasks.md is held to — a `Done` task without one is a checkbox
/// overclaim, not evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Open,
    Done,
}

/// A task within a change (join-spine `task_id` row: task ↔ evidence ↔
/// trajectory). `change_id` is never a separate field — `task_id`
/// already embeds it; use [`TaskId::change_id`] to decompose.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Task {
    #[serde(flatten)]
    pub envelope: Envelope,
    pub task_id: TaskId,
    pub title: String,
    pub status: TaskStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_note: Option<String>,
    /// Optional, declaratively-authored scenario-coverage references
    /// (design s20 Decision 1) — which `Scenario`(s) this task is
    /// meant to satisfy, populated ONLY from an explicit `[covers:
    /// …]` `tasks.md` segment, never inferred from prose. Empty by
    /// default, mirroring `EvidenceRecord.surface_ref`'s own
    /// additive-field shape; a `Task` with no declared refs is
    /// byte-identical to a pre-s20 `Task` on the wire.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scenario_refs: Vec<ScenarioId>,
}

impl Task {
    pub fn new(envelope: Envelope, task_id: TaskId, title: impl Into<String>, status: TaskStatus, evidence_note: Option<String>) -> Self {
        debug_assert_eq!(envelope.kind, RecordKind::Task);
        Self { envelope, task_id, title: title.into(), status, evidence_note, scenario_refs: Vec::new() }
    }

    /// Builder for [`Task::scenario_refs`] — mirrors
    /// `EvidenceRecord::with_surface_ref`'s additive-field pattern;
    /// `Task::new`'s own signature stays unchanged.
    pub fn with_scenario_refs(mut self, scenario_refs: Vec<ScenarioId>) -> Self {
        self.scenario_refs = scenario_refs;
        self
    }
}

impl CanonRecord for Task {
    const KIND: RecordKind = RecordKind::Task;
    fn envelope(&self) -> &Envelope {
        &self.envelope
    }
}

/// A scenario (join-spine `scenario_id` row: spec ↔ test ↔ ledger ↔
/// divergence). A ledger INDEX record (design D2) — `project_id` +
/// `scenario_id` are the composite identity (design D6: `project_id`
/// is REQUIRED, clean-cutover, no legacy `Option` branch); `title`/
/// `description` are a denormalized nicety kept for free. The GENERAL
/// index is `envelope + project_id + scenario_id + title + description +
/// source_digest` ONLY (s15 P3a/task 3.3) — rich facts (steps,
/// provenance, `covered_by`) stay in the S11-validated family
/// documents; `canon inventory sync` derives this index from the
/// `.feature` corpus ALONE, never a second source of truth for those
/// documents. `covered`/`surface_ref` are deliberately NOT core fields
/// (P1 shipped them, P3a removed them). Coverage stays `canon-gate`'s
/// own `uncovered-cell` authority. Any donor-inventory-derived
/// enrichment (for example a donor `covered_by` join) stays
/// plugin-extensible: a future s16 porting plugin owns it as a
/// foreign-namespace overlay record, never as a field that core
/// re-materializes here, because a plugin cannot safely own a field
/// that core clobbers on every sync.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Scenario {
    #[serde(flatten)]
    pub envelope: Envelope,
    pub project_id: ProjectId,
    pub scenario_id: ScenarioId,
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// sha256-hex over the source `.feature` file's bytes (design D4) —
    /// the freshness signal `sync`'s logical-idempotence check compares
    /// against, NOT `ids::Sha` (a 40-hex git commit sha).
    pub source_digest: SpecDigest,
    /// The durable [`Subject`] this scenario is specced against (s36,
    /// additive — `#[serde(default, skip_serializing_if =
    /// "Option::is_none")]`, so a pre-s36 `Scenario` is byte-identical
    /// on the wire), mapped from a `.feature` scenario's
    /// `@subject:<subject-id>` Gherkin tag by `canon inventory sync`
    /// (mirrors `Change.subject_id`, which `subject adopt` stamps). A
    /// plain `pub` field left `None` by [`Scenario::new`] and populated
    /// outside this crate; a malformed or absent tag simply leaves it
    /// `None` (fail-soft), never a hard parse error here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject_id: Option<SubjectId>,
}

impl Scenario {
    pub fn new(envelope: Envelope, project_id: ProjectId, scenario_id: ScenarioId, title: impl Into<String>, description: impl Into<String>, source_digest: SpecDigest) -> Self {
        debug_assert_eq!(envelope.kind, RecordKind::Scenario);
        Self { envelope, project_id, scenario_id, title: title.into(), description: description.into(), source_digest, subject_id: None }
    }
}

impl CanonRecord for Scenario {
    const KIND: RecordKind = RecordKind::Scenario;
    fn envelope(&self) -> &Envelope {
        &self.envelope
    }
}

/// An agent-CLI session (join-spine `session_id` row: session ↔ cost ↔
/// run ↔ trajectory). `client` names the adapter that produced it (e.g.
/// `"claude-code"`, `"codex"`, `"omp"`) as a plain string, not a closed
/// enum — S3 (`canon-ingest`) owns the adapter registry; a `Session`
/// record itself is adapter-agnostic.
///
/// `workspace_key`/`workspace_label`/`project_key` (s31 D3, additive —
/// `#[serde(default, skip_serializing_if = "Option::is_none")]` on all
/// three, so a pre-s31 `Session` simply lacks the keys on reserialize):
/// `workspace_key`/`workspace_label` are populated by
/// `canon_ingest::normalize` from the session's own `UnifiedRow`/
/// `DirectiveRow` workspace context (first non-`None` across its rows);
/// `project_key` is left `None` by `canon-ingest` (a plain `pub` field,
/// never derived inside this crate or `canon-ingest` — s31 design D3:
/// "project_key set by the CLI layer") and stamped on directly by
/// `canon-cli`'s ingest pass once it has resolved the current project's
/// main-worktree key, so queries can aggregate a repo's main worktree
/// and its linked `git worktree`s as one project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Session {
    #[serde(flatten)]
    pub envelope: Envelope,
    pub session_id: SessionId,
    pub client: String,
    pub started_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_key: Option<String>,
}

impl Session {
    pub fn new(envelope: Envelope, session_id: SessionId, client: impl Into<String>, started_at: DateTime<Utc>, ended_at: Option<DateTime<Utc>>) -> Self {
        debug_assert_eq!(envelope.kind, RecordKind::Session);
        Self { envelope, session_id, client: client.into(), started_at, ended_at, workspace_key: None, workspace_label: None, project_key: None }
    }
}

impl CanonRecord for Session {
    const KIND: RecordKind = RecordKind::Session;
    fn envelope(&self) -> &Envelope {
        &self.envelope
    }
}

/// A run's lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Aborted,
}

/// A full content SNAPSHOT of one retrieved strategy (S8 design D2),
/// never a live pointer: `strategy_id` names its source
/// `canon-learn::StrategyItem` for provenance only — `title`/`content`
/// are copied by value at the moment they were shown to an agent, so a
/// later edit or demotion of the source strategy can never retroactively
/// change what a [`Run`]'s [`Run::injected_guidance`] already recorded
/// (the "replay reproduces byte-identical run inputs" guarantee this
/// type exists to make possible). Deliberately NOT a `CanonRecord`
/// itself — it only ever lives nested inside [`Run::injected_guidance`],
/// mirroring how [`crate::envelope::Actor`] composes into [`Envelope`]
/// without being one of the twelve closed record kinds itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StrategyRef {
    pub strategy_id: String,
    pub title: String,
    pub content: String,
}

impl StrategyRef {
    pub fn new(strategy_id: impl Into<String>, title: impl Into<String>, content: impl Into<String>) -> Self {
        Self { strategy_id: strategy_id.into(), title: title.into(), content: content.into() }
    }
}

/// A run (join-spine `run_id` row: run ↔ events ↔ manifest).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Run {
    #[serde(flatten)]
    pub envelope: Envelope,
    pub run_id: RunId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<SessionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    pub status: RunStatus,
    pub started_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<DateTime<Utc>>,
    /// Strategy guidance injected at dispatch time (S8 design D2), a
    /// verbatim snapshot of what `retrieve_guidance` returned then —
    /// see [`StrategyRef`]. `#[serde(default, skip_serializing_if =
    /// "Vec::is_empty")]` is load-bearing, the same backward/forward-
    /// compat discipline `canon_learn::StrategyItem::demotion` already
    /// establishes: a pre-S8 manifest with no `injected_guidance` key
    /// deserializes to an empty `Vec`, AND a `Run` with empty guidance
    /// reserializes WITHOUT the key at all — an S8 build never
    /// perturbs an S7-era manifest's on-disk shape for the (still
    /// overwhelmingly common) no-guidance case.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub injected_guidance: Vec<StrategyRef>,
}

impl Run {
    pub fn new(
        envelope: Envelope,
        run_id: RunId,
        session_id: Option<SessionId>,
        task_id: Option<TaskId>,
        status: RunStatus,
        started_at: DateTime<Utc>,
        ended_at: Option<DateTime<Utc>>,
    ) -> Self {
        debug_assert_eq!(envelope.kind, RecordKind::Run);
        Self { envelope, run_id, session_id, task_id, status, started_at, ended_at, injected_guidance: Vec::new() }
    }

    /// Records S8's retrieved guidance into this run's manifest (design
    /// decision 2) — meant to be called exactly ONCE, at dispatch time,
    /// mirroring the donor tuning project's sweep-manifest injected-guidance write.
    /// Never merges with any prior value; a second call simply replaces
    /// it, since a `Run` is only ever dispatched once (S1's own
    /// `RunStatus` lifecycle has no "re-dispatch" transition).
    pub fn with_injected_guidance(mut self, injected_guidance: Vec<StrategyRef>) -> Self {
        self.injected_guidance = injected_guidance;
        self
    }
}

impl CanonRecord for Run {
    const KIND: RecordKind = RecordKind::Run;
    fn envelope(&self) -> &Envelope {
        &self.envelope
    }
}

/// One event within a run (join-spine `run_id` row: run ↔ events ↔
/// manifest). `detail` is deliberately open (`serde_json::Value`) —
/// events are the most heterogeneous record kind (tool calls, token
/// deltas, tool errors, …); narrowing `detail` to a closed shape is a
/// later, per-event-family change, not S1's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Event {
    #[serde(flatten)]
    pub envelope: Envelope,
    pub run_id: RunId,
    pub seq: u64,
    pub label: String,
    #[serde(default)]
    pub detail: serde_json::Value,
}

impl Event {
    pub fn new(envelope: Envelope, run_id: RunId, seq: u64, label: impl Into<String>, detail: serde_json::Value) -> Self {
        debug_assert_eq!(envelope.kind, RecordKind::Event);
        Self { envelope, run_id, seq, label: label.into(), detail }
    }
}

impl CanonRecord for Event {
    const KIND: RecordKind = RecordKind::Event;
    fn envelope(&self) -> &Envelope {
        &self.envelope
    }
}

/// Which provenance a `Review` cites — exactly one of the two ref
/// fields the donor parity harness's review-ref set (`upstream_ref`,
/// `original_spec_ref`) requires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceRef {
    UpstreamRef(String),
    OriginalSpecRef(String),
}

/// A review attestation (join-spine `scenario_id` row: spec ↔ test ↔
/// ledger ↔ divergence). Mirrors the donor parity harness's required
/// review fields (`scenario_id`, `reviewer`, `pin`) plus its
/// provenance-ref requirement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Review {
    #[serde(flatten)]
    pub envelope: Envelope,
    pub project_id: ProjectId,
    pub scenario_id: ScenarioId,
    pub reviewer: String,
    pub pin: String,
    pub provenance_ref: ProvenanceRef,
}

impl Review {
    pub fn new(
        envelope: Envelope,
        project_id: ProjectId,
        scenario_id: ScenarioId,
        reviewer: impl Into<String>,
        pin: impl Into<String>,
        provenance_ref: ProvenanceRef,
    ) -> Self {
        debug_assert_eq!(envelope.kind, RecordKind::Review);
        Self { envelope, project_id, scenario_id, reviewer: reviewer.into(), pin: pin.into(), provenance_ref }
    }
}

impl CanonRecord for Review {
    const KIND: RecordKind = RecordKind::Review;
    fn envelope(&self) -> &Envelope {
        &self.envelope
    }
}

/// A divergence's fold-relevant state (design D8). `Open`/`Resolved`
/// are the pre-s15 pair (still bare `"open"`/`"resolved"` on the wire —
/// unaffected by the two additive variants below); `StillDivergent` is
/// a re-review that found the divergence persists; `Deferred` postpones
/// review until `expiry` (honored by [`crate::fold::fold_to_current_state`]'s
/// `as_of` parameter). `ResolvedInvalid` deliberately does NOT exist
/// here — it is a fold-time-DERIVED [`crate::fold::FoldedState`]
/// output, never a persisted status (design D8/D9: the on-disk record
/// is never rewritten to reflect a stale binding).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DivergenceStatus {
    Open,
    Resolved,
    StillDivergent,
    Deferred { reason: String, expiry: DateTime<Utc> },
}

/// A tracked divergence (join-spine `scenario_id` row). `run_seq`/
/// `round` are the fold-ordering fields the design doc's Risk section
/// calls out for `Divergence`/`EvidenceRecord` — mirrors
/// `divergence-log.md`'s "serialized-integrator monotonic `run_seq`
/// (primary), `round` (tiebreak-only)" fold rule; the fold algorithm
/// itself lives in [`crate::fold`]. `project_id` is REQUIRED (design
/// D6, clean cutover) and, together with `scenario_id`, is the fold's
/// grouping key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Divergence {
    #[serde(flatten)]
    pub envelope: Envelope,
    pub project_id: ProjectId,
    pub scenario_id: ScenarioId,
    pub sha: Sha,
    pub status: DivergenceStatus,
    /// The SOLE primary fold-ordering key within a `(project_id,
    /// scenario_id)` group — `round` below is a tiebreak ONLY among
    /// equal `run_seq` values, never an independent ordering axis.
    pub run_seq: TotalOrder,
    pub round: u32,
    pub reviewer: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detail: String,
}

impl Divergence {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        envelope: Envelope,
        project_id: ProjectId,
        scenario_id: ScenarioId,
        sha: Sha,
        status: DivergenceStatus,
        run_seq: TotalOrder,
        round: u32,
        reviewer: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        debug_assert_eq!(envelope.kind, RecordKind::Divergence);
        Self { envelope, project_id, scenario_id, sha, status, run_seq, round, reviewer: reviewer.into(), detail: detail.into() }
    }
}

impl CanonRecord for Divergence {
    const KIND: RecordKind = RecordKind::Divergence;
    fn envelope(&self) -> &Envelope {
        &self.envelope
    }
}

/// A reward-eligible trajectory (join-spine rows: `sha`/`pr` — reward
/// signals ↔ trajectory; `session_id` — session ↔ cost ↔ run ↔
/// trajectory; `task_id` via `run_id` — task ↔ evidence ↔ trajectory).
/// `reward` is a bare numeric signal here; S7 (`reward-statistical-
/// promotion`) owns how it is computed and aggregated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Trajectory {
    #[serde(flatten)]
    pub envelope: Envelope,
    pub run_id: RunId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<SessionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha: Option<Sha>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<PrNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reward: Option<f64>,
}

impl Trajectory {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        envelope: Envelope,
        run_id: RunId,
        task_id: Option<TaskId>,
        session_id: Option<SessionId>,
        sha: Option<Sha>,
        pr: Option<PrNumber>,
        reward: Option<f64>,
    ) -> Self {
        debug_assert_eq!(envelope.kind, RecordKind::Trajectory);
        Self { envelope, run_id, task_id, session_id, sha, pr, reward }
    }
}

impl CanonRecord for Trajectory {
    const KIND: RecordKind = RecordKind::Trajectory;
    fn envelope(&self) -> &Envelope {
        &self.envelope
    }
}

/// A written strategy insight (join-spine `regime_key` row: strategy
/// write ↔ retrieval, identical at both ends). S6
/// (`role-strategy-memory`) owns retrieval/promotion policy; this type
/// only carries the join key + content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StrategyItem {
    #[serde(flatten)]
    pub envelope: Envelope,
    pub regime_key: RegimeKey,
    pub role: RoleId,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_task_id: Option<TaskId>,
}

impl StrategyItem {
    pub fn new(envelope: Envelope, regime_key: RegimeKey, role: RoleId, content: impl Into<String>, source_task_id: Option<TaskId>) -> Self {
        debug_assert_eq!(envelope.kind, RecordKind::StrategyItem);
        Self { envelope, regime_key, role, content: content.into(), source_task_id }
    }
}

impl CanonRecord for StrategyItem {
    const KIND: RecordKind = RecordKind::StrategyItem;
    fn envelope(&self) -> &Envelope {
        &self.envelope
    }
}

/// The three-way verdict an evidence record carries. `Divergent` is an
/// explicit third value rather than "no record exists" — the donor
/// parity harness represents non-faithful as *absence* of a
/// design-review/code-review record, which works for a directory-scanned
/// ledger but not for a single, standalone canon record whose own
/// `Deserialize` must always succeed or fail on its own — so canon makes
/// the state explicit instead of leaning on record-absence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceVerdict {
    Faithful,
    NotApplicable,
    Divergent,
}

/// Deserialize a PRESENT field's value into `Some(T)`, rejecting an
/// explicit JSON `null`. Serde invokes a `deserialize_with` ONLY for a
/// key that is actually present; a MISSING key is handled by the field's
/// `#[serde(default)]` (→ `None`). So pairing this with `default` gives
/// the three-way read (design D9 / R3, `gate-native-record-fields` spec):
/// absent key → `None` (safe default), present well-formed → `Some(T)`,
/// present `null` (or any malformed value) → this whole record's
/// `Deserialize` fails, so it lands as `malformed-evidence` rather than
/// silently collapsing to the absent default (which would let a
/// `"flagged": null` dodge the human-only flag ratchet).
fn present_value<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// The evidence-integrity spec's own record kind: the candidate shape
/// [`crate::evidence::validate_evidence`] validates. Carries whichever
/// join keys are relevant to what it attests (join-spine `task_id`
/// row: task ↔ evidence ↔ trajectory). `project_id` is OPTIONAL
/// (design D6: real records may exist via `canon gate promote` before
/// every producer is project-aware). The five trailing fields are s15's
/// native home for what were `canon-gate`-owned raw-JSON companions
/// (design D9) — each reads THREE-way: a legitimately ABSENT key
/// deserializes to `None`/empty (the documented safe default —
/// `canon-gate` still owns what that default MEANS per field, e.g.
/// absent `lifecycle` = draft), a PRESENT well-formed value deserializes
/// typed, and a PRESENT malformed value fails this whole record's
/// `Deserialize` — never silently collapsed to absent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceRecord {
    #[serde(flatten)]
    pub envelope: Envelope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<ProjectId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scenario_id: Option<ScenarioId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<RunId>,
    pub verdict: EvidenceVerdict,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "present_value")]
    pub lifecycle: Option<TrustLifecycle>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "present_value")]
    pub flagged: Option<FlaggedOverlay>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "present_value")]
    pub evidence_sha: Option<Sha>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "present_value")]
    pub run_seq: Option<TotalOrder>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surface_ref: Vec<String>,
}

impl EvidenceRecord {
    /// Unchanged signature (s15 task 1.5: "keep existing callers
    /// working") — every field this constructor doesn't take defaults
    /// to `None`/empty, mirroring [`Run::new`]'s
    /// `injected_guidance: Vec::new()` precedent for an additive field
    /// no early caller needs to set. Use the `with_*` builders below to
    /// set them.
    pub fn new(envelope: Envelope, task_id: Option<TaskId>, scenario_id: Option<ScenarioId>, run_id: Option<RunId>, verdict: EvidenceVerdict) -> Self {
        debug_assert_eq!(envelope.kind, RecordKind::EvidenceRecord);
        Self {
            envelope,
            project_id: None,
            task_id,
            scenario_id,
            run_id,
            verdict,
            lifecycle: None,
            flagged: None,
            evidence_sha: None,
            run_seq: None,
            surface_ref: Vec::new(),
        }
    }

    pub fn with_project_id(mut self, project_id: ProjectId) -> Self {
        self.project_id = Some(project_id);
        self
    }

    pub fn with_lifecycle(mut self, lifecycle: TrustLifecycle) -> Self {
        self.lifecycle = Some(lifecycle);
        self
    }

    pub fn with_flagged(mut self, flagged: FlaggedOverlay) -> Self {
        self.flagged = Some(flagged);
        self
    }

    pub fn with_evidence_sha(mut self, evidence_sha: Sha) -> Self {
        self.evidence_sha = Some(evidence_sha);
        self
    }

    pub fn with_run_seq(mut self, run_seq: TotalOrder) -> Self {
        self.run_seq = Some(run_seq);
        self
    }

    pub fn with_surface_ref(mut self, surface_ref: Vec<String>) -> Self {
        self.surface_ref = surface_ref;
        self
    }
}

impl CanonRecord for EvidenceRecord {
    const KIND: RecordKind = RecordKind::EvidenceRecord;
    fn envelope(&self) -> &Envelope {
        &self.envelope
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::Actor;
    use crate::ids::RoleId;

    fn envelope(kind: RecordKind) -> Envelope {
        Envelope::new(1, kind, Utc::now(), Actor::new("codex-cli", RoleId::parse("implementer").unwrap()))
    }

    fn project_id() -> ProjectId {
        ProjectId::parse("root").unwrap()
    }

    macro_rules! round_trip_test {
        ($fn_name:ident, $value:expr) => {
            #[test]
            fn $fn_name() {
                let original = $value;
                let json = serde_json::to_value(&original).unwrap();
                assert!(json.get("schema").is_some());
                assert!(json.get("kind").is_some());
                assert!(json.get("at").is_some());
                let actor = json.get("actor").and_then(|a| a.as_object()).expect("actor object");
                assert!(actor.get("agent_id").is_some());
                assert!(actor.get("role").is_some());
                let round_tripped = serde_json::from_value(json).unwrap();
                assert_eq!(original, round_tripped);
            }
        };
    }

    round_trip_test!(
        change_round_trips,
        Change::new(envelope(RecordKind::Change), ChangeId::parse("s1-state-model-join-spine").unwrap(), "S1", "join spine", ChangeStatus::InProgress)
    );

    round_trip_test!(
        change_with_subject_id_round_trips,
        {
            let mut c = Change::new(
                envelope(RecordKind::Change),
                ChangeId::parse("s36-subject-domain-loop").unwrap(),
                "S36",
                "subject loop",
                ChangeStatus::InProgress,
            );
            c.subject_id = Some(SubjectId::parse("subject-domain-loop").unwrap());
            c
        }
    );

    round_trip_test!(
        subject_round_trips,
        Subject::new(
            envelope(RecordKind::Subject),
            SubjectId::parse("subject-domain-loop").unwrap(),
            "subject-domain loop",
            "the durable product unit",
            "dev",
            SubjectStatus::Building,
            RoleId::parse("implementer").unwrap(),
        )
        .with_links(
            vec![ChangeId::parse("s36-subject-domain-loop").unwrap()],
            vec![ScenarioId::parse("world.subject-loop.01").unwrap()],
        )
    );

    round_trip_test!(
        subject_without_links_round_trips,
        Subject::new(
            envelope(RecordKind::Subject),
            SubjectId::parse("payments").unwrap(),
            "payments",
            "billing subject",
            "planning",
            SubjectStatus::Proposed,
            RoleId::parse("planner").unwrap(),
        )
    );

    /// A pre-s36 `Change` (no `subject_id` key at all) still
    /// deserializes to `subject_id: None` and never reserializes a
    /// spurious `"subject_id": null` — the additive-field bar this
    /// change is held to.
    #[test]
    fn change_without_subject_id_key_deserializes_none_and_reserializes_without_the_key() {
        let json = serde_json::json!({
            "schema": 1,
            "kind": "change",
            "at": "2026-07-20T12:00:00Z",
            "actor": {"agent_id": "codex-cli", "role": "implementer"},
            "change_id": "s1-state-model-join-spine",
            "title": "S1",
            "summary": "join spine",
            "status": "in_progress"
        });
        let change: Change = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(change.subject_id, None);
        assert_eq!(serde_json::to_value(&change).unwrap(), json);
    }

    /// A `Subject` whose `domain` is present but not a kebab slug fails
    /// this record's whole `Deserialize` (design D2: validated shape
    /// only, but a malformed shape is never silently kept) — the
    /// closed vocabulary itself is `canon/vocab`'s (S10) concern, not
    /// this crate's.
    #[test]
    fn subject_with_malformed_domain_fails_to_deserialize() {
        let json = serde_json::json!({
            "schema": 1,
            "kind": "subject",
            "at": "2026-07-20T12:00:00Z",
            "actor": {"agent_id": "codex-cli", "role": "implementer"},
            "subject_id": "subject-domain-loop",
            "title": "t",
            "summary": "s",
            "domain": "Not A Slug",
            "status": "building",
            "owner_role": "implementer"
        });
        assert!(serde_json::from_value::<Subject>(json).is_err());
    }

    round_trip_test!(
        task_round_trips,
        Task::new(envelope(RecordKind::Task), TaskId::parse("s1-state-model-join-spine#6.2").unwrap(), "fixtures", TaskStatus::Done, Some("evidence".into()))
    );

    round_trip_test!(
        task_with_scenario_refs_round_trips,
        Task::new(envelope(RecordKind::Task), TaskId::parse("s1-state-model-join-spine#6.2").unwrap(), "fixtures", TaskStatus::Done, Some("evidence".into()))
            .with_scenario_refs(vec![ScenarioId::parse("wall.render.01").unwrap(), ScenarioId::parse("wall.render.02").unwrap()])
    );

    /// A `Task` from before s20 (no `scenario_refs` key at all in the
    /// JSON) still deserializes — to an empty `Vec` — and every
    /// existing field/behavior is byte-identical to before this
    /// change (task-scenario-join spec, "A Task with no declared
    /// scenario refs is unchanged"). Forward stability mirrors
    /// `Run.injected_guidance`'s own bar: an empty `scenario_refs`
    /// never reserializes a spurious `"scenario_refs": []` key.
    #[test]
    fn task_without_scenario_refs_key_deserializes_empty_and_reserializes_without_the_key() {
        let at = serde_json::to_value(Utc::now()).unwrap();
        let pre_s20_json = serde_json::json!({
            "schema": 1,
            "kind": "task",
            "at": at,
            "actor": {"agent_id": "codex-cli", "role": "implementer"},
            "task_id": "s1-state-model-join-spine#6.2",
            "title": "fixtures",
            "status": "done",
            "evidence_note": "evidence",
        });

        let task: Task = serde_json::from_value(pre_s20_json.clone()).expect("a pre-s20 Task with no scenario_refs key must still deserialize");
        assert!(task.scenario_refs.is_empty());

        let reserialized = serde_json::to_value(&task).unwrap();
        assert_eq!(reserialized, pre_s20_json, "empty scenario_refs must not introduce a spurious key on reserialize");
    }

    round_trip_test!(
        scenario_round_trips,
        Scenario::new(
            envelope(RecordKind::Scenario),
            project_id(),
            ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap(),
            "hotdeal",
            "desc",
            SpecDigest::of(b"fixture .feature bytes"),
        )
    );

    round_trip_test!(
        scenario_with_subject_id_round_trips,
        {
            let mut s = Scenario::new(
                envelope(RecordKind::Scenario),
                project_id(),
                ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap(),
                "hotdeal",
                "desc",
                SpecDigest::of(b"fixture .feature bytes"),
            );
            s.subject_id = Some(SubjectId::parse("subject-domain-loop").unwrap());
            s
        }
    );

    /// A pre-s36 `Scenario` (no `subject_id` key at all) still
    /// deserializes to `subject_id: None` and never reserializes a
    /// spurious `"subject_id": null` — the additive-field bar, mirroring
    /// `Change.subject_id`.
    #[test]
    fn scenario_without_subject_id_key_deserializes_none_and_reserializes_without_the_key() {
        let at = serde_json::to_value(Utc::now()).unwrap();
        let json = serde_json::json!({
            "schema": 1,
            "kind": "scenario",
            "at": at,
            "actor": {"agent_id": "codex-cli", "role": "implementer"},
            "project_id": "root",
            "scenario_id": "world.firstbuy-hotdeal.26",
            "title": "hotdeal",
            "source_digest": SpecDigest::of(b"fixture .feature bytes").to_string(),
        });
        let scenario: Scenario = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(scenario.subject_id, None);
        assert_eq!(serde_json::to_value(&scenario).unwrap(), json);
    }

    round_trip_test!(
        session_round_trips,
        Session::new(envelope(RecordKind::Session), SessionId::parse("f47ac10b-58cc-4372-a567-0e02b2c3d479").unwrap(), "claude-code", Utc::now(), None)
    );

    /// A pre-s31 `Session` JSON (no `workspace_key`/`workspace_label`/
    /// `project_key` keys at all) still deserializes — to `None` on all
    /// three — and reserializes back to the IDENTICAL shape, never
    /// introducing a spurious key (design D3: "Sessions ingested
    /// pre-s31 simply lack the fields"), same backward/forward-compat
    /// bar `Run.injected_guidance`/`Task.scenario_refs` already set.
    #[test]
    fn session_without_workspace_keys_deserializes_none_and_reserializes_without_the_keys() {
        let at = serde_json::to_value(Utc::now()).unwrap();
        let started_at = serde_json::to_value(Utc::now()).unwrap();
        let pre_s31_json = serde_json::json!({
            "schema": 1,
            "kind": "session",
            "at": at,
            "actor": {"agent_id": "codex-cli", "role": "implementer"},
            "session_id": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
            "client": "claude-code",
            "started_at": started_at,
        });

        let session: Session = serde_json::from_value(pre_s31_json.clone()).expect("a pre-s31 Session with no workspace/project keys must still deserialize");
        assert!(session.workspace_key.is_none());
        assert!(session.workspace_label.is_none());
        assert!(session.project_key.is_none());

        let reserialized = serde_json::to_value(&session).unwrap();
        assert_eq!(reserialized, pre_s31_json, "None workspace/project fields must not introduce spurious keys on reserialize");
    }

    round_trip_test!(
        run_round_trips,
        Run::new(envelope(RecordKind::Run), RunId::new(), None, None, RunStatus::Succeeded, Utc::now(), Some(Utc::now()))
    );

    round_trip_test!(
        run_with_injected_guidance_round_trips,
        Run::new(envelope(RecordKind::Run), RunId::new(), None, None, RunStatus::Succeeded, Utc::now(), Some(Utc::now()))
            .with_injected_guidance(vec![
                StrategyRef::new("01ARZ3NDEKTSV4RRFFQ69G5FAV", "title", "content"),
                StrategyRef::new("01ARZ3NDEKTSV4RRFFQ69G5FAW", "title2", "content2"),
            ])
    );

    /// S7-era `Run`/manifest JSON — the exact shape `Run::new` produced
    /// before this change added `injected_guidance` — has no
    /// `injected_guidance` key at all. Backward compat: it must still
    /// deserialize (to an empty `Vec`). Forward stability: a `Run` with
    /// empty guidance must reserialize to the IDENTICAL shape, never
    /// introducing a spurious `"injected_guidance": []` — so an S8
    /// build never perturbs an existing, still-overwhelmingly-common
    /// no-guidance manifest on disk.
    #[test]
    fn run_without_injected_guidance_key_deserializes_empty_and_reserializes_without_the_key() {
        // Round DateTime<Utc> values through serde's own encoding
        // (chrono's serde feature emits `Z`, not `to_rfc3339`'s
        // `+00:00`) so the hand-built JSON matches exactly what `Run`
        // itself serializes to — otherwise the byte-stability
        // assertion below would fail on timestamp formatting alone,
        // not on the `injected_guidance` key it's meant to test.
        let at = serde_json::to_value(Utc::now()).unwrap();
        let started_at = serde_json::to_value(Utc::now()).unwrap();
        let pre_s8_json = serde_json::json!({
            "schema": 1,
            "kind": "run",
            "at": at,
            "actor": {"agent_id": "codex-cli", "role": "implementer"},
            "run_id": RunId::new().to_string(),
            "status": "succeeded",
            "started_at": started_at,
        });

        let run: Run = serde_json::from_value(pre_s8_json.clone()).expect("a pre-S8 manifest with no injected_guidance key must still deserialize");
        assert!(run.injected_guidance.is_empty());

        let reserialized = serde_json::to_value(&run).unwrap();
        assert_eq!(reserialized, pre_s8_json, "empty injected_guidance must not introduce a spurious key on reserialize");
    }

    round_trip_test!(
        event_round_trips,
        Event::new(envelope(RecordKind::Event), RunId::new(), 1, "tool_call", serde_json::json!({"tool": "read"}))
    );

    round_trip_test!(
        review_round_trips,
        Review::new(
            envelope(RecordKind::Review),
            project_id(),
            ScenarioId::parse("world.place-lock.01").unwrap(),
            "reviewer",
            "9c93d024b1a2",
            ProvenanceRef::UpstreamRef("routes/world.tsx#onPurchased".into())
        )
    );

    round_trip_test!(
        divergence_round_trips,
        Divergence::new(
            envelope(RecordKind::Divergence),
            project_id(),
            ScenarioId::parse("world.place-lock.01").unwrap(),
            Sha::parse("8c81f9e13e9bda0a6a5ee29ba1b6b5137e7bf552").unwrap(),
            DivergenceStatus::Open,
            TotalOrder::new(3),
            8,
            "reviewer",
            "detail"
        )
    );

    #[test]
    fn pre_s15_divergence_status_open_and_resolved_still_deserialize() {
        assert_eq!(serde_json::from_value::<DivergenceStatus>(serde_json::json!("open")).unwrap(), DivergenceStatus::Open);
        assert_eq!(serde_json::from_value::<DivergenceStatus>(serde_json::json!("resolved")).unwrap(), DivergenceStatus::Resolved);
    }

    #[test]
    fn divergence_status_still_divergent_and_deferred_round_trip() {
        assert_eq!(
            serde_json::from_value::<DivergenceStatus>(serde_json::json!("still_divergent")).unwrap(),
            DivergenceStatus::StillDivergent
        );
        let deferred = DivergenceStatus::Deferred { reason: "needs re-review".into(), expiry: Utc::now() };
        let json = serde_json::to_value(&deferred).unwrap();
        assert_eq!(serde_json::from_value::<DivergenceStatus>(json).unwrap(), deferred);
    }

    round_trip_test!(
        trajectory_round_trips,
        Trajectory::new(envelope(RecordKind::Trajectory), RunId::new(), None, None, None, Some(PrNumber::parse(42).unwrap()), Some(0.5))
    );

    round_trip_test!(
        strategy_item_round_trips,
        StrategyItem::new(
            envelope(RecordKind::StrategyItem),
            RegimeKey::parse("implementer/canon/join-spine/9c93d024b1a2").unwrap(),
            RoleId::parse("implementer").unwrap(),
            "content",
            None
        )
    );

    round_trip_test!(
        evidence_record_round_trips,
        EvidenceRecord::new(envelope(RecordKind::EvidenceRecord), None, None, None, EvidenceVerdict::Faithful)
    );

    #[test]
    fn evidence_record_with_no_native_fields_deserializes_to_defaults() {
        let at = serde_json::to_value(Utc::now()).unwrap();
        let json = serde_json::json!({
            "schema": 1,
            "kind": "evidence_record",
            "at": at,
            "actor": {"agent_id": "codex-cli", "role": "implementer"},
            "verdict": "faithful",
        });
        let record: EvidenceRecord =
            serde_json::from_value(json).expect("a pre-s15 EvidenceRecord with none of the five native fields must still deserialize");
        assert!(record.project_id.is_none());
        assert!(record.lifecycle.is_none());
        assert!(record.flagged.is_none());
        assert!(record.evidence_sha.is_none());
        assert!(record.run_seq.is_none());
        assert!(record.surface_ref.is_empty());
    }

    #[test]
    fn evidence_record_with_present_but_garbage_flagged_fails_to_deserialize() {
        let at = serde_json::to_value(Utc::now()).unwrap();
        let json = serde_json::json!({
            "schema": 1,
            "kind": "evidence_record",
            "at": at,
            "actor": {"agent_id": "codex-cli", "role": "implementer"},
            "verdict": "faithful",
            "flagged": {"not_a_valid_flagged_shape": true},
        });
        assert!(
            serde_json::from_value::<EvidenceRecord>(json).is_err(),
            "a present-but-malformed `flagged` must fail deserialize, never silently default to absent"
        );
    }

    #[test]
    fn evidence_record_with_an_explicit_null_native_field_fails_to_deserialize() {
        // A present `null` is present-but-malformed, NOT absent: it must
        // fail the record, never silently read as the absent default —
        // otherwise `"flagged": null` would be a silent flag-clear that
        // dodges the human-only ratchet (design D9 / R3).
        let at = serde_json::to_value(Utc::now()).unwrap();
        for field in ["lifecycle", "flagged", "evidence_sha", "run_seq"] {
            let mut obj = serde_json::json!({
                "schema": 1,
                "kind": "evidence_record",
                "at": at,
                "actor": {"agent_id": "codex-cli", "role": "implementer"},
                "verdict": "faithful",
            });
            obj[field] = serde_json::Value::Null;
            assert!(
                serde_json::from_value::<EvidenceRecord>(obj).is_err(),
                "a present `{field}: null` must fail deserialize, never collapse to the absent default"
            );
        }
    }
}
