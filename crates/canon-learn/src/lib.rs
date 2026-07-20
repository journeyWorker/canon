//! `canon-learn`: role-namespaced strategy memory (S6).
//!
//! Generalizes the donor harness's reasoning-bank trace→verdict→
//! distill→store→retrieve→apply loop (raw `PatternTrajectory` +
//! distilled `StrategyMemoryItem`, closed `dev|content|sim`
//! `PatternNamespace`) and the donor tuning project's sweep-trajectory
//! write/read-symmetric `simRetrievalKey` discipline into a single,
//! open, [`RegimeKey`](canon_model::ids::RegimeKey)-keyed two-tier
//! store any role/repo/area can populate:
//!
//! - [`role`] — the open role registry (design decision 1): built-in
//!   `planning|design|dev|test|review|content|sim`, `canon.yaml`-
//!   extensible, never a closed Rust `enum`.
//! - [`trajectory`] — the raw, cold, immutable tier: one captured trace
//!   ([`canon_ingest::verdict::VerdictRow`]s + the reasoning/context
//!   that produced them), keyed by `regime_key`.
//! - [`strategy`] — the distilled, warm tier: title/description/
//!   content strategy items with `source_trajectory_ids` provenance.
//! - [`store`] — [`store::TrajectoryStore`]/[`store::StrategyStore`],
//!   both backed by [`store::ParquetTrajectoryStore`]/
//!   [`store::ParquetStrategyStore`] for THIS change (OQ2 resolved
//!   parquet-first, `store` module doc) behind a trait a future
//!   vector-backed impl can implement without a caller rewrite.
//! - [`distill`] — the deterministic (non-LLM) distiller: raw
//!   trajectories -> distilled strategy items.
//! - [`rebuild_namespace`] — the non-destructive delete-rebuild
//!   primitive (design decision 3): re-derives ONE regime's strategy
//!   items from its retained, untouched raw trajectories.
//! - [`retrieve`] — the read side of the apply loop: a namespace's
//!   strategy items, deterministically ordered.
//! - [`write::store_trajectory`] — the role-registry-gated write path.
//! - [`guidance`] — S8 (`retrieve-before-task`)'s library core:
//!   [`guidance::retrieve_guidance`], a fail-soft-at-the-type-level
//!   wrapper over [`retrieve`] (Vec, never Result — a store outage or
//!   malformed row degrades to empty guidance, logged, never
//!   propagated) that excludes demoted strategies and caps at `k`; and
//!   [`guidance::manifest_guidance_for_replay`], the named replay/
//!   live-retrieval boundary that returns a `canon_model::Run`'s
//!   recorded `injected_guidance` unchanged, never a fresh lookup.
//!   `canon retrieve` (the CLI) and the pre-dispatch hook wiring that
//!   populates `injected_guidance` at real dispatch time are S8 part2,
//!   deferred (`guidance` module doc).
//! - [`verdict_outcome`] — S7's [`verdict_outcome::VerdictOutcome`]/
//!   [`verdict_outcome::TrajectoryVerdict`]: the rolled-up outcome+
//!   reward pair a [`trajectory::Trajectory`] carries alongside its raw
//!   `VerdictRow` evidence (design D2).
//! - [`reward`] — S7's per-role [`reward::RewardFn`] registry
//!   ([`reward::RewardRegistry`]), generalizing `computeDevReward`'s
//!   weighted composite (design D1).
//! - [`mark_verdict::mark_trajectory_verdict`] — S7's completion of
//!   this crate's own write-back surface (design D2): writes a
//!   covering verdict+reward onto a stored trajectory, never leaving it
//!   `Pending` once called.
//! - [`promotion`] — the S7 statistical-promotion seam (design D3/D4):
//!   [`promotion::PromotionGate`], [`promotion::PromotionDecision`],
//!   [`promotion::CrnPromotionGate`] (task group 2 — a clean-room port
//!   of MaTTS's pure statistics core, `promotion::crn` module
//!   doc), and (task group 3) [`promotion::OccurrencePromotionGate`]
//!   — the n-occurrence + zero-contradiction gate for domains that
//!   can't run CRN replay (`promotion::occurrence` module doc). (task
//!   group 4) [`promotion::demote_strategy`] — WIDENED from S7Core's
//!   original 2-arg stub to take real persistence context
//!   (`promotion::demote` module doc); a role picks its gate via
//!   [`LearnConfig::promotion_config_for`] (`promotion.<role>.mode`,
//!   reconciled into `canon.yaml`'s `learn:` section — `config` module
//!   doc's "S7 task 3.2 reconciliation"). `PromotionGate::evaluate`
//!   takes an explicit `as_of: DateTime<Utc>` — a PURE function of
//!   `(regime_key, samples, as_of)`, never `Utc::now()` read
//!   internally (`promotion` module doc); [`promotion::evaluate_now`]
//!   is the one non-trait convenience that reads the wall clock for a
//!   live caller.
//! - [`webhook`] — S7's PR/CI webhook receiver (design D5, task group
//!   5): normalizes GitHub `pull_request.merged`/`workflow_run.
//!   conclusion` payloads into S4's `VerdictRow` shape, then calls
//!   [`reward::RewardRegistry::compute`] +
//!   [`mark_verdict::mark_trajectory_verdict`]. Resolves the commit SHA
//!   to a trajectory via S1's typed `Sha` join-spine key
//!   ([`webhook::resolve_trajectory_by_sha`], task 5.4 — closes the
//!   join the donor's own webhook translator never built, which borrowed
//!   the SHA itself as a trajectory-id slot). Gates the `no-rollback`
//!   reward factor behind a deterministic, offline-testable timer
//!   ([`webhook::check_no_rollback`], task 5.5 — the donor's first real
//!   implementation of a timer its own docs deferred and never
//!   shipped). Gated behind `canon.yaml` `webhook.enabled`
//!   ([`webhook::WebhookConfig`], task 5.3); no HTTP server ships in
//!   this change (`webhook` module doc, Migration Step 1).
//!
//! Out of this crate's scope (see
//! `openspec/changes/{s6-role-strategy-memory,s7-reward-statistical-
//! promotion}/{design,tasks}.md`):
//! git-tier strategy promotion (`canon learn promote`, a `canon-cli`
//! surface — [`promotion::demote_strategy`]'s own git-tier soft-flag/
//! hard-delete only ever touches a file `canon learn promote` would
//! have written; a strategy never promoted to the git tier has
//! nothing to touch, see `promotion::demote` module doc), the
//! donor harness cutover plan, any production `canon ingest`
//! artifact-driver wiring a real `VerdictRow` pipeline into this
//! crate end-to-end (deferred — this crate's own tests build
//! `VerdictRow`s synthetically), and an actual HTTP server for the
//! webhook receiver (`webhook` module doc, Migration Step 1 — the
//! receiver's normalize/join/reward/mark_trajectory_verdict LOGIC is
//! implemented and tested against synthetic payloads; wiring a real
//! listener is `canon-cli`/deployment territory).

pub mod config;
pub mod distill;
pub mod error;
pub mod guidance;
pub mod ids;
pub mod mark_verdict;
pub mod promotion;
pub mod rebuild;
pub mod retrieve;
pub mod reward;
pub mod role;
pub mod store;
pub mod strategy;
pub mod trajectory;
pub mod verdict_outcome;
pub mod webhook;
pub mod write;

pub use config::{DemotionConfig, LearnConfig, PromotionMode, PromotionRoleConfig};
pub use distill::{distill_namespace, distill_trajectory};
pub use error::LearnError;
pub use guidance::{DEFAULT_K, manifest_guidance_for_replay, retrieve_first_nonempty, retrieve_guidance};
pub use ids::{StrategyId, TrajectoryId};
pub use mark_verdict::mark_trajectory_verdict;
pub use promotion::{
    CrnPromotionGate, DemotionPolicy, DemotionRecord, OccurrencePromotionGate, Promotion, PromotionDecision, PromotionGate,
    demote_strategy, evaluate_now, plan_promotion, promote_strategy,
};
pub use rebuild::rebuild_namespace;
pub use retrieve::retrieve;
pub use reward::{RewardFn, RewardRegistry};
pub use role::RoleRegistry;
pub use store::{ParquetStrategyStore, ParquetTrajectoryStore, StrategyStore, TrajectoryStore};
pub use strategy::{DemotionEvidence, StrategyItem};
pub use trajectory::Trajectory;
pub use verdict_outcome::{TrajectoryVerdict, VerdictOutcome};
pub use webhook::{
    JoinedMerge, WebhookConfig, WebhookError, WebhookOutcome, evaluate_no_rollback_timer, handle_pull_request_merged,
    handle_workflow_run,
};
pub use write::store_trajectory;
