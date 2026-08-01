//! The thirteen non-`Handoff` record kinds (task 1.2). `Handoff` itself
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
use crate::evidence::{EvidenceViolation, FailureClass};
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
    /// Optional, declaratively-authored DEPENDENCY references (s37
    /// `execution-graph-topology`, subject `flywheel-execution-graph`)
    /// — which sibling `Task`(s) in the SAME change this task's own
    /// plan text declares it comes after. Populated ONLY from a
    /// dependency expression the dialect's corpus demonstrably uses
    /// (the openspec dialect's in-row `depends on <n>`/`after <n>`
    /// marker prose; the superpowers dialect's `- Consumes: … from
    /// Task <n>` interface line), never from a syntax invented for
    /// canon's convenience and never from dotted-numbering nesting —
    /// `<change>#6.2` does NOT implicitly depend on `<change>#6`. A
    /// reference that fails to resolve to a real sibling row is
    /// DROPPED with a named import diagnostic, never an import
    /// failure, because prose is ambiguous and "malformed evidence is
    /// no evidence".
    ///
    /// DECLARED INTENT ONLY: canon never schedules, orders, blocks, or
    /// executes anything from this field. It is the PLAN-side half of
    /// the execution graph — the half s37's plan-vs-actual diff reads
    /// against `Run.parent_run_id`'s OBSERVED dispatch lineage, which
    /// is what makes a declared-but-never-honored ordering visible to
    /// the reward flywheel at all.
    ///
    /// Empty by default, mirroring [`Task::scenario_refs`]'s own
    /// additive-field shape (and, like it, `skip_serializing_if` so an
    /// empty vec never introduces a spurious key — every pre-s37
    /// `Task` stays byte-identical on the wire, and its content digest
    /// with it).
    ///
    /// This field IS nonetheless what bumped `Task` to
    /// [`RecordKind::schema_version`] `2`
    /// (`s38-evidence-bearing-memory`), and NOT because the wire form
    /// broke — it did not. A `Task`'s `Envelope.at` is
    /// `file_modified_at(<source plan doc>)` (s20 D7), byte-stable so an
    /// unchanged plan re-imports idempotently; adding this field changed
    /// the PARSER, not the source document's mtime, so the stale and
    /// fresh record for one `task_id` carry an IDENTICAL `at` and
    /// `canon_store::fold::fold_latest_by_key` needs a generation signal
    /// to order them. Without the bump it fell through to the
    /// lexicographic digest tie-break and surfaced this field on an
    /// arbitrary SUBSET of one file's rows. See
    /// [`RecordKind::schema_version`] for why `Run`/`Handoff` gained
    /// fields on the same branch and correctly stayed at `1`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<TaskId>,
}

impl Task {
    pub fn new(envelope: Envelope, task_id: TaskId, title: impl Into<String>, status: TaskStatus, evidence_note: Option<String>) -> Self {
        debug_assert_eq!(envelope.kind, RecordKind::Task);
        Self { envelope, task_id, title: title.into(), status, evidence_note, scenario_refs: Vec::new(), depends_on: Vec::new() }
    }

    /// Builder for [`Task::scenario_refs`] — mirrors
    /// `EvidenceRecord::with_surface_ref`'s additive-field pattern;
    /// `Task::new`'s own signature stays unchanged.
    pub fn with_scenario_refs(mut self, scenario_refs: Vec<ScenarioId>) -> Self {
        self.scenario_refs = scenario_refs;
        self
    }

    /// Builder for [`Task::depends_on`] — mirrors
    /// [`Task::with_scenario_refs`]'s additive-field pattern; `Task::
    /// new`'s own signature stays unchanged. A plan adapter calls this
    /// only AFTER it has read the change's complete row set, since a
    /// declared reference is resolved against that set (a reference to
    /// a row that does not exist is dropped, not carried).
    pub fn with_depends_on(mut self, depends_on: Vec<TaskId>) -> Self {
        self.depends_on = depends_on;
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
    /// The run that DISPATCHED this run — a subagent run's parent,
    /// `None` for a root run. canon's ONLY representation of agent
    /// execution topology: without it a session's runs are a flat bag
    /// and a dispatch tree (main agent -> N subagents -> their own
    /// subagents) is unrecoverable after ingest, which is exactly what
    /// the plan-vs-actual graph diff needs to compare a declared
    /// [`Task::depends_on`] plan against what actually ran.
    ///
    /// A pointer along the SAME join-spine `run_id` key this record is
    /// already keyed by — deliberately not a new join key, so
    /// `run <-> events <-> manifest` stays the one run-scoped spine.
    /// `#[serde(default, skip_serializing_if = "Option::is_none")]`
    /// carries the same backward/forward-compat discipline
    /// [`Run::injected_guidance`] establishes: a pre-existing manifest
    /// with no `parent_run_id` key deserializes to `None`, AND a root
    /// run reserializes WITHOUT the key at all — so adding execution
    /// lineage never perturbs the on-disk shape of the (still
    /// overwhelmingly common) single-agent, no-parent case.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_run_id: Option<RunId>,
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
    /// Constructs a ROOT run: `parent_run_id` starts `None` (and
    /// `injected_guidance` empty), mirroring the same
    /// "additive field no early caller needs to set" precedent
    /// [`EvidenceRecord::new`] cites. Use [`Run::with_parent_run_id`]
    /// to make it a dispatched child.
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
        Self { envelope, run_id, parent_run_id: None, session_id, task_id, status, started_at, ended_at, injected_guidance: Vec::new() }
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

    /// Records the run that DISPATCHED this one — see
    /// [`Run::parent_run_id`]. Meant to be called exactly ONCE, at the
    /// moment the dispatch edge is known (dispatch time for a live run,
    /// normalization time for an ingested transcript), for the same
    /// reason [`Run::with_injected_guidance`] is: a `Run` is only ever
    /// dispatched once, so a second call simply replaces the value
    /// rather than merging.
    pub fn with_parent_run_id(mut self, parent_run_id: RunId) -> Self {
        self.parent_run_id = Some(parent_run_id);
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

/// How much a [`Finding`] blocks — the reviewer's own judgement at the
/// moment they raised it, never a score canon derives. `Blocker` must
/// be resolved before the reviewed line of work ships; `ShouldFix` is
/// a real defect that may be scheduled; `Note` is an observation that
/// carries no obligation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    Blocker,
    ShouldFix,
    Note,
}

/// What became of a [`Finding`]. `Open` is raised-and-outstanding;
/// `Fixed` is closed BY A COMMIT (and is the only disposition that
/// carries a [`Finding::resolution_sha`]); `Rejected` is closed by the
/// reviewer withdrawing or the author declining, with no commit
/// involved; `Deferred` is acknowledged and postponed. There is
/// deliberately no `FixOfFix` variant — see [`Finding`]'s own doc for
/// why that relationship is DERIVED by join and can never be stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FindingDisposition {
    Open,
    Fixed,
    Rejected,
    Deferred,
}

impl FindingDisposition {
    /// This disposition's wire string — the value
    /// `#[serde(rename_all = "snake_case")]` above already produces,
    /// exposed so a diagnostic can name a disposition in the same
    /// spelling an operator will grep the corpus for (asserted
    /// against serde by a test, mirroring
    /// [`crate::envelope::RecordKind::as_str`]).
    pub fn as_str(self) -> &'static str {
        match self {
            FindingDisposition::Open => "open",
            FindingDisposition::Fixed => "fixed",
            FindingDisposition::Rejected => "rejected",
            FindingDisposition::Deferred => "deferred",
        }
    }
}

/// One code-review finding (s43, the reviewed 14th kind): the durable
/// home for a single issue one reviewer raised in one review round on
/// one change. Before this kind existed, canon already CLASSIFIED
/// these during ingest
/// (`canon_ingest::artifact_adapter::ArtifactEventKind::CodeReviewFinding`,
/// a transient normalization enum) and then discarded them for want of
/// a record kind to write them into — so a release
/// narrative's issue counts could only ever be hand-typed into prose,
/// which is exactly how v0.4.0's git tag shipped two wrong numbers in
/// one sentence with nothing in canon able to contradict them.
///
/// Deliberately NOT a [`Review`] (a scenario-scoped parity attestation
/// with no finding text, round, disposition, or resolution) and NOT a
/// [`Divergence`] (which mandates a `scenario_id`, carries a single
/// `sha` with no raised-vs-resolved distinction, and has no rejected
/// disposition). A finding is scoped to the CHANGE under review, never
/// to a scenario, so it is a flat, non-area-scoped kind
/// ([`RecordKind::is_area_scoped`] is false for it) whose natural key
/// is the composite `{change_id}__{round:04}__{seq:04}`
/// (`canon_store::partition::resolve_partition`). No new join-spine
/// newtype is owed: `change_id` is [`crate::ids::ChangeId`], already
/// on the spine, so a finding joins finding → change → task through
/// keys that exist — and nothing joins TO a finding by id, since
/// [`Finding::introduced_by`] points at a SHA that [`crate::ids::Sha`]
/// already spines. `ChangeId`'s kebab grammar forbids `_`, which keeps
/// `__` an unambiguous split point in the composite.
///
/// # `reviewed_sha` is OPTIONAL, because most rounds review a worktree
/// A review round usually examines UNCOMMITTED work: the reviewer
/// reads a working tree, the author fixes what came back, and only the
/// result gets committed. That round has no sha for what it saw.
/// Round 8 of this very repo's v0.4.0 campaign was exactly that — ten
/// findings against a tree whose reviewed state was never committed
/// (its FIXES landed as `f438c610`, which is a different thing) — so a
/// required `reviewed_sha` would have silently excluded a third of the
/// backfill this kind exists to produce. `Some` means the reviewed
/// state WAS committed and this names it; `None` means it was a
/// worktree. `None` is never backfilled with "the commit nearest the
/// round", which would fabricate a provenance nobody reviewed.
/// Ordering rounds therefore uses `round`, never `reviewed_sha`.
///
/// # `resolution_sha` is the commit that CLOSED the finding
/// It is present if and only if `disposition` is
/// [`FindingDisposition::Fixed`]. `Fixed` without one, or one set
/// while `Open`, is incoherent and canon never stores it. The wire
/// form keeps `resolution_sha` a FLAT top-level field rather than
/// folding it into a `Fixed { resolution_sha }` struct variant (the
/// [`DivergenceStatus::Deferred`] idiom this crate otherwise reaches
/// for) precisely because the fix-of-fix join below reads it as a
/// top-level field, and a nested variant payload would bury the one
/// query this kind exists to serve under
/// `$.disposition.fixed.resolution_sha`.
///
/// A flat pair cannot make the incoherent state UNREPRESENTABLE, so
/// the type makes it unREACHABLE instead — on EVERY construction path
/// there is, because the claim is a biconditional and one open route
/// falsifies it:
///
/// - **Constructors.** [`Finding::fixed_by`] is the only path to
///   `Fixed` and it cannot be called without the closing sha, while
///   [`Finding::rejected`]/[`Finding::deferred`] clear it.
/// - **Mutation.** `disposition` and `resolution_sha` are the two
///   PRIVATE fields of an otherwise-`pub` record, read through
///   [`Finding::disposition`]/[`Finding::resolution_sha`]. While they
///   were `pub`, a clone-and-mutate reached either invalid state in
///   one line, so "the constructors are the only path" was a
///   convention rather than a guarantee.
/// - **Deserialization.** `Finding`'s [`Deserialize`] is hand-written,
///   not derived, and runs [`Finding::check_coherence`] before it
///   yields a value — so `serde_json::from_value` REFUSES an
///   incoherent body instead of materializing one that no constructor
///   could have produced.
///
/// The consequence worth stating plainly, because it is the property
/// the round trip rests on: every `Finding` VALUE that exists is
/// coherent, so a record canon itself wrote is always a record canon
/// can read back. A hand-authored on-disk body is the only remaining
/// way to express the incoherent pair, and [`Finding::from_body`] —
/// the read path's own entry point
/// (`canon_store::partition::validate_body`'s `Finding` arm) — reports
/// it as [`FailureClass::Malformed`] against `resolution_sha` BY NAME
/// rather than as an opaque deserialize failure, so it never quietly
/// counts as a fix nobody can point at a commit for.
///
/// # `introduced_by` is the commit that INTRODUCED the defect
/// `None` means the introducing commit could not be SOURCED — never
/// "there probably isn't one". It is never inferred from timing,
/// adjacency, `git blame` heuristics, or whichever commit happens to
/// precede the review round.
///
/// The consequence is load-bearing, and it is ONE sentence that every
/// consumer repeats verbatim rather than paraphrasing. This doc used
/// to assert a one-directional bound instead, which was wrong in the
/// over-counting direction and had already been copied into the CLI
/// help, the mart docs, the report panel, the dashboard and the skill
/// before anyone checked it against a corpus (s43 round 1 seq 1, and
/// round 2 findings 5 and 7 for the four surfaces the first correction
/// missed). The retired word is deliberately not repeated here: a
/// denial makes a reader weigh it against the word, and the word wins
/// — `canon_report::render::FIX_OF_FIX_MEANING` is the one place that
/// records the history, and this is its sentence, character for
/// character:
///
/// `fix_of_fix` bounds NOTHING — not from below, not from above: it
/// UNDER-counts, because an unsourced finding is never counted and a
/// fix in one change that breaks something first found while reviewing
/// a DIFFERENT change is not counted at all; it OVER-counts, because a
/// `resolution_sha` commit may carry work BEYOND the fix and every
/// finding recording that commit is counted regardless; and for any
/// individual match the data cannot say whether the fix or the other
/// work in that commit introduced the defect.
///
/// # Fix-of-fix is DERIVED, never stored
/// A finding is a *fix-of-fix* when its `introduced_by` equals some
/// EARLIER finding's `resolution_sha`. **EARLIER means lower
/// `(round, seq)` within the same `change_id`** — never "an earlier
/// `reviewed_sha`", because a round that reviewed a worktree has none,
/// and never `Envelope.at`, which is authoring time and can be
/// backfilled out of order. That relationship is computed by joining
/// findings to each other, and it MUST never become a stored boolean,
/// an `is_fix_of_fix` field, a severity variant, or a reviewer-set
/// label. The whole reason this kind exists is that the number stopped
/// being hand-typed; a stored flag would merely relocate the
/// hand-typing from the release notes into the record.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct Finding {
    #[serde(flatten)]
    pub envelope: Envelope,
    /// The change under review — the finding's scope, the first
    /// component of its natural key, and its join onto the existing
    /// spine (finding → change → task).
    pub change_id: ChangeId,
    /// The commit whose state this round reviewed, when that state was
    /// committed at all — see this type's doc: `None` is a round that
    /// reviewed an uncommitted working tree, which is the common case,
    /// and is never backfilled by guessing a nearby commit.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "present_value")]
    pub reviewed_sha: Option<Sha>,
    /// Which review round on this change raised it (1-based). This, not
    /// `reviewed_sha`, is what orders rounds — a round that reviewed a
    /// worktree has no sha to order by.
    pub round: u32,
    /// This finding's index WITHIN `round` (1-based) — `round` alone
    /// does not identify a finding, so the two together with
    /// `change_id` form the natural key.
    pub seq: u32,
    pub severity: FindingSeverity,
    /// PRIVATE, with [`Finding::disposition`] the reader (type doc):
    /// half of the biconditional, and a `pub` half is a one-line route
    /// around every constructor that upholds it.
    disposition: FindingDisposition,
    /// Who raised it. A plain string for the same reason
    /// [`Divergence::reviewer`] is one — a reviewer may be a human name
    /// a canon [`crate::ids::RoleId`] does not model.
    pub reviewer: String,
    /// One line of what the finding IS, in the reviewer's own words.
    pub summary: String,
    /// The commit that closed it — see this type's doc: present iff
    /// `disposition` is [`FindingDisposition::Fixed`]. PRIVATE for the
    /// same reason `disposition` is, and read through
    /// [`Finding::resolution_sha`].
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "present_value")]
    resolution_sha: Option<Sha>,
    /// The commit that introduced the defect — see this type's doc:
    /// `None` means UNSOURCED, never "no cause", and is one of the two
    /// reasons a derived fix-of-fix count UNDER-counts.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "present_value")]
    pub introduced_by: Option<Sha>,
    /// Where in the tree, as `path/to/file.rs:120-134`. A plain string:
    /// a line range is not a canon join key and inventing a newtype for
    /// it would spine nothing.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "present_value")]
    pub file_ref: Option<String>,
}

/// [`Finding`]'s deserialize-only twin: the SAME wire shape, with no
/// coherence rule attached.
///
/// It exists because `Finding`'s own [`Deserialize`] must run
/// [`Finding::check_coherence`] before yielding a value (type doc), and
/// a hand-written `Deserialize` that called itself would recurse. So
/// the derive lives here, on a type nothing outside this module can
/// name, and both entry points ([`Finding::deserialize`] and
/// [`Finding::from_body`]) funnel through [`FindingWire::into_finding`]
/// — ONE place where the wire form becomes a `Finding`, so the two can
/// never disagree about what a valid body is.
///
/// The duplicated field list is compile-guarded rather than trusted:
/// `into_finding` builds `Finding` with a struct literal, so a field
/// added to `Finding` and not to this twin fails to compile. Serde
/// ATTRIBUTE drift is caught by `finding_with_every_optional_round_trips`,
/// which round-trips a fully-populated record through this exact path.
/// `skip_serializing_if` is deliberately absent: this type is never
/// serialized, and [`Finding`]'s own `Serialize` derive still owns the
/// absent-vs-`null` wire rule.
#[derive(Deserialize)]
struct FindingWire {
    #[serde(flatten)]
    envelope: Envelope,
    change_id: ChangeId,
    #[serde(default, deserialize_with = "present_value")]
    reviewed_sha: Option<Sha>,
    round: u32,
    seq: u32,
    severity: FindingSeverity,
    disposition: FindingDisposition,
    reviewer: String,
    summary: String,
    #[serde(default, deserialize_with = "present_value")]
    resolution_sha: Option<Sha>,
    #[serde(default, deserialize_with = "present_value")]
    introduced_by: Option<Sha>,
    #[serde(default, deserialize_with = "present_value")]
    file_ref: Option<String>,
}

impl FindingWire {
    /// The one gate every wire body passes through on its way to being
    /// a [`Finding`] value.
    fn into_finding(self) -> Result<Finding, EvidenceViolation> {
        let finding = Finding {
            envelope: self.envelope,
            change_id: self.change_id,
            reviewed_sha: self.reviewed_sha,
            round: self.round,
            seq: self.seq,
            severity: self.severity,
            disposition: self.disposition,
            reviewer: self.reviewer,
            summary: self.summary,
            resolution_sha: self.resolution_sha,
            introduced_by: self.introduced_by,
            file_ref: self.file_ref,
        };
        finding.check_coherence()?;
        Ok(finding)
    }
}

/// Hand-written so the `disposition` ⇔ `resolution_sha` biconditional
/// holds for every `Finding` VALUE, not merely for every value some
/// constructor produced (type doc). A derived `Deserialize` was the
/// open route: `serde_json::from_value` built `Fixed` with no closing
/// sha, and that value then serialized to a body the read path
/// rejects — a record canon wrote and canon could not read.
///
/// The violation's own `detail` is carried into the serde error rather
/// than a restated message, so a caller that only has the `Err` still
/// reads the same sentence [`Finding::check_coherence`] produces.
/// [`Finding::from_body`] is the entry point that keeps the STRUCTURED
/// [`EvidenceViolation`] (subject `resolution_sha`, class
/// [`FailureClass::Malformed`]) instead of flattening it to text.
impl<'de> Deserialize<'de> for Finding {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        FindingWire::deserialize(deserializer)?.into_finding().map_err(|violation| serde::de::Error::custom(violation.detail))
    }
}

impl Finding {
    /// A freshly raised finding: `Open`, with no resolution, no sourced
    /// introducing commit, no reviewed sha, and no file ref. A round
    /// that reviewed a COMMITTED state adds it with
    /// [`Self::reviewing_sha`]. Every closed state is reached through
    /// [`Self::fixed_by`]/[`Self::rejected`]/[`Self::deferred`], which
    /// is what makes the `disposition`/`resolution_sha` pairing
    /// unconstructible-wrong through this API (see the type doc).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        envelope: Envelope,
        change_id: ChangeId,
        round: u32,
        seq: u32,
        severity: FindingSeverity,
        reviewer: impl Into<String>,
        summary: impl Into<String>,
    ) -> Self {
        debug_assert_eq!(envelope.kind, RecordKind::Finding);
        Self {
            envelope,
            change_id,
            reviewed_sha: None,
            round,
            seq,
            severity,
            disposition: FindingDisposition::Open,
            reviewer: reviewer.into(),
            summary: summary.into(),
            resolution_sha: None,
            introduced_by: None,
            file_ref: None,
        }
    }

    /// Name the COMMITTED state this round reviewed. Only call it when
    /// the reviewed state really was a commit — a round that read a
    /// working tree leaves `reviewed_sha` absent rather than borrowing
    /// an adjacent sha (type doc).
    pub fn reviewing_sha(mut self, reviewed_sha: Sha) -> Self {
        self.reviewed_sha = Some(reviewed_sha);
        self
    }

    /// What became of this finding. A reader, because the field is
    /// private (type doc): the biconditional cannot survive a `pub`
    /// half that a clone-and-mutate flips in one line.
    pub fn disposition(&self) -> FindingDisposition {
        self.disposition
    }

    /// The commit that closed it — `Some` if and only if
    /// [`Self::disposition`] is [`FindingDisposition::Fixed`] (type
    /// doc). Private for [`Self::disposition`]'s reason.
    pub fn resolution_sha(&self) -> Option<&Sha> {
        self.resolution_sha.as_ref()
    }

    /// Close it as `Fixed` BY `resolution_sha` — the only path to
    /// [`FindingDisposition::Fixed`], so the state can never exist
    /// without the commit that produced it.
    pub fn fixed_by(mut self, resolution_sha: Sha) -> Self {
        self.disposition = FindingDisposition::Fixed;
        self.resolution_sha = Some(resolution_sha);
        self
    }

    /// Close it as `Rejected` — no commit closed it, so any
    /// previously-set `resolution_sha` is cleared rather than left to
    /// contradict the disposition.
    pub fn rejected(mut self) -> Self {
        self.disposition = FindingDisposition::Rejected;
        self.resolution_sha = None;
        self
    }

    /// Postpone it — same clearing rule as [`Self::rejected`].
    pub fn deferred(mut self) -> Self {
        self.disposition = FindingDisposition::Deferred;
        self.resolution_sha = None;
        self
    }

    /// Record the SOURCED introducing commit. There is deliberately no
    /// "guess it" counterpart: if it cannot be sourced, the field stays
    /// `None`, which is one of the two reasons a derived fix-of-fix
    /// count UNDER-counts (type doc's canonical sentence).
    pub fn with_introduced_by(mut self, introduced_by: Sha) -> Self {
        self.introduced_by = Some(introduced_by);
        self
    }

    pub fn with_file_ref(mut self, file_ref: impl Into<String>) -> Self {
        self.file_ref = Some(file_ref.into());
        self
    }

    /// The `disposition` ⇔ `resolution_sha` biconditional, re-checked
    /// on the READ path. `Finding`'s constructors already make an
    /// incoherent pair unconstructible in Rust, but a record read back
    /// from a tier was hand-authorable, so
    /// `canon_store::partition::validate_body` calls this after the
    /// concrete `Deserialize` succeeds — an incoherent record lands as
    /// [`FailureClass::Malformed`] ("malformed evidence is no
    /// evidence") instead of quietly counting as a fix nobody can point
    /// at a commit for.
    pub fn check_coherence(&self) -> Result<(), EvidenceViolation> {
        match (self.disposition, &self.resolution_sha) {
            (FindingDisposition::Fixed, None) => Err(EvidenceViolation::new(
                FailureClass::Malformed,
                "resolution_sha",
                "disposition is `fixed` but no `resolution_sha` names the commit that closed it",
            )),
            (disposition, Some(_)) if disposition != FindingDisposition::Fixed => Err(EvidenceViolation::new(
                FailureClass::Malformed,
                "resolution_sha",
                format!(
                    "`resolution_sha` is set but disposition is `{}` — only a `fixed` finding was closed by a commit",
                    disposition.as_str()
                ),
            )),
            _ => Ok(()),
        }
    }

    /// The READ path's entry point: a raw ledger body to a validated
    /// `Finding`, with a STRUCTURED [`EvidenceViolation`] for either
    /// failure mode (`canon_store::partition::validate_body`'s
    /// `Finding` arm is one call to this).
    ///
    /// Distinct from `serde_json::from_value::<Finding>` only in what
    /// it reports, never in what it accepts — both run
    /// [`FindingWire::into_finding`], so they cannot disagree about
    /// validity. This one keeps the coherence failure's own subject
    /// (`resolution_sha`) and class instead of collapsing it into a
    /// serde error string, so the read path still names the field a
    /// hand-authored record got wrong.
    pub fn from_body(json: &serde_json::Value) -> Result<Self, EvidenceViolation> {
        serde_json::from_value::<FindingWire>(json.clone())
            .map_err(|e| EvidenceViolation::new(FailureClass::Malformed, "<candidate>", e.to_string()))?
            .into_finding()
    }
}

impl CanonRecord for Finding {
    const KIND: RecordKind = RecordKind::Finding;
    fn envelope(&self) -> &Envelope {
        &self.envelope
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::Actor;
    use crate::ids::RoleId;

    /// Every round-trip fixture below builds its envelope at the kind's
    /// OWN current generation ([`RecordKind::schema_version`]) — never a
    /// hardcoded `1`, which would silently assert a generation the
    /// writers no longer stamp once a kind is bumped
    /// (`s38-evidence-bearing-memory`).
    fn envelope(kind: RecordKind) -> Envelope {
        Envelope::current(kind, Utc::now(), Actor::new("codex-cli", RoleId::parse("implementer").unwrap()))
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
        task_with_depends_on_round_trips,
        Task::new(envelope(RecordKind::Task), TaskId::parse("s37-execution-graph-topology#6.3").unwrap(), "fixtures", TaskStatus::Open, None).with_depends_on(vec![
            TaskId::parse("s37-execution-graph-topology#6.1").unwrap(),
            TaskId::parse("s37-execution-graph-topology#6.2").unwrap()
        ])
    );

    /// A `Task` from before s37 (no `depends_on` key at all in the
    /// JSON) still deserializes — to an empty `Vec` — and reserializes
    /// byte-identically, so the whole pre-s37 corpus keeps its content
    /// digest. The same additive-field bar `scenario_refs` above (and
    /// `Run.injected_guidance` before it) is held to: an empty
    /// `depends_on` never reserializes a spurious `"depends_on": []`
    /// key.
    ///
    /// This fixture deliberately stays at `"schema": 1`: it MODELS a
    /// pre-bump record, which is exactly the generation
    /// `s38-evidence-bearing-memory` has to keep readable — a
    /// `Task` canon writes TODAY carries `2`
    /// ([`RecordKind::schema_version`]), and the two coexisting in one
    /// corpus is the whole point of the fold's schema rung. Bumping this
    /// literal would delete the backward-compatibility case it exists to
    /// prove.
    #[test]
    fn task_without_depends_on_key_deserializes_empty_and_reserializes_without_the_key() {
        let at = serde_json::to_value(Utc::now()).unwrap();
        let pre_s37_json = serde_json::json!({
            "schema": 1,
            "kind": "task",
            "at": at,
            "actor": {"agent_id": "codex-cli", "role": "implementer"},
            "task_id": "s1-state-model-join-spine#6.2",
            "title": "fixtures",
            "status": "done",
            "evidence_note": "evidence",
        });

        let task: Task = serde_json::from_value(pre_s37_json.clone()).expect("a pre-s37 Task with no depends_on key must still deserialize");
        assert!(task.depends_on.is_empty());

        let reserialized = serde_json::to_value(&task).unwrap();
        assert_eq!(reserialized, pre_s37_json, "empty depends_on must not introduce a spurious key on reserialize");
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

    round_trip_test!(
        run_with_parent_run_id_round_trips,
        Run::new(envelope(RecordKind::Run), RunId::new(), None, None, RunStatus::Succeeded, Utc::now(), Some(Utc::now())).with_parent_run_id(RunId::new())
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

    /// s37-execution-graph-topology, the additive-field bar
    /// `injected_guidance` above already sets, applied to
    /// `parent_run_id`: a pre-s37 run record (no `parent_run_id` key)
    /// deserializes to `None`, and a ROOT run reserializes WITHOUT the
    /// key — never a spurious `"parent_run_id": null`. Load-bearing
    /// beyond cosmetics: canon's write-time idempotence keys on a
    /// content digest over these bytes, so an extra key would silently
    /// re-digest every already-persisted run in the corpus.
    #[test]
    fn run_without_parent_run_id_key_deserializes_none_and_reserializes_without_the_key() {
        // Same serde-encoded timestamp discipline the
        // injected_guidance test above explains.
        let at = serde_json::to_value(Utc::now()).unwrap();
        let started_at = serde_json::to_value(Utc::now()).unwrap();
        let pre_s37_json = serde_json::json!({
            "schema": 1,
            "kind": "run",
            "at": at,
            "actor": {"agent_id": "codex-cli", "role": "implementer"},
            "run_id": RunId::new().to_string(),
            "status": "succeeded",
            "started_at": started_at,
        });

        let run: Run = serde_json::from_value(pre_s37_json.clone()).expect("a pre-s37 run record with no parent_run_id key must still deserialize");
        assert_eq!(run.parent_run_id, None);

        let reserialized = serde_json::to_value(&run).unwrap();
        assert_eq!(reserialized, pre_s37_json, "a root run must not introduce a spurious parent_run_id key on reserialize");
    }

    /// s37-execution-graph-topology: `with_parent_run_id` sets the
    /// dispatch edge without disturbing anything else, and — like
    /// `with_injected_guidance` — replaces rather than merges on a
    /// second call, since a run is only ever dispatched once.
    #[test]
    fn with_parent_run_id_replaces_rather_than_accumulating() {
        let first_parent = RunId::new();
        let second_parent = RunId::new();
        let run = Run::new(envelope(RecordKind::Run), RunId::new(), None, None, RunStatus::Succeeded, Utc::now(), None)
            .with_parent_run_id(first_parent)
            .with_parent_run_id(second_parent);

        assert_eq!(run.parent_run_id, Some(second_parent));
        assert!(run.injected_guidance.is_empty(), "setting a parent must not perturb any other field");
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

    fn sha(byte: char) -> Sha {
        Sha::parse(std::iter::repeat_n(byte, 40).collect::<String>()).unwrap()
    }

    fn change_id() -> ChangeId {
        ChangeId::parse("s42-close-the-open-loops").unwrap()
    }

    /// The COMMON case (s43): a round that reviewed an uncommitted
    /// working tree, so it carries no `reviewed_sha` at all.
    fn finding(round: u32, seq: u32) -> Finding {
        Finding::new(envelope(RecordKind::Finding), change_id(), round, seq, FindingSeverity::Blocker, "review-voice", "teardown races HMR")
    }

    round_trip_test!(finding_reviewing_a_worktree_round_trips, finding(8, 10));

    round_trip_test!(
        finding_with_every_optional_round_trips,
        finding(11, 4)
            .reviewing_sha(sha('a'))
            .fixed_by(sha('b'))
            .with_introduced_by(sha('c'))
            .with_file_ref("crates/canon-model/src/records.rs:120-134")
    );

    /// `reviewed_sha` is provenance, not identity: a round that read a
    /// worktree simply omits it, and it is never backfilled with a
    /// nearby commit. The v0.4.0 campaign's round 8 was exactly this
    /// shape, and a required `reviewed_sha` would have excluded it.
    #[test]
    fn a_worktree_round_omits_reviewed_sha_and_a_committed_one_names_it() {
        let worktree = serde_json::to_value(finding(8, 10)).unwrap();
        assert!(worktree.get("reviewed_sha").is_none(), "a worktree round must not invent a reviewed_sha");
        assert_eq!(worktree.get("change_id").and_then(|v| v.as_str()), Some("s42-close-the-open-loops"));

        let committed = serde_json::to_value(finding(9, 1).reviewing_sha(sha('a'))).unwrap();
        assert_eq!(committed.get("reviewed_sha").and_then(|v| v.as_str()), Some(sha('a').as_str()));
    }

    /// The additive-field discipline every canon optional follows: an
    /// unset optional is ABSENT from the wire form, never a literal
    /// `null` (which would change the record's `content_digest12` and
    /// break write-time idempotence).
    #[test]
    fn finding_omits_its_unset_optionals_entirely() {
        let json = serde_json::to_value(finding(1, 1)).unwrap();
        for field in ["reviewed_sha", "resolution_sha", "introduced_by", "file_ref"] {
            assert!(json.get(field).is_none(), "an unset `{field}` must be absent from the wire form, not null");
        }
    }

    #[test]
    fn finding_with_a_present_null_optional_fails_to_deserialize() {
        let mut json = serde_json::to_value(finding(1, 1)).unwrap();
        for field in ["reviewed_sha", "resolution_sha", "introduced_by", "file_ref"] {
            json[field] = serde_json::Value::Null;
            assert!(
                serde_json::from_value::<Finding>(json.clone()).is_err(),
                "a present `{field}: null` must fail deserialize, never collapse to the absent default"
            );
            json.as_object_mut().unwrap().remove(field);
        }
    }

    /// `fixed_by` is the ONLY path to `Fixed`, and the two closed-
    /// without-a-commit dispositions clear any resolution rather than
    /// leaving one to contradict them — so the incoherent pair the read
    /// path guards against is unconstructible through this API.
    #[test]
    fn only_fixed_by_produces_a_fixed_finding_and_it_always_names_its_commit() {
        let raised = finding(1, 1);
        assert_eq!(raised.disposition(), FindingDisposition::Open);
        assert_eq!(raised.resolution_sha(), None);
        raised.check_coherence().unwrap();

        let fixed = finding(1, 1).fixed_by(sha('b'));
        assert_eq!(fixed.disposition(), FindingDisposition::Fixed);
        assert_eq!(fixed.resolution_sha(), Some(&sha('b')));
        fixed.check_coherence().unwrap();

        for closed in [finding(1, 1).fixed_by(sha('b')).rejected(), finding(1, 1).fixed_by(sha('b')).deferred()] {
            assert_eq!(closed.resolution_sha(), None, "a disposition that no commit closed must not keep a resolution_sha");
            closed.check_coherence().unwrap();
        }
    }

    #[test]
    fn check_coherence_rejects_both_incoherent_directions() {
        let mut fixed_without_commit = finding(1, 1).fixed_by(sha('b'));
        fixed_without_commit.resolution_sha = None;
        let err = fixed_without_commit.check_coherence().unwrap_err();
        assert_eq!(err.class, FailureClass::Malformed);
        assert!(err.detail.contains("no `resolution_sha`"), "{}", err.detail);

        let mut open_with_commit = finding(1, 1);
        open_with_commit.resolution_sha = Some(sha('b'));
        let err = open_with_commit.check_coherence().unwrap_err();
        assert_eq!(err.class, FailureClass::Malformed);
        assert!(err.detail.contains("disposition is `open`"), "{}", err.detail);
    }

    /// The route the DERIVED `Deserialize` left open, and the reason
    /// the type doc's biconditional was a claim stronger than the code:
    /// `serde_json::from_value` happily built `Fixed` with no closing
    /// sha, so `fixed_by` was not in fact the only path to `Fixed`, and
    /// the resulting value re-serialized to a body the read path
    /// rejects. Both directions are now refused BY DESERIALIZE, before
    /// any value exists to be written.
    #[test]
    fn deserialize_refuses_an_incoherent_body_instead_of_materializing_one() {
        let mut fixed_without_commit = serde_json::to_value(finding(1, 1)).unwrap();
        fixed_without_commit["disposition"] = serde_json::json!("fixed");
        let err = serde_json::from_value::<Finding>(fixed_without_commit.clone()).unwrap_err();
        assert!(err.to_string().contains("no `resolution_sha`"), "{err}");

        let mut open_with_commit = serde_json::to_value(finding(1, 1)).unwrap();
        open_with_commit["resolution_sha"] = serde_json::to_value(sha('b')).unwrap();
        let err = serde_json::from_value::<Finding>(open_with_commit.clone()).unwrap_err();
        assert!(err.to_string().contains("disposition is `open`"), "{err}");

        // `from_body` accepts EXACTLY the same bodies `from_value` does
        // (one `into_finding`, two entry points) and keeps the
        // STRUCTURED violation the read path reports — the diagnostic a
        // flattened serde error string would have lost.
        for incoherent in [fixed_without_commit, open_with_commit] {
            let violation = Finding::from_body(&incoherent).unwrap_err();
            assert_eq!(violation.class, FailureClass::Malformed);
            assert_eq!(violation.subject, "resolution_sha", "the read path must still name the field, not merely fail");
        }
    }

    /// The round-trip contract the sealed biconditional exists to make
    /// TRUE: every `Finding` a constructor can produce serializes to a
    /// body the read path accepts and returns unchanged. A record canon
    /// itself wrote is never a record canon rejects.
    #[test]
    fn every_constructible_finding_round_trips_through_the_read_path() {
        for constructed in [
            finding(1, 1),
            finding(1, 2).fixed_by(sha('b')),
            finding(1, 3).rejected(),
            finding(1, 4).deferred(),
            finding(1, 5).fixed_by(sha('b')).rejected(),
            finding(1, 6).fixed_by(sha('b')).deferred(),
            finding(1, 7).reviewing_sha(sha('a')).fixed_by(sha('b')).with_introduced_by(sha('c')).with_file_ref("a.rs:1-2"),
        ] {
            let body = serde_json::to_value(&constructed).expect("a Finding always serializes");
            let read_back = Finding::from_body(&body)
                .unwrap_or_else(|v| panic!("canon wrote a body canon cannot read: {} — {}", v.subject, v.detail));
            assert_eq!(read_back, constructed, "the read path must return the record that was written");
        }
    }

    /// Fix-of-fix is DERIVED by joining findings to each other, never
    /// stored. This test IS the derivation, and it deliberately orders
    /// by `(round, seq)` rather than by `reviewed_sha`: the round that
    /// raised the fix-of-fix reviewed an uncommitted worktree and has
    /// no sha to order by, which is precisely the case a sha-ordered
    /// derivation would drop.
    #[test]
    fn fix_of_fix_is_derived_by_round_order_and_the_count_bounds_nothing() {
        let closed_round_one = finding(1, 1).reviewing_sha(sha('a')).fixed_by(sha('b'));
        let caused_by_that_fix = finding(2, 1).with_introduced_by(sha('b'));
        let unsourced = finding(2, 2);

        let mut corpus = vec![&caused_by_that_fix, &unsourced, &closed_round_one];
        corpus.sort_by_key(|f| (f.round, f.seq));
        assert!(corpus.iter().all(|f| f.change_id == change_id()), "the join is scoped to one change");

        let mut resolved_so_far: std::collections::HashSet<&Sha> = std::collections::HashSet::new();
        let mut fix_of_fix: Vec<(u32, u32)> = Vec::new();
        for f in &corpus {
            if f.introduced_by.as_ref().is_some_and(|sha| resolved_so_far.contains(sha)) {
                fix_of_fix.push((f.round, f.seq));
            }
            if let Some(resolution) = f.resolution_sha.as_ref() {
                resolved_so_far.insert(resolution);
            }
        }

        assert_eq!(fix_of_fix, vec![(2, 1)], "only the finding whose introduced_by names an EARLIER round's resolution_sha joins");
        assert_eq!(caused_by_that_fix.reviewed_sha, None, "the fix-of-fix round reviewed a worktree — sha ordering would have lost it");
        assert_eq!(unsourced.introduced_by, None, "an unsourced finding stays None — never inferred from being in a later round");

        // No stored flag anywhere on the wire form: the relationship
        // exists only in the join above.
        let json = serde_json::to_value(&caused_by_that_fix).unwrap();
        let keys: Vec<&String> = json.as_object().unwrap().keys().collect();
        assert!(!keys.iter().any(|k| k.contains("fix_of_fix")), "fix-of-fix must never become a stored field: {keys:?}");
    }

    /// s43 round 2, finding 5. The round-1 correction reached the
    /// report panel and stopped there; this type's own doc — the
    /// upstream every other surface is written FROM, and the text
    /// `schemas/finding.schema.json` embeds verbatim — still called an
    /// `introduced_by`-derived count "a FLOOR, not a total". So the
    /// guard that was only ever run against the panel now runs here
    /// too, over the `Finding` type doc and the two field/builder docs
    /// that restate it.
    ///
    /// Scoped to those doc blocks rather than the whole file on
    /// purpose: "dropped them on the floor" a few lines above is an
    /// idiom, and a guard that cannot tell the two apart gets widened
    /// until it catches nothing.
    #[test]
    fn the_finding_docs_claim_no_bound_and_state_the_canonical_sentence() {
        let source = include_str!("records.rs");
        // Walks BACK from the item to the top of its unbroken `///`
        // run (tolerating the `#[derive]`/`#[serde]` attributes that sit
        // between doc and item), then restores source order so the
        // canonical sentence is contiguous rather than reversed.
        let doc_block = |anchor: &str| -> String {
            let end = source.find(anchor).unwrap_or_else(|| panic!("{anchor} is no longer in records.rs"));
            // Back up to the start of the anchor's own line: an indented
            // anchor would otherwise leave its leading whitespace as a
            // final "line" and stop the walk before it began.
            let end = source[..end].rfind('\n').map_or(0, |newline| newline + 1);
            let mut lines: Vec<&str> = source[..end]
                .lines()
                .rev()
                .take_while(|line| line.trim_start().starts_with("///") || line.trim_start().starts_with("#["))
                .filter(|line| line.trim_start().starts_with("///"))
                .collect();
            lines.reverse();
            lines.iter().map(|line| line.trim_start().trim_start_matches("///").trim()).collect::<Vec<_>>().join(" ")
        };
        let surfaces = [
            ("the `Finding` type doc", doc_block("pub struct Finding {")),
            ("the `introduced_by` field doc", doc_block("pub introduced_by: Option<Sha>,")),
            ("`with_introduced_by`'s doc", doc_block("pub fn with_introduced_by(")),
        ];

        // `crates/canon-report/src/render.rs`'s own DIRECTIONAL_PHRASES,
        // which is the list the panel has been held to since round 1.
        // Space-prefixed for the word boundary, exactly as there.
        const DIRECTIONAL_PHRASES: &[&str] = &[
            "floor",
            "lower bound",
            "lower-bound",
            "bounds from below",
            "minimum",
            "at least",
            "no fewer than",
            "conservative",
            "understates",
            "underestimate",
            "under-estimate",
            "upper bound",
            "ceiling",
            "at most",
            "no more than",
            "overstates",
        ];
        for (where_, doc) in &surfaces {
            assert!(!doc.is_empty(), "{where_} scanned as empty — the anchor moved and this guard stopped guarding");
            let haystack = format!(" {}", doc.to_ascii_lowercase());
            for phrase in DIRECTIONAL_PHRASES {
                assert!(!haystack.contains(&format!(" {phrase}")), "{where_} bounds a derived count via {phrase:?}: {doc}");
            }
        }

        // And the type doc states the whole sentence, not a clause of
        // it. Whitespace-collapsed because `///` wraps it across lines;
        // `packages/dashboard/test/panel-copy.test.ts` pins this same
        // text against `canon_report::render::FIX_OF_FIX_MEANING` and
        // every other surface (canon-model cannot depend on
        // canon-report — the dependency runs the other way).
        const CANONICAL: &str = "`fix_of_fix` bounds NOTHING — not from below, not from above: it UNDER-counts, because an unsourced finding is never counted and a fix in one change that breaks something first found while reviewing a DIFFERENT change is not counted at all; it OVER-counts, because a `resolution_sha` commit may carry work BEYOND the fix and every finding recording that commit is counted regardless; and for any individual match the data cannot say whether the fix or the other work in that commit introduced the defect.";
        assert!(surfaces[0].1.contains(CANONICAL), "the `Finding` type doc must state the canonical sentence verbatim: {}", surfaces[0].1);
    }

    #[test]
    fn finding_disposition_as_str_matches_its_serde_encoding() {
        for disposition in
            [FindingDisposition::Open, FindingDisposition::Fixed, FindingDisposition::Rejected, FindingDisposition::Deferred]
        {
            let json = serde_json::to_string(&disposition).unwrap();
            assert_eq!(json, format!("\"{}\"", disposition.as_str()), "{disposition:?} as_str() disagrees with its own serde encoding");
        }
    }
}
