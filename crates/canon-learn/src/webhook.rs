//! The PR/CI webhook receiver (S7 design D5, task group 5) — closes all
//! three deferred seams the design doc's Risk/Scope section names as a
//! single, independently-tracked gap (not just the happy-path receiver):
//!
//! - (a) **SHA→trajectoryId join** (task 5.4): the donor's own webhook
//!   translator never built this join — it borrowed the commit SHA
//!   itself as a trajectory-id slot, which never actually matched
//!   anything. This module resolves a webhook payload's SHA through
//!   [`canon_model::ids::Sha`] (S1's typed join-spine key, whose own
//!   `JOINS` doc reads "reward signals ↔ trajectory") against a
//!   `sha:<40-hex>` tag on [`crate::trajectory::Trajectory::tags`] (this
//!   crate's own frozen, free-form side channel — [`crate::promotion`]
//!   documents the same pattern for CRN panel/config identity) — never
//!   a bare-string comparison, so a wrongly-shaped join is impossible by
//!   construction, not just by convention.
//! - (b) **The no-rollback timer** (task 5.5): no production
//!   implementation of this exists anywhere in the donor harness —
//!   the donor's own dev-reward backfill doc defers it as "the
//!   webhook-receiver wiring... lands later" and nothing ever built it.
//!   [`check_no_rollback`] is the first real implementation, modeled as
//!   a PURE function of an explicit `as_of` timestamp + event log +
//!   window (never `Utc::now()` internally) so a "scheduled/deferred
//!   check" is deterministically testable offline, exactly like
//!   the donor's MaTTS pure-statistics-core split this crate already follows
//!   elsewhere (`promotion.rs`'s own module doc).
//! - The receiver itself (tasks 5.1-5.3): normalizes GitHub
//!   `pull_request.merged`/`workflow_run.conclusion` payloads into S4's
//!   `{role, polarity, becomes}` [`VerdictRow`] shape (design D5: "built
//!   on S4, not a bespoke ingester" — this module never duplicates S4's
//!   ingest adapter registry, `ArtifactEventKind`, or `derive_verdict`;
//!   it only reproduces the two table rows S4's own `derive_verdict`
//!   already assigns a fixed `dev` role to — `PrMergeNoRevert`/
//!   `CiFailOrPrRevert`), then calls [`crate::reward::RewardRegistry::
//!   compute`] + [`crate::mark_verdict::mark_trajectory_verdict`]. Gated
//!   behind `canon.yaml`'s `webhook.enabled` (task 5.3) — local-only
//!   mode (§9) works with zero network, since [`WebhookConfig::default`]
//!   is disabled and every entry point below checks it FIRST.
//!
//! **Migration Step 1 posture (design doc)**: no HTTP server ships here.
//! This module is a pure payload→verdict normalizer plus the reward
//! wiring — every entry point takes an already-parsed payload/event log
//! and an already-resolved trajectory candidate slice (mirrors
//! [`crate::promotion::PromotionGate::evaluate`]'s own "neither gate
//! reads a store directly" discipline). Wiring a real HTTP endpoint that
//! deserializes an inbound GitHub delivery and gathers candidates from a
//! live store is `canon-cli`/a future dashboard's job, out of this
//! change's scope (Migration Step 2) — this module owns only the parse
//! + join + timer + reward-write, never the transport.

use canon_ingest::verdict::{Becomes, Polarity, VerdictRow};
use canon_model::ids::{JoinKeyError, PrNumber, RoleId, Sha};
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;

use crate::error::LearnError;
use crate::ids::TrajectoryId;
use crate::mark_verdict::mark_trajectory_verdict;
use crate::reward::RewardRegistry;
use crate::store::TrajectoryStore;
use crate::trajectory::Trajectory;
use crate::verdict_outcome::TrajectoryVerdict;

/// The `Trajectory::tags` convention this module reads/writes for the
/// SHA→trajectory join (task 5.4). A trajectory-recording caller that
/// wants to be webhook-joinable tags itself `sha:<40-hex-commit-sha>` at
/// write time (this module only ever READS the tag — nothing in this
/// crate's insulated surface lets it mutate `Trajectory::tags` after
/// `append`, design decision 3's raw-tier immutability).
const SHA_TAG_PREFIX: &str = "sha:";

/// `no_rollback.window`'s default when `canon.yaml` carries no explicit
/// `no_rollback.window_hours` — provisional, same "conservative default,
/// tunable per policy diff, never a code change" discipline design D7
/// documents for `OccurrencePromotionGate`'s own defaults.
pub const DEFAULT_NO_ROLLBACK_WINDOW_HOURS: i64 = 24;

/// Git's own revert-commit-message convention
/// (`git revert`'s default body: `"This reverts commit <sha>."`) — the
/// only signal GitHub's webhook stream carries for "a merge is a
/// revert": there is no dedicated GitHub webhook event for it, a revert
/// is just another `pull_request.merged` delivery.
const REVERT_MARKER: &str = "This reverts commit ";

/// This module's own error type — kept local to `webhook.rs` rather than
/// widening [`LearnError`] (this crate's shared error surface; the
/// wave-2 coordination protocol scopes cross-agent shared touch-points
/// to `lib.rs` + the `promotion` module only, so a webhook-only error
/// variant has no reason to live anywhere but here).
#[derive(Debug, thiserror::Error)]
pub enum WebhookError {
    /// A `pull_request` payload had `merged: true` but no `merged_at`
    /// timestamp — malformed evidence (GitHub always populates this on
    /// an actually-merged PR); never defaulted to a wall-clock read,
    /// which would break this module's determinism guarantee.
    #[error("webhook payload: pull_request.merged is true but merged_at is missing")]
    MissingMergedAt,
    /// A `workflow_run` payload had `action: "completed"` but no
    /// `updated_at` timestamp.
    #[error("webhook payload: workflow_run.action is \"completed\" but updated_at is missing")]
    MissingUpdatedAt,
    /// A join-spine key (`Sha`/`PrNumber`) failed its grammar check —
    /// passed through verbatim from `canon-model`.
    #[error(transparent)]
    JoinKey(#[from] JoinKeyError),
    /// `canon.yaml`'s `webhook:`/`no_rollback:` sections failed to
    /// parse, or `no_rollback.window_hours` was non-positive.
    #[error("canon.yaml `webhook`/`no_rollback` section: {0}")]
    Config(String),
    /// Propagated from [`mark_trajectory_verdict`]/[`TrajectoryStore`].
    #[error(transparent)]
    Learn(#[from] LearnError),
}

// ── Raw GitHub webhook payloads (task 5.1) ──────────────────────────────
//
// Deliberately narrow: only the fields this module actually reads.
// Every other field GitHub sends is silently ignored by `serde`'s
// default "unknown fields are fine" behavior (no `deny_unknown_fields`),
// matching `LearnConfig`'s own "parse only the key(s) you own" doc.

/// `pull_request` webhook event — the top-level `action` plus the
/// nested `pull_request` object.
#[derive(Debug, Clone, Deserialize)]
pub struct PullRequestMergedPayload {
    pub action: String,
    pub pull_request: PullRequestPayload,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PullRequestPayload {
    pub number: u32,
    pub merged: bool,
    pub merge_commit_sha: String,
    #[serde(default)]
    pub merged_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub body: Option<String>,
}

/// `workflow_run` webhook event — the top-level `action` plus the
/// nested `workflow_run` object.
#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowRunPayload {
    pub action: String,
    pub workflow_run: WorkflowRunInner,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkflowRunInner {
    pub head_sha: String,
    #[serde(default)]
    pub conclusion: Option<String>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

// ── Normalized events ────────────────────────────────────────────────────

/// One entry in the no-rollback timer's event log (task 5.5) — the
/// deterministic substitute for "watch the webhook stream live".
/// `subject_sha`/`at` let [`check_no_rollback`] scan a log without
/// matching on the variant twice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebhookLogEvent {
    /// A `pull_request.merged` delivery that is NOT itself a revert.
    Merged { sha: Sha, pr: Option<PrNumber>, at: DateTime<Utc> },
    /// A `pull_request.merged` delivery whose body matched git's own
    /// `"This reverts commit <sha>."` convention — `reverted_sha` is the
    /// ORIGINAL commit being undone, not this revert commit's own sha.
    Reverted { reverted_sha: Sha, at: DateTime<Utc> },
}

impl WebhookLogEvent {
    /// The commit sha this event is evidence ABOUT — the original merge
    /// sha for [`WebhookLogEvent::Merged`], the sha being undone for
    /// [`WebhookLogEvent::Reverted`].
    pub fn subject_sha(&self) -> &Sha {
        match self {
            Self::Merged { sha, .. } => sha,
            Self::Reverted { reverted_sha, .. } => reverted_sha,
        }
    }

    pub fn at(&self) -> DateTime<Utc> {
        match self {
            Self::Merged { at, .. } | Self::Reverted { at, .. } => *at,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CiOutcome {
    Success,
    Failure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedCiEvent {
    pub sha: Sha,
    pub outcome: CiOutcome,
    pub at: DateTime<Utc>,
}

/// Scans a merged PR's body for git's revert-commit-message convention,
/// returning the SHA being reverted. Only the FIRST match is honored —
/// a body can name at most one revert target per GitHub/git convention.
fn detect_revert_target(body: Option<&str>) -> Option<Sha> {
    let body = body?;
    let start = body.find(REVERT_MARKER)? + REVERT_MARKER.len();
    let candidate: String = body[start..].chars().take_while(|c| c.is_ascii_hexdigit()).collect();
    Sha::parse(candidate).ok()
}

/// Normalizes a `pull_request` webhook payload (task 5.1). Returns
/// `Ok(None)` for any delivery that carries no verdict-relevant event —
/// mirrors S4's own `ArtifactEventKind::NonVerdict` (`derive_verdict`
/// returns `None`, never a synthesized guess): a PR closed WITHOUT
/// merging (`action == "closed"`, `merged == false`) or any other
/// `action` (`opened`/`synchronize`/…) is not evidence of anything.
///
/// A revert-shaped merge (body matches git's revert-commit-message
/// convention, see `detect_revert_target`) is
/// normalized to [`WebhookLogEvent::Reverted`], never
/// [`WebhookLogEvent::Merged`] — the revert PR's OWN merge sha is not
/// what this event is evidence about; the ORIGINAL sha it undoes is.
pub fn normalize_pull_request_merged(payload: &PullRequestMergedPayload) -> Result<Option<WebhookLogEvent>, WebhookError> {
    if payload.action != "closed" || !payload.pull_request.merged {
        return Ok(None);
    }
    let at = payload.pull_request.merged_at.ok_or(WebhookError::MissingMergedAt)?;
    if let Some(reverted_sha) = detect_revert_target(payload.pull_request.body.as_deref()) {
        return Ok(Some(WebhookLogEvent::Reverted { reverted_sha, at }));
    }
    let sha = Sha::parse(payload.pull_request.merge_commit_sha.clone())?;
    // A PR number of 0 cannot occur on GitHub; treat a bogus one as
    // "not carried" rather than fail the whole normalization over
    // metadata the join doesn't actually need (`sha` is the join key).
    let pr = PrNumber::parse(payload.pull_request.number).ok();
    Ok(Some(WebhookLogEvent::Merged { sha, pr, at }))
}

/// Normalizes a `workflow_run` webhook payload (task 5.1). Returns
/// `Ok(None)` for any delivery this module doesn't map: a non-`completed`
/// action (`requested`/`in_progress`), or a `completed` run whose
/// `conclusion` is neither `"success"` nor `"failure"`
/// (`cancelled`/`skipped`/`neutral`/`timed_out`/`action_required`/…) —
/// none of those are a dev-reward signal S4's own table carries a row
/// for, matching `derive_verdict`'s "unmapped kind returns `None`,
/// never a guess" discipline.
pub fn normalize_workflow_run(payload: &WorkflowRunPayload) -> Result<Option<NormalizedCiEvent>, WebhookError> {
    if payload.action != "completed" {
        return Ok(None);
    }
    let outcome = match payload.workflow_run.conclusion.as_deref() {
        Some("success") => CiOutcome::Success,
        Some("failure") => CiOutcome::Failure,
        _ => return Ok(None),
    };
    let at = payload.workflow_run.updated_at.ok_or(WebhookError::MissingUpdatedAt)?;
    let sha = Sha::parse(payload.workflow_run.head_sha.clone())?;
    Ok(Some(NormalizedCiEvent { sha, outcome, at }))
}

// ── SHA → trajectoryId join (task 5.4) ──────────────────────────────────

/// The canonical `Trajectory::tags` entry for `sha` — the write-side
/// half of the join convention this module's read side
/// ([`resolve_trajectory_by_sha`]) consumes. Exposed so a trajectory-
/// recording caller (out of this module's own scope — this crate never
/// writes a `Trajectory` from a webhook event) can tag itself
/// join-ably at `append` time.
pub fn sha_tag(sha: &Sha) -> String {
    format!("{SHA_TAG_PREFIX}{sha}")
}

/// Recovers the `Sha` a trajectory was tagged with, if any — parses the
/// tag payload through [`Sha::parse`] rather than trusting the raw
/// string, so a malformed or truncated tag is silently treated as "no
/// tag" rather than a false join.
fn trajectory_sha(trajectory: &Trajectory) -> Option<Sha> {
    trajectory.tags.iter().find_map(|tag| tag.strip_prefix(SHA_TAG_PREFIX).and_then(|hex| Sha::parse(hex).ok()))
}

/// Resolves a commit SHA to the [`TrajectoryId`] that produced it (task
/// 5.4) — the seam the donor's own webhook translator never built (it
/// borrowed the SHA itself as a trajectory-id slot, which never
/// actually matched). `candidates` is a caller-resolved pool (typically
/// a `TrajectoryStore::query_by_regime_key` result for the `dev` regime
/// this GitHub repo/area writes under — this function does no I/O
/// itself, mirroring [`crate::promotion::PromotionGate::evaluate`]'s own
/// pure-function-over-resolved-samples discipline).
///
/// Returns `None`, EXPLICITLY, when no candidate carries a matching
/// [`sha_tag`] — a caller MUST handle this case (every entry point
/// below surfaces it as [`WebhookOutcome::UnjoinedSha`]), never silently
/// mis-joins onto an unrelated trajectory or falls back to treating the
/// SHA as if it were itself a valid [`TrajectoryId`].
pub fn resolve_trajectory_by_sha(sha: &Sha, candidates: &[Trajectory]) -> Option<TrajectoryId> {
    candidates.iter().find(|t| trajectory_sha(t).as_ref() == Some(sha)).map(|t| t.id)
}

// ── No-rollback timer (task 5.5) ────────────────────────────────────────

/// The no-rollback timer's outcome (task 5.5) at a given `as_of`
/// instant — never a bare `bool`, so a caller cannot conflate "still
/// waiting" with "confirmed no rollback" (a `bool` would force one of
/// them to be the default, and that default would be wrong for the
/// other case: a fresh merge must never read as prematurely satisfied,
/// nor must a genuinely-clean merge read as reverted).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoRollbackStatus {
    /// The window has not yet elapsed and no revert has landed for this
    /// SHA — the caller (a scheduled/deferred re-check, never a
    /// blocking wait) must re-invoke [`check_no_rollback`] later with a
    /// later `as_of`. No verdict is marked at this status.
    Pending,
    /// The window fully elapsed with zero revert events for this SHA —
    /// the no-rollback reward factor is satisfied (task 1.1's weighted
    /// composite `no_rollback` term).
    Satisfied,
    /// A revert event for this SHA landed inside the window — mirrors
    /// `compute_dev_reward`'s own `rollback` override ("overrides
    /// everything"): permanently disqualified, a caller should NOT keep
    /// re-checking this SHA. A revert landing AFTER the window is a
    /// demotion concern (S7 task group 4, [`crate::promotion::
    /// demote_strategy`]) — genuinely out of this timer's scope, since
    /// by then the covering verdict has already been written.
    Reverted,
}

/// The no-rollback timer's pure core (task 5.5) — a function of
/// `event_log` + `window` + the explicit `as_of` instant, NEVER
/// `Utc::now()` internally, so "wait `no_rollback.window` after a merge
/// with no subsequent revert" is deterministically testable offline
/// without any real waiting (a real caller passes `Utc::now()` at its
/// own call site; this function never reads a clock itself — the same
/// "pure statistics core, sampling integration layer separate" split
/// `promotion.rs`'s own module doc credits to MaTTS, applied here
/// to timer math instead of significance math).
///
/// A revert strictly BEFORE `merged_at` (a log recording history prior
/// to this merge) never counts — only `[merged_at, merged_at + window]`
/// is the window this merge's own no-rollback factor is evaluated over.
/// Every event is additionally bound by `at <= as_of`: a persisted
/// event log spans arbitrary history (a reconciliation/replay caller
/// passes the FULL log, not just the events known "at the time"), so
/// without this bound a caller asking for status as of an earlier
/// instant would observe an event from the log's future — this
/// function is a TRUE function of `as_of` only, never of what the log
/// happens to already contain.
pub fn check_no_rollback(
    sha: &Sha,
    merged_at: DateTime<Utc>,
    window: Duration,
    event_log: &[WebhookLogEvent],
    as_of: DateTime<Utc>,
) -> NoRollbackStatus {
    let window_end = merged_at + window;
    let reverted_in_window = event_log.iter().any(|event| match event {
        WebhookLogEvent::Reverted { reverted_sha, at } => {
            reverted_sha == sha && *at >= merged_at && *at <= window_end && *at <= as_of
        }
        WebhookLogEvent::Merged { .. } => false,
    });
    if reverted_in_window {
        return NoRollbackStatus::Reverted;
    }
    if as_of >= window_end { NoRollbackStatus::Satisfied } else { NoRollbackStatus::Pending }
}

// ── Config (task 5.3) ────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Deserialize)]
struct WebhookManifest {
    #[serde(default)]
    webhook: Option<WebhookSectionRaw>,
    #[serde(default)]
    no_rollback: Option<NoRollbackSectionRaw>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct WebhookSectionRaw {
    #[serde(default)]
    enabled: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct NoRollbackSectionRaw {
    #[serde(default)]
    window_hours: Option<i64>,
}

/// A parsed, validated `canon.yaml` `webhook:`/`no_rollback:` section
/// (task 5.3) — mirrors [`crate::config::LearnConfig`]'s exact "parse
/// only the key(s) this module owns, `#[serde(default)]` everywhere,
/// never `deny_unknown_fields`" discipline. Resolving the repo root and
/// reading the real file is `canon-cli`'s job (same Non-Goal
/// [`crate::config::LearnConfig`]'s own module doc states) — this
/// module owns only the parse + the typed, validated result.
#[derive(Debug, Clone, PartialEq)]
pub struct WebhookConfig {
    /// `webhook.enabled` — `false` by default (design Migration Step 2:
    /// "a consumer repo without a public endpoint... is unaffected").
    /// Every pipeline entry point below checks this FIRST and returns
    /// [`WebhookOutcome::Disabled`] without touching a store or
    /// candidate slice at all when it is `false` — local-only mode
    /// works with zero network, by construction, not by convention.
    pub enabled: bool,
    /// `no_rollback.window_hours` — how long after a merge with no
    /// revert the no-rollback factor is satisfied (task 5.5).
    pub no_rollback_window: Duration,
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self { enabled: false, no_rollback_window: Duration::hours(DEFAULT_NO_ROLLBACK_WINDOW_HOURS) }
    }
}

impl WebhookConfig {
    /// Parse `canon.yaml`'s content, narrowed to the `webhook:`/
    /// `no_rollback:` keys this module owns. A missing section (or an
    /// empty `canon.yaml`) is not an error — it resolves to
    /// [`WebhookConfig::default`] (disabled, default window). A
    /// non-positive `window_hours` fails loud rather than silently
    /// producing a timer that is always-already-elapsed or never-elapsing.
    pub fn from_manifest(canon_yaml: &str) -> Result<Self, WebhookError> {
        let manifest: WebhookManifest = serde_yaml::from_str(canon_yaml).map_err(|e| WebhookError::Config(e.to_string()))?;
        let enabled = manifest.webhook.map(|section| section.enabled).unwrap_or(false);
        let window_hours =
            manifest.no_rollback.and_then(|section| section.window_hours).unwrap_or(DEFAULT_NO_ROLLBACK_WINDOW_HOURS);
        if window_hours <= 0 {
            return Err(WebhookError::Config(format!("no_rollback.window_hours must be positive, got {window_hours}")));
        }
        Ok(Self { enabled, no_rollback_window: Duration::hours(window_hours) })
    }
}

// ── Pipeline: receiver → join → timer → reward → mark_trajectory_verdict ──

/// What a pipeline entry point observed happened — the shape a future
/// `canon-cli`/dashboard driver (or this module's own tests) inspects.
#[derive(Debug, Clone, PartialEq)]
pub enum WebhookOutcome {
    /// `webhook.enabled` is `false` (task 5.3) — a deliberate, silent
    /// no-op. No store or candidate slice was touched.
    Disabled,
    /// The payload/event carried no verdict-relevant signal this module
    /// maps (see [`normalize_pull_request_merged`]/
    /// [`normalize_workflow_run`]'s own docs for exactly which cases).
    NonVerdict,
    /// The event's commit SHA matched no candidate trajectory (task
    /// 5.4) — explicit, never a silent mis-join.
    UnjoinedSha(Sha),
    /// A `pull_request.merged`/revert event normalized and joined
    /// successfully, and was appended to the caller's event log — no
    /// verdict marked yet (the no-rollback timer, task 5.5, decides
    /// that later via [`evaluate_no_rollback_timer`]).
    Recorded { trajectory_id: TrajectoryId, log_event: WebhookLogEvent },
    /// [`check_no_rollback`] returned [`NoRollbackStatus::Pending`] — no
    /// verdict marked; the caller should re-invoke
    /// [`evaluate_no_rollback_timer`] later.
    AwaitingNoRollbackWindow { trajectory_id: TrajectoryId, sha: Sha },
    /// A covering verdict was computed via [`RewardRegistry::compute`]
    /// and persisted via [`mark_trajectory_verdict`].
    Marked { trajectory_id: TrajectoryId, verdict: TrajectoryVerdict },
}

/// GitHub PR-merge/CI-fail evidence is `dev`-role by construction — S4's
/// own `derive_verdict` table hardcodes `role("dev")` for BOTH
/// `PrMergeNoRevert` and `CiFailOrPrRevert` (there is no table row
/// mapping either kind to any other role), so this module mirrors that
/// fixed assignment rather than exposing a `role` parameter a caller
/// could point at a role these two GitHub-native event kinds were never
/// defined for.
fn dev_role() -> RoleId {
    RoleId::parse("dev").unwrap_or_else(|e| panic!("built-in role slug \"dev\" must be a valid RoleId: {e}"))
}

fn compute_and_mark(
    rewards: &RewardRegistry,
    store: &dyn TrajectoryStore,
    trajectory_id: TrajectoryId,
    polarity: Polarity,
    becomes: Becomes,
) -> Result<WebhookOutcome, WebhookError> {
    let row = VerdictRow { role: dev_role(), polarity, becomes };
    let (outcome, reward) = rewards.compute(&dev_role(), &[row]);
    let verdict = mark_trajectory_verdict(store, &trajectory_id, outcome, reward)?;
    Ok(WebhookOutcome::Marked { trajectory_id, verdict })
}

/// Receiver entry point for `pull_request` webhook deliveries (tasks
/// 5.1/5.2/5.3/5.4). Normalizes, then joins the resulting event's
/// subject SHA to a trajectory — does NOT itself decide the no-rollback
/// factor (see [`evaluate_no_rollback_timer`]): calling
/// `mark_trajectory_verdict` with `Success` at merge time, before any
/// window has elapsed, is exactly the donor's own undocumented shortcut this
/// change's design doc names as never actually built safely.
pub fn handle_pull_request_merged(
    config: &WebhookConfig,
    candidates: &[Trajectory],
    payload: &PullRequestMergedPayload,
) -> Result<WebhookOutcome, WebhookError> {
    if !config.enabled {
        return Ok(WebhookOutcome::Disabled);
    }
    let Some(log_event) = normalize_pull_request_merged(payload)? else {
        return Ok(WebhookOutcome::NonVerdict);
    };
    let sha = log_event.subject_sha();
    let Some(trajectory_id) = resolve_trajectory_by_sha(sha, candidates) else {
        return Ok(WebhookOutcome::UnjoinedSha(sha.clone()));
    };
    Ok(WebhookOutcome::Recorded { trajectory_id, log_event })
}

/// Receiver entry point for `workflow_run` webhook deliveries (tasks
/// 5.1/5.2/5.3/5.4). A CI conclusion is already a terminal fact the
/// instant it lands — unlike a merge's no-rollback factor, there is
/// nothing to wait on, so a `"failure"` conclusion marks a covering
/// verdict IMMEDIATELY. A `"success"` conclusion has no dedicated S4
/// table row of its own (`reward.rs`'s module doc: S4 only emits the
/// full positive triad once a `dev` outcome has FULLY resolved
/// favorably) — this module surfaces it as [`WebhookOutcome::NonVerdict`]
/// rather than fabricate a partial verdict.
pub fn handle_workflow_run(
    config: &WebhookConfig,
    rewards: &RewardRegistry,
    store: &dyn TrajectoryStore,
    candidates: &[Trajectory],
    payload: &WorkflowRunPayload,
) -> Result<WebhookOutcome, WebhookError> {
    if !config.enabled {
        return Ok(WebhookOutcome::Disabled);
    }
    let Some(ci_event) = normalize_workflow_run(payload)? else {
        return Ok(WebhookOutcome::NonVerdict);
    };
    let Some(trajectory_id) = resolve_trajectory_by_sha(&ci_event.sha, candidates) else {
        return Ok(WebhookOutcome::UnjoinedSha(ci_event.sha));
    };
    if ci_event.outcome == CiOutcome::Success {
        return Ok(WebhookOutcome::NonVerdict);
    }
    compute_and_mark(rewards, store, trajectory_id, Polarity::Failure, Becomes::GuardrailCandidate)
}

/// The `(trajectory_id, sha, merged_at)` triple a successful
/// [`WebhookOutcome::Recorded`] already carries (via `trajectory_id`
/// and `log_event`) — bundled as ONE argument for
/// [`evaluate_no_rollback_timer`] rather than three separate
/// parameters, keeping this crate's default `clippy::too_many_arguments`
/// budget.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinedMerge {
    pub trajectory_id: TrajectoryId,
    pub sha: Sha,
    pub merged_at: DateTime<Utc>,
}

/// The no-rollback timer's own entry point (task 5.5) — "a
/// scheduled/deferred check that, `no_rollback.window` after a
/// `pull_request.merged` event with no subsequent revert/rollback
/// event, marks the `no-rollback` reward factor satisfied". `as_of` is
/// the caller-supplied "now" (a real scheduled caller passes
/// `Utc::now()`; this module never reads a clock itself, keeping the
/// whole call chain deterministic and offline-testable). `merge` is
/// what [`handle_pull_request_merged`]'s [`WebhookOutcome::Recorded`]
/// already resolved via the SHA join — this function does not re-join,
/// it only re-evaluates the timer.
pub fn evaluate_no_rollback_timer(
    config: &WebhookConfig,
    rewards: &RewardRegistry,
    store: &dyn TrajectoryStore,
    merge: &JoinedMerge,
    event_log: &[WebhookLogEvent],
    as_of: DateTime<Utc>,
) -> Result<WebhookOutcome, WebhookError> {
    if !config.enabled {
        return Ok(WebhookOutcome::Disabled);
    }
    match check_no_rollback(&merge.sha, merge.merged_at, config.no_rollback_window, event_log, as_of) {
        NoRollbackStatus::Pending => {
            Ok(WebhookOutcome::AwaitingNoRollbackWindow { trajectory_id: merge.trajectory_id, sha: merge.sha.clone() })
        }
        NoRollbackStatus::Reverted => {
            compute_and_mark(rewards, store, merge.trajectory_id, Polarity::Failure, Becomes::GuardrailCandidate)
        }
        NoRollbackStatus::Satisfied => {
            compute_and_mark(rewards, store, merge.trajectory_id, Polarity::Success, Becomes::StrategyCandidate)
        }
    }
}

#[cfg(test)]
mod tests {
    use canon_model::ids::RegimeKey;

    use super::*;
    use crate::store::ParquetTrajectoryStore;
    use crate::verdict_outcome::VerdictOutcome;

    fn sha(byte: char) -> Sha {
        Sha::parse(byte.to_string().repeat(40)).unwrap()
    }

    fn regime() -> RegimeKey {
        RegimeKey::parse(canon_model::ids::regime_key("dev", "repo", "auth", "abc123")).unwrap()
    }

    /// A `dev` trajectory tagged join-ably for `sha` (the write-side
    /// convention [`sha_tag`] documents).
    fn trajectory_for(sha: &Sha) -> Trajectory {
        let verdict = VerdictRow { role: RoleId::parse("dev").unwrap(), polarity: Polarity::Success, becomes: Becomes::StrategyCandidate };
        Trajectory::new(TrajectoryId::new(), regime(), "task", "ctx", vec![verdict], Utc::now(), vec![sha_tag(sha)]).unwrap()
    }

    fn merged_payload(action: &str, merged: bool, merge_sha: &str, merged_at: Option<DateTime<Utc>>, body: Option<&str>) -> PullRequestMergedPayload {
        PullRequestMergedPayload {
            action: action.to_string(),
            pull_request: PullRequestPayload {
                number: 42,
                merged,
                merge_commit_sha: merge_sha.to_string(),
                merged_at,
                body: body.map(str::to_string),
            },
        }
    }

    fn workflow_payload(action: &str, conclusion: Option<&str>, head_sha: &str, updated_at: Option<DateTime<Utc>>) -> WorkflowRunPayload {
        WorkflowRunPayload {
            action: action.to_string(),
            workflow_run: WorkflowRunInner { head_sha: head_sha.to_string(), conclusion: conclusion.map(str::to_string), updated_at },
        }
    }

    fn enabled_config() -> WebhookConfig {
        WebhookConfig { enabled: true, no_rollback_window: Duration::hours(24) }
    }

    // ── normalize ────────────────────────────────────────────────────

    #[test]
    fn a_pr_closed_without_merging_is_non_verdict() {
        let payload = merged_payload("closed", false, &"a".repeat(40), Some(Utc::now()), None);
        assert_eq!(normalize_pull_request_merged(&payload).unwrap(), None);
    }

    #[test]
    fn an_action_other_than_closed_is_non_verdict() {
        let payload = merged_payload("opened", false, &"a".repeat(40), None, None);
        assert_eq!(normalize_pull_request_merged(&payload).unwrap(), None);
    }

    #[test]
    fn a_merged_pr_normalizes_to_a_merged_log_event() {
        let at = Utc::now();
        let payload = merged_payload("closed", true, &"a".repeat(40), Some(at), None);
        let event = normalize_pull_request_merged(&payload).unwrap().unwrap();
        assert_eq!(event, WebhookLogEvent::Merged { sha: sha('a'), pr: Some(PrNumber::parse(42).unwrap()), at });
    }

    #[test]
    fn a_merged_pr_with_no_merged_at_is_a_loud_error() {
        let payload = merged_payload("closed", true, &"a".repeat(40), None, None);
        assert!(matches!(normalize_pull_request_merged(&payload), Err(WebhookError::MissingMergedAt)));
    }

    #[test]
    fn a_malformed_merge_commit_sha_is_a_loud_join_key_error() {
        let payload = merged_payload("closed", true, "not-a-sha", Some(Utc::now()), None);
        assert!(matches!(normalize_pull_request_merged(&payload), Err(WebhookError::JoinKey(_))));
    }

    #[test]
    fn a_revert_shaped_merge_normalizes_to_reverted_of_the_original_sha() {
        let at = Utc::now();
        let body = format!("This reverts commit {}.", "a".repeat(40));
        let payload = merged_payload("closed", true, &"b".repeat(40), Some(at), Some(&body));
        let event = normalize_pull_request_merged(&payload).unwrap().unwrap();
        assert_eq!(event, WebhookLogEvent::Reverted { reverted_sha: sha('a'), at });
    }

    #[test]
    fn a_non_completed_workflow_run_is_non_verdict() {
        let payload = workflow_payload("requested", None, &"a".repeat(40), None);
        assert_eq!(normalize_workflow_run(&payload).unwrap(), None);
    }

    #[test]
    fn a_completed_success_conclusion_normalizes() {
        let at = Utc::now();
        let payload = workflow_payload("completed", Some("success"), &"a".repeat(40), Some(at));
        let event = normalize_workflow_run(&payload).unwrap().unwrap();
        assert_eq!(event, NormalizedCiEvent { sha: sha('a'), outcome: CiOutcome::Success, at });
    }

    #[test]
    fn a_completed_failure_conclusion_normalizes() {
        let at = Utc::now();
        let payload = workflow_payload("completed", Some("failure"), &"a".repeat(40), Some(at));
        let event = normalize_workflow_run(&payload).unwrap().unwrap();
        assert_eq!(event.outcome, CiOutcome::Failure);
    }

    #[test]
    fn a_completed_cancelled_conclusion_is_non_verdict() {
        let payload = workflow_payload("completed", Some("cancelled"), &"a".repeat(40), Some(Utc::now()));
        assert_eq!(normalize_workflow_run(&payload).unwrap(), None);
    }

    #[test]
    fn a_completed_run_with_no_updated_at_is_a_loud_error() {
        let payload = workflow_payload("completed", Some("failure"), &"a".repeat(40), None);
        assert!(matches!(normalize_workflow_run(&payload), Err(WebhookError::MissingUpdatedAt)));
    }

    #[test]
    fn a_realistic_github_pull_request_merged_json_payload_deserializes_and_normalizes() {
        // Trimmed to the fields this module reads, but real GitHub
        // `pull_request` webhook nesting/key-names (proves the serde
        // structs actually parse the wire format, not just Rust
        // struct literals this test file hand-constructs elsewhere).
        let json = format!(
            r#"{{
                "action": "closed",
                "number": 42,
                "pull_request": {{
                    "number": 42,
                    "merged": true,
                    "merge_commit_sha": "{sha}",
                    "merged_at": "2026-01-15T10:00:00Z",
                    "body": "Fixes the thing.",
                    "title": "Fix the thing"
                }},
                "repository": {{ "full_name": "repo-owner/canon" }}
            }}"#,
            sha = "a".repeat(40)
        );
        let payload: PullRequestMergedPayload = serde_json::from_str(&json).unwrap();
        let event = normalize_pull_request_merged(&payload).unwrap().unwrap();
        assert_eq!(
            event,
            WebhookLogEvent::Merged {
                sha: sha('a'),
                pr: Some(PrNumber::parse(42).unwrap()),
                at: DateTime::parse_from_rfc3339("2026-01-15T10:00:00Z").unwrap().with_timezone(&Utc),
            }
        );
    }

    #[test]
    fn a_realistic_github_workflow_run_completed_json_payload_deserializes_and_normalizes() {
        let json = format!(
            r#"{{
                "action": "completed",
                "workflow_run": {{
                    "id": 987654321,
                    "head_sha": "{sha}",
                    "conclusion": "failure",
                    "status": "completed",
                    "updated_at": "2026-01-15T11:30:00Z"
                }},
                "repository": {{ "full_name": "repo-owner/canon" }}
            }}"#,
            sha = "a".repeat(40)
        );
        let payload: WorkflowRunPayload = serde_json::from_str(&json).unwrap();
        let event = normalize_workflow_run(&payload).unwrap().unwrap();
        assert_eq!(event.sha, sha('a'));
        assert_eq!(event.outcome, CiOutcome::Failure);
    }

    // ── sha join ─────────────────────────────────────────────────────

    #[test]
    fn resolves_a_tagged_trajectory() {
        let t = trajectory_for(&sha('a'));
        let id = resolve_trajectory_by_sha(&sha('a'), std::slice::from_ref(&t));
        assert_eq!(id, Some(t.id));
    }

    #[test]
    fn a_sha_with_no_matching_trajectory_resolves_to_none_explicitly() {
        let t = trajectory_for(&sha('a'));
        let id = resolve_trajectory_by_sha(&sha('b'), std::slice::from_ref(&t));
        assert_eq!(id, None);
    }

    #[test]
    fn never_mis_joins_a_sha_as_if_it_were_a_trajectory_id() {
        // the donor's own broken improvisation: no candidates at all, so
        // the only way to "succeed" would be treating the sha string
        // itself as an id. This must return None, not panic or
        // fabricate an id from the sha's bytes.
        let id = resolve_trajectory_by_sha(&sha('a'), &[]);
        assert_eq!(id, None);
    }

    // ── no-rollback timer ────────────────────────────────────────────

    #[test]
    fn pending_before_the_window_elapses_with_no_revert() {
        let merged_at = Utc::now();
        let status = check_no_rollback(&sha('a'), merged_at, Duration::hours(24), &[], merged_at + Duration::hours(1));
        assert_eq!(status, NoRollbackStatus::Pending);
    }

    #[test]
    fn satisfied_once_the_window_fully_elapses_with_no_revert() {
        let merged_at = Utc::now();
        let status = check_no_rollback(&sha('a'), merged_at, Duration::hours(24), &[], merged_at + Duration::hours(24));
        assert_eq!(status, NoRollbackStatus::Satisfied);
    }

    #[test]
    fn a_revert_inside_the_window_prevents_satisfaction_even_before_the_window_elapses() {
        let merged_at = Utc::now();
        let log = vec![WebhookLogEvent::Reverted { reverted_sha: sha('a'), at: merged_at + Duration::hours(2) }];
        let status = check_no_rollback(&sha('a'), merged_at, Duration::hours(24), &log, merged_at + Duration::hours(3));
        assert_eq!(status, NoRollbackStatus::Reverted);
    }

    #[test]
    fn a_revert_inside_the_window_still_reads_reverted_after_the_window_elapses() {
        let merged_at = Utc::now();
        let log = vec![WebhookLogEvent::Reverted { reverted_sha: sha('a'), at: merged_at + Duration::hours(2) }];
        let status = check_no_rollback(&sha('a'), merged_at, Duration::hours(24), &log, merged_at + Duration::hours(48));
        assert_eq!(status, NoRollbackStatus::Reverted);
    }

    #[test]
    fn a_revert_of_a_different_sha_does_not_affect_this_sha() {
        let merged_at = Utc::now();
        let log = vec![WebhookLogEvent::Reverted { reverted_sha: sha('c'), at: merged_at + Duration::hours(2) }];
        let status = check_no_rollback(&sha('a'), merged_at, Duration::hours(24), &log, merged_at + Duration::hours(24));
        assert_eq!(status, NoRollbackStatus::Satisfied);
    }

    #[test]
    fn a_revert_landing_after_the_window_does_not_retroactively_flip_satisfied() {
        let merged_at = Utc::now();
        let log = vec![WebhookLogEvent::Reverted { reverted_sha: sha('a'), at: merged_at + Duration::hours(30) }];
        let status = check_no_rollback(&sha('a'), merged_at, Duration::hours(24), &log, merged_at + Duration::hours(30));
        assert_eq!(status, NoRollbackStatus::Satisfied);
    }

    #[test]
    fn a_revert_recorded_in_the_log_but_after_as_of_is_not_yet_observed() {
        let merged_at = Utc::now();
        let log = vec![WebhookLogEvent::Reverted { reverted_sha: sha('a'), at: merged_at + Duration::hours(2) }];
        // A reconciliation/replay caller passes the FULL persisted log
        // (including the future-relative-to-as_of revert) but asks for
        // status at an as_of strictly before the revert's own `at` —
        // the timer must not observe an event from the log's future.
        let pending = check_no_rollback(&sha('a'), merged_at, Duration::hours(24), &log, merged_at + Duration::hours(1));
        assert_eq!(pending, NoRollbackStatus::Pending);
        let reverted = check_no_rollback(&sha('a'), merged_at, Duration::hours(24), &log, merged_at + Duration::hours(2));
        assert_eq!(reverted, NoRollbackStatus::Reverted);
    }

    // ── config ───────────────────────────────────────────────────────

    #[test]
    fn empty_manifest_is_disabled_at_the_default_window() {
        let config = WebhookConfig::from_manifest("").unwrap();
        assert_eq!(config, WebhookConfig::default());
        assert!(!config.enabled);
        assert_eq!(config.no_rollback_window, Duration::hours(DEFAULT_NO_ROLLBACK_WINDOW_HOURS));
    }

    #[test]
    fn explicit_enabled_and_window_are_parsed() {
        let yaml = "webhook:\n  enabled: true\nno_rollback:\n  window_hours: 48\n";
        let config = WebhookConfig::from_manifest(yaml).unwrap();
        assert!(config.enabled);
        assert_eq!(config.no_rollback_window, Duration::hours(48));
    }

    #[test]
    fn manifest_with_other_sections_only_reads_webhook_and_no_rollback() {
        let yaml = "learn:\n  root: canon/learn-custom\n";
        let config = WebhookConfig::from_manifest(yaml).unwrap();
        assert_eq!(config, WebhookConfig::default());
    }

    #[test]
    fn a_non_positive_window_fails_loud() {
        let yaml = "no_rollback:\n  window_hours: 0\n";
        assert!(matches!(WebhookConfig::from_manifest(yaml), Err(WebhookError::Config(_))));
    }

    // ── pipeline ─────────────────────────────────────────────────────

    #[test]
    fn disabled_config_is_a_clean_no_op_across_every_entry_point() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetTrajectoryStore::open(dir.path());
        let t = trajectory_for(&sha('a'));
        store.append(&t).unwrap();
        let config = WebhookConfig::default();
        let rewards = RewardRegistry::builtin();

        let merged = merged_payload("closed", true, &"a".repeat(40), Some(Utc::now()), None);
        assert_eq!(handle_pull_request_merged(&config, std::slice::from_ref(&t), &merged).unwrap(), WebhookOutcome::Disabled);

        let ci = workflow_payload("completed", Some("failure"), &"a".repeat(40), Some(Utc::now()));
        assert_eq!(
            handle_workflow_run(&config, &rewards, &store, std::slice::from_ref(&t), &ci).unwrap(),
            WebhookOutcome::Disabled
        );

        let outcome = evaluate_no_rollback_timer(
            &config,
            &rewards,
            &store,
            &JoinedMerge { trajectory_id: t.id, sha: sha('a'), merged_at: Utc::now() },
            &[],
            Utc::now() + Duration::hours(48),
        )
        .unwrap();
        assert_eq!(outcome, WebhookOutcome::Disabled);

        // The store must be untouched — still Pending at the default.
        let found = store.find_by_id(&t.id).unwrap().unwrap();
        assert!(found.verdict_record.is_pending());
    }

    #[test]
    fn an_unjoined_sha_is_reported_explicitly_never_mis_joined() {
        let config = enabled_config();
        let t = trajectory_for(&sha('a'));
        let merged = merged_payload("closed", true, &"b".repeat(40), Some(Utc::now()), None);
        let outcome = handle_pull_request_merged(&config, std::slice::from_ref(&t), &merged).unwrap();
        assert_eq!(outcome, WebhookOutcome::UnjoinedSha(sha('b')));
    }

    #[test]
    fn a_merged_pr_payload_normalizes_and_joins_but_does_not_yet_mark() {
        let config = enabled_config();
        let t = trajectory_for(&sha('a'));
        let merged_at = Utc::now();
        let merged = merged_payload("closed", true, &"a".repeat(40), Some(merged_at), None);
        let outcome = handle_pull_request_merged(&config, std::slice::from_ref(&t), &merged).unwrap();
        assert_eq!(
            outcome,
            WebhookOutcome::Recorded {
                trajectory_id: t.id,
                log_event: WebhookLogEvent::Merged { sha: sha('a'), pr: Some(PrNumber::parse(42).unwrap()), at: merged_at },
            }
        );
    }

    #[test]
    fn a_merged_pr_payload_end_to_end_marks_the_joined_trajectory_success_once_the_window_elapses() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetTrajectoryStore::open(dir.path());
        let t = trajectory_for(&sha('a'));
        store.append(&t).unwrap();
        let config = enabled_config();
        let rewards = RewardRegistry::builtin();
        let merged_at = Utc::now();
        let merged = merged_payload("closed", true, &"a".repeat(40), Some(merged_at), None);

        let recorded = handle_pull_request_merged(&config, std::slice::from_ref(&t), &merged).unwrap();
        let WebhookOutcome::Recorded { trajectory_id, log_event } = recorded else { panic!("expected Recorded") };

        let outcome = evaluate_no_rollback_timer(
            &config,
            &rewards,
            &store,
            &JoinedMerge { trajectory_id, sha: log_event.subject_sha().clone(), merged_at },
            &[],
            merged_at + Duration::hours(24),
        )
        .unwrap();

        assert_eq!(outcome, WebhookOutcome::Marked { trajectory_id, verdict: TrajectoryVerdict::new(VerdictOutcome::Success, 1.0) });
        let found = store.find_by_id(&t.id).unwrap().unwrap();
        assert_eq!(found.verdict_record.outcome, VerdictOutcome::Success);
        assert_eq!(found.verdict_record.reward, 1.0);
    }

    #[test]
    fn a_revert_inside_the_window_marks_failure_instead_of_success() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetTrajectoryStore::open(dir.path());
        let t = trajectory_for(&sha('a'));
        store.append(&t).unwrap();
        let config = enabled_config();
        let rewards = RewardRegistry::builtin();
        let merged_at = Utc::now();
        let log = vec![WebhookLogEvent::Reverted { reverted_sha: sha('a'), at: merged_at + Duration::hours(2) }];

        let merge = JoinedMerge { trajectory_id: t.id, sha: sha('a'), merged_at };
        let outcome = evaluate_no_rollback_timer(&config, &rewards, &store, &merge, &log, merged_at + Duration::hours(24)).unwrap();

        assert_eq!(outcome, WebhookOutcome::Marked { trajectory_id: t.id, verdict: TrajectoryVerdict::new(VerdictOutcome::Failure, 0.1) });
        let found = store.find_by_id(&t.id).unwrap().unwrap();
        assert_eq!(found.verdict_record.outcome, VerdictOutcome::Failure);
    }

    #[test]
    fn awaiting_the_window_marks_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetTrajectoryStore::open(dir.path());
        let t = trajectory_for(&sha('a'));
        store.append(&t).unwrap();
        let config = enabled_config();
        let rewards = RewardRegistry::builtin();
        let merged_at = Utc::now();

        let merge = JoinedMerge { trajectory_id: t.id, sha: sha('a'), merged_at };
        let outcome = evaluate_no_rollback_timer(&config, &rewards, &store, &merge, &[], merged_at + Duration::hours(1)).unwrap();

        assert_eq!(outcome, WebhookOutcome::AwaitingNoRollbackWindow { trajectory_id: t.id, sha: sha('a') });
        let found = store.find_by_id(&t.id).unwrap().unwrap();
        assert!(found.verdict_record.is_pending());
    }

    #[test]
    fn a_workflow_run_failure_marks_failure_immediately() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetTrajectoryStore::open(dir.path());
        let t = trajectory_for(&sha('a'));
        store.append(&t).unwrap();
        let config = enabled_config();
        let rewards = RewardRegistry::builtin();
        let ci = workflow_payload("completed", Some("failure"), &"a".repeat(40), Some(Utc::now()));

        let outcome = handle_workflow_run(&config, &rewards, &store, std::slice::from_ref(&t), &ci).unwrap();

        assert_eq!(outcome, WebhookOutcome::Marked { trajectory_id: t.id, verdict: TrajectoryVerdict::new(VerdictOutcome::Failure, 0.1) });
        let found = store.find_by_id(&t.id).unwrap().unwrap();
        assert_eq!(found.verdict_record.outcome, VerdictOutcome::Failure);
        assert_eq!(found.verdict_record.reward, 0.1);
    }

    #[test]
    fn a_workflow_run_success_marks_nothing_no_dedicated_table_row() {
        let config = enabled_config();
        let rewards = RewardRegistry::builtin();
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetTrajectoryStore::open(dir.path());
        let t = trajectory_for(&sha('a'));
        store.append(&t).unwrap();
        let ci = workflow_payload("completed", Some("success"), &"a".repeat(40), Some(Utc::now()));

        let outcome = handle_workflow_run(&config, &rewards, &store, std::slice::from_ref(&t), &ci).unwrap();
        assert_eq!(outcome, WebhookOutcome::NonVerdict);
        let found = store.find_by_id(&t.id).unwrap().unwrap();
        assert!(found.verdict_record.is_pending());
    }

    #[test]
    fn a_workflow_run_with_unjoined_sha_is_reported_explicitly() {
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetTrajectoryStore::open(dir.path());
        let t = trajectory_for(&sha('a'));
        store.append(&t).unwrap();
        let config = enabled_config();
        let rewards = RewardRegistry::builtin();
        let ci = workflow_payload("completed", Some("failure"), &"b".repeat(40), Some(Utc::now()));

        let outcome = handle_workflow_run(&config, &rewards, &store, std::slice::from_ref(&t), &ci).unwrap();
        assert_eq!(outcome, WebhookOutcome::UnjoinedSha(sha('b')));
    }
}
