//! `canon ingest artifacts [--repo <dir>] [--watch]` (S14
//! `s14-artifact-ingest-cli`): the artifact/verdict half of canon's
//! join-spine driver — the exact "future `canon ingest` artifact-ingest
//! CLI wiring" `crates/canon-ingest/src/artifact_registry.rs`'s own doc
//! comment and `openspec/changes/{s4-artifact-ingest,s6-role-strategy-
//! memory}/tasks.md`'s honesty notes name as a DEFERRED residual —
//! mirrors [`crate::ingest`]'s already-proven `canon ingest sessions`
//! shape (adapters -> normalize/derive -> persist, with a documented
//! seam for whatever couldn't be persisted), generalized from
//! `SessionAdapter`/canon-store to `ArtifactAdapter`/canon-learn.
//!
//! # The S4 dependency boundary (frozen, never crossed here)
//! `canon-ingest` stays canon-store-free at runtime (S4 task 0.1) —
//! this module is where the two meet, exactly like `crate::ingest` is
//! for `SessionAdapter`/canon-store. Two adapter shapes, fed two
//! different ways:
//! - **`Path`-source adapters** (`ledger`/`divergence`/`openspec-task`)
//!   resolve their own root from the `canon.yaml`-configured
//!   [`canon_ingest::ArtifactSourceConfig`] and scan it directly —
//!   [`canon_ingest::artifact_registry::resolve_and_parse`]'s existing
//!   config-driven scan path, unchanged, is the ONLY thing this module
//!   calls for them.
//! - **`Records`-source adapters** (today, only `handoff`) cannot be
//!   driven by that config-driven scan at all — `resolve_and_parse`
//!   returns [`canon_ingest::artifact_registry::ArtifactDispatchOutcome::UnsupportedSource`]
//!   for them, by design (the P1 silent-drop `ReviewS4Full` fixed).
//!   THIS driver is what `artifact_registry`'s own doc comment names as
//!   the missing piece: it reads canon's own `Handoff`/`Review`/
//!   `Divergence` records straight off `canon-store`'s `Tier` (via a
//!   [`crate::tiers::build_lenient_tiers_for_kinds`] built ONCE, up
//!   front, over the union of every `Records`-source adapter's own
//!   `RecordKind` this pass will actually run — s29 design D6, before
//!   which this module called the strict [`crate::tiers::build_tiers`]
//!   PER adapter — + [`canon_store::registry::TierRegistry::query`],
//!   the same read path `canon query` uses) and hands the resulting
//!   `Vec<RawRecord>` to [`canon_ingest::ArtifactAdapter::parse`]
//!   directly as [`canon_ingest::ArtifactSourceHandle::Records`] —
//!   `canon-ingest` itself never touches `canon-store`.
//!
//! Every adapter's contribution (or its absence, and why) is reported
//! in [`ArtifactIngestOutcome::adapters`] — a `Records`-source adapter
//! whose read failed degrades to `status: "unavailable"` with the
//! reason, it is NEVER folded into a silent zero-events outcome
//! indistinguishable from "nothing to report" (the exact collapse
//! `ArtifactDispatchOutcome::UnsupportedSource` exists to prevent one
//! layer down — this driver extends that same discipline to its own
//! records-source read step).
//!
//! # Verdict derivation and persistence
//! Every collected [`canon_ingest::ArtifactEvent`] is folded through
//! [`canon_ingest::verdict::derive_verdict`] +
//! [`canon_ingest::verdict::attach_regime_key`] (S4's own frozen,
//! table-driven mapping — this module adds no verdict logic of its
//! own), grouped by the resulting `regime_key` into
//! [`canon_learn::Trajectory`]s, and persisted via
//! [`canon_learn::store_trajectory`] into the SAME
//! `ParquetTrajectoryStore` (under the `canon.yaml`-configured
//! `learn.root`, `canon_learn::LearnConfig::root`) that `canon
//! retrieve` and `canon-report`'s marts already read — no second store,
//! no new seam in `canon-learn` was needed (`store_trajectory`/
//! `Trajectory::new`/`distill_namespace` are already public API,
//! `crates/canon-learn/tests/fixture_round_trip.rs` already proves the
//! exact store->distill->rebuild->search round trip this module drives
//! with SYNTHETIC data; this module is the real-data caller that test's
//! own doc comment names as the deferred residual). Immediately after a
//! successful persist, [`rebuild_namespace_converged`] re-derives that
//! regime's distilled `StrategyItem`s too — without this, S9's
//! `mart_role_memory` (which reads ONLY the distilled tier,
//! `stg_strategy_items`) would stay empty even after a successful
//! ingest, defeating this whole change's purpose.
//!
//! `regime_key`'s `<hash>` component is
//! [`canon_ingest::normalize::content_digest`] of the source event's
//! own [`event_identity`] (`scenario:<id>@<project_id>` /
//! `scenario:<id>@-` when the event names no project /
//! `handoff:<id>` / `task:<id>`) — the SAME digest primitive S3
//! session-ingest already uses for its own write-identity, reused here
//! (not a new hashing scheme) to give related events sharing one
//! identity (e.g. an open review finding and its later remediation on
//! ONE project's scenario) the identical `regime_key`, folding them
//! onto ONE trajectory exactly as [`canon_learn::Trajectory`]'s own doc
//! comment describes. The project half is load-bearing: a `Scenario`'s
//! identity is the PAIR `(project_id, scenario_id)`, so without it two
//! projects defining one scenario id folded onto one regime (see
//! [`join_key_identity`]).
//!
//! Write-time idempotence (S4 tasks.md group 6) IS
//! enforced here, over the trajectory's FULL identity
//! ([`trajectory_content_digest`]: an identity-shape version, the
//! `regime_key`, the ordered `VerdictRow` contents, AND the rendered
//! `task`/`context` — reusing the SAME [`content_digest`] primitive
//! `regime_key`'s own `<hash>` component uses, not a new hashing
//! scheme). Before persisting, [`run`] compares that identity against
//! every trajectory already recorded under the EXACT SAME `regime_key`
//! and skips the persist when one matches (counted,
//! `ArtifactIngestOutcome::trajectories_skipped_duplicate`) rather than
//! double-writing — a repeat `canon ingest artifacts` pass over an
//! UNCHANGED corpus re-derives the identical identity
//! (`canon_ingest::scanner::scan_dir`'s deterministic byte-lexical file
//! order, plus [`group_by_regime`]'s total data-derived sort, means the
//! SAME `VerdictRow` sequence AND the same rendered bytes every pass)
//! and persists nothing new.
//!
//! # A rebuild supersedes this driver's own older derivation
//! Folding the RENDERED text into that identity is what makes an
//! UPGRADE converge. The pre-s38 digest covered only `regime_key` +
//! ordered verdict rows and was compared BEFORE the text was rendered,
//! so every trajectory written before s38/s39 kept its digest across
//! the text change, was skipped as a duplicate on the next pass, and
//! re-distilled its old ingest-driver prose forever — an upgrading user
//! never gained evidence lines, antecedents, or joined scenario titles
//! unless they deleted `.canon/learn` by hand. Hand-deleting a store is
//! not an upgrade path, so it is not this driver's contract.
//!
//! A fresher identity ALONE would only double the corpus, though: the
//! stale row still sits in the namespace and a distill pass folds every
//! row handed to it. So the identity is split in two.
//! [`trajectory_derivation_key`] names the LOGICAL verdict set
//! (`regime_key` + the ordered rows — the pre-s38 digest's exact shape,
//! deliberately unversioned so it still recognizes rows written by any
//! earlier canon), and a re-derivation of a logical verdict set this
//! driver already wrote REPLACES that row rather than landing beside it,
//! by reusing the stored row's own [`canon_learn::TrajectoryId`].
//! [`plan_trajectory`] states the rule and why that id is the stable
//! choice. A genuinely NEW verdict set (a different or additional
//! verdict folded onto the same `regime_key`) is still a FRESH row
//! alongside the untouched prior ones — the raw tier stays append-only
//! across DISTINCT derivations (design decision 3); the only bytes this
//! driver ever overwrites are its own previous answer to the identical
//! question.
//!
//! ## Two stored copies of one verdict set converge too
//! s41 (`review-hardening`) first stated that rule for a SINGLE stale
//! row, arguing the candidate set could never be larger because this
//! driver writes one row per verdict set. The re-review rejected the
//! argument, and rightly: a pre-s38 pass appended a SECOND row whenever
//! the renderer changed under it (its digest ignored rendered text), so
//! an upgrading user really can hold two rows for one verdict set — and
//! then whichever sibling happened to carry the fresh text made the
//! duplicate check return before supersession ran, stranding the other
//! row's pre-s38 prose in the distilled tier forever.
//!
//! So the whole candidate set converges, on ONE rule stated in ONE place
//! ([`canonical_driver_row`]): among this driver's own rows sharing a
//! derivation key, the SMALLEST id is CANONICAL and every other is
//! SUPERSEDED. [`plan_trajectory`] keeps the canonical row current — it
//! skips a pass only when the CANONICAL row itself already carries the
//! freshly rendered identity, never because a sibling does — and
//! [`rebuild_namespace_converged`] distills what supersession left and
//! removes the superseded rows it is allowed to remove, so one logical
//! verdict set becomes one strategy set instead of one per stored copy.
//! One pass converges any number of copies: the rule is a pure function
//! of the stored rows, so both halves pick the same canonical row
//! without passing any state.
//!
//! ## …and the superseded rows are removed, not merely withheld
//! s41 could only withhold them. `canon_learn::TrajectoryStore` had no
//! deletion primitive at all, so an upgraded namespace's raw row COUNT
//! could never shrink and N stored copies of one verdict set stayed N
//! files forever — every tier a reader consumed converged while the raw
//! layer accumulated. s42 (`close-the-open-loops`) added
//! `canon_learn::TrajectoryStore::delete_by_id` and
//! [`rebuild_namespace_converged`] uses it, so the raw layer converges
//! too.
//!
//! That destroys something, which is worth naming plainly. A superseded
//! row's bytes were the audit trail of what an EARLIER canon rendered for
//! this verdict set, and once deleted three things stop resolving: a
//! `source_trajectory_ids` provenance pointer in a strategy promoted
//! (git-tracked, front-matter) before this convergence rule existed, back
//! when a superseded copy could still be distilled; a
//! `canon_learn::mark_trajectory_verdict` aimed at that id from the S7
//! webhook path, which now fails loud rather than marking a row nothing
//! reads; and the row's own line in S9's `stg_trajectories`, so the
//! funnel's verdict count stops counting one logical verdict set once per
//! stored copy.
//!
//! ### The derivation key is NOT enough to authorize that delete
//! Being superseded means only that the canonical row answers the same
//! QUESTION: same `regime_key`, same ordered `VerdictRow`s (that IS
//! [`trajectory_derivation_key`]), same [`ARTIFACT_INGEST_TAG`]. It says
//! nothing about the two fields a stored row carries that no derivation
//! determines, because both are written independently and LATER
//! ([`RowProvenance`]): S7's rolled-up `verdict_record` (outcome+reward,
//! written back by `canon_learn::mark_trajectory_verdict` — this
//! driver's own persist loop, or the S7 webhook receiver) and the
//! `run_id` a `--run` pass stamped. Both are deliberately outside
//! [`trajectory_content_digest`] AND [`trajectory_derivation_key`], for
//! reasons stated at each, so two rows CAN share a derivation key and
//! still disagree about them.
//!
//! Deleting on the derivation key alone would therefore destroy real
//! evidence, not an old rendering. `canon learn promote` reads exactly
//! these fields off `query_by_regime_key`'s rows — `OccurrencePromotionGate`
//! folds `verdict_record.outcome` (one `Failure` resets its success
//! streak) and `CrnPromotionGate` folds `verdict_record.reward` — so
//! dropping a losing row recorded `Failure` while the canonical row holds
//! `Success` removes the contradiction the gate was refusing on, and
//! turns a refusal into a promotion. And dropping a losing row's `run_id`
//! can remove this regime's last trajectory attributing that run, which
//! is the row `mart_flywheel_funnel`'s attribution stage joins on
//! (`(run_id, role)`) — the canonical row does not answer for it, because
//! attribution is stamped per WRITE, not per verdict set.
//!
//! So the delete carries a SECOND bar, on top of supersession
//! ([`RawConvergence`]): a superseded row is removed only when every
//! independently-written field it carries is also carried by the
//! canonical row — either unset on the loser (`Pending` at the default
//! reward; no `run_id`), i.e. no claim at all, or byte-for-byte the
//! canonical row's own value. For those rows, and only those, the
//! original statement holds exactly: no verdict evidence is lost, only an
//! older RENDERING of evidence the canonical row still carries verbatim.
//!
//! A superseded row that fails the bar is KEPT — not merged, not
//! deleted, and reported
//! ([`ArtifactIngestOutcome::convergence_conflicts`]). Keeping it is the
//! conservative direction on purpose: a `Failure` and a `Success` for one
//! verdict set have no lossless merge, so any automatic resolution is a
//! choice about promotion evidence made by a garbage collector, whereas
//! the cost of keeping the row is one parquet file and one standing
//! report line an operator can act on. It stays withheld from the
//! distiller exactly as s41 left it, so its stale prose is still served
//! to nobody; the raw file simply survives until a human decides which
//! verdict is true. The convergence is a fixpoint either way — a kept
//! conflict is re-reported, never re-deleted, and the distilled tier it
//! rebuilds is byte-identical every pass.
//!
//! And s41 had already denied superseded rows every tier a reader
//! consumes, so nothing served to an agent could cite them before the
//! delete either; what changes is that the deletable ones stop costing
//! bytes and stop inflating a raw-tier count. A row this driver did not
//! write is never a candidate ([`canonical_driver_row`] rung 2), so no
//! fixture, no webhook write and no future writer's evidence is reachable
//! down this path.
//!
//! # Documented seam
//! A `Records`-source adapter's read genuinely CAN fail — no live
//! `tiers.pg` DSN (s29 design D6: the printed reason now names the
//! configured `dsn_env`/`bucket_env`, never a bare guess), `canon.yaml`
//! missing/unreadable, or `handoff`/`review`/`divergence-native`
//! simply unrouted — that adapter alone degrades to zero events
//! (reported, never silent, see above) while every `Path`-source
//! adapter and the persistence step still run normally. A genuinely
//! MALFORMED `canon.yaml` (bad YAML/policy syntax, an invalid pg
//! schema, a non-forward aging rule, …) is NOT this per-adapter
//! degrade, though (s29 design D6): it fails the WHOLE `canon ingest
//! artifacts` command loud, exactly like `crate::ingest`'s own
//! contract — "lenient" describes rung reachability only, config
//! correctness always stays loud. `canon-learn`'s own parquet store
//! has no analogous "unreachable" failure mode (`ParquetTrajectoryStore::open`
//! never fails — it is a bare `PathBuf`, directories are created lazily
//! on write), so unlike `crate::ingest`'s whole-batch unwritten
//! fallback, this driver's persistence step degrades per-trajectory: an
//! unregistered role (`canon.yaml` `learn.roles`) is skipped and
//! counted (`ArtifactIngestOutcome::trajectories_skipped_unregistered_role`),
//! never a fatal error for the rest of the batch.
//!
//! # S15 P4: native verdict adapters (design D7)
//! Two more `Records`-source adapters, `review`/`divergence-native`,
//! read canon's OWN `Review`/`Divergence` tiers (never a raw
//! ledger/divergence-manifest artifact) into verdicts — tagged
//! [`canon_ingest::artifact_registry::ArtifactAdapterEntry::native_verdict`]
//! `true` (`handoff` stays `false`: `Records`-kind but not a native
//! verdict source). They are driven ONLY when
//! `ArtifactSourceConfig::native_records` is `true` — [`run`] validates
//! ([`validate_artifact_source_config`]) that this switch is
//! XOR-exclusive with `ledger_root`/`divergences_root`/`openspec_root`
//! BEFORE any adapter read runs (spec `native-record-flywheel`
//! Requirement 3: the raw and native paths' verdict rows differ
//! slightly, so [`trajectory_content_digest`] would not dedupe them,
//! silently double-counting the same evidence). A `native_verdict:
//! true` entry with the switch off reports `status: "disabled"`
//! (never `"unavailable"`, reserved for a genuine read failure) and is
//! skipped entirely — `handoff` is UNAFFECTED and always runs. Their
//! events carry `detail["native_kind"]` (`"review"` |
//! `"divergence"`, plus `"status"` for the latter) instead of a
//! `derive_verdict`-mapped [`canon_ingest::ArtifactEventKind`] (always
//! `NonVerdict` for these two) — [`derive_verdict_for_event`] reads
//! that tag to dispatch to
//! [`canon_ingest::verdict::derive_native_review_verdict`]/
//! [`canon_ingest::verdict::derive_native_divergence_verdict`] instead
//! of [`canon_ingest::verdict::derive_verdict`], then rejoins the SAME
//! `attach_regime_key` + grouping + `trajectory_content_digest` +
//! `store_trajectory` + [`rebuild_namespace_converged`] path every
//! other event already uses.
//!
//! # s39: the antecedent join (`joined-evidence-grounding`)
//! The verdict derivation above is a FILTER. An event
//! [`derive_verdict_for_event`] scores `None` for — most plainly an
//! `open`/`deferred` native divergence, which
//! [`canon_ingest::verdict::derive_native_divergence_verdict`] maps to
//! `None` CORRECTLY, because an open finding is not an outcome yet —
//! never reaches the accumulator, and its prose goes with it. On
//! canon's own corpus that discards the richest evidence the repo
//! holds: the `open` ship-blocker findings, each a file-and-line
//! locator plus a symptom and a remedy, while what survives is the bare
//! resolution narrative ("Fixed and independently re-verified+attested
//! at `<sha>`"). canon distilled THAT it got fixed and threw away WHAT
//! was broken — and both halves were already parsed, in the same batch,
//! in the same `Vec`, one loop before the drop.
//!
//! s39 (`joined-evidence-grounding`) closes that read-side, over the
//! join spine, with no new record kind and no extra tier read:
//! [`index_antecedents`] indexes every prose-bearing NON-verdict event
//! by its [`event_identity`] before the accumulation loop runs, and
//! each verdict-bearing event absorbs ([`antecedents_before`]) the
//! indexed events on its OWN identity at or before it in time. That
//! identity carries the `project_id` for a scenario-keyed event, and it
//! is the SAME string [`regime_hash`] digests — so "shares my join key"
//! and "would have folded onto my regime" can never disagree, and one
//! project's finding can never attach to another project's verdict. A
//! `resolved` divergence therefore distills together with the `open`
//! findings it closed, rendered as a visibly distinct block in the
//! trajectory's `context` ([`RegimeEvidence::trajectory_text`]).
//!
//! Nothing about verdict derivation moves. An open finding still mints
//! no verdict, no reward and no trajectory of its own — it contributes
//! evidence TEXT to a trajectory some OTHER event's verdict created.
//!
//! # s42: attributing a trajectory to its dispatched run
//! [`run`] takes an OPTIONAL [`RunId`] — `canon ingest artifacts --run`,
//! s42 (`close-the-open-loops`) task 3.2 — and stamps it onto every
//! trajectory this pass WRITES ([`plan_trajectory`]). That closes s40
//! task 3.1, which asked `mart_flywheel_funnel`'s `applied` stage to
//! count "a resolved trajectory joined to its own run" and was left open
//! as unimplementable: this driver mints a trajectory from an ARTIFACT,
//! so it had no run in hand, and `canon_learn::Trajectory` carried no
//! run id to put one in.
//!
//! The value is PASSED, never derived. Nothing here consults the wall
//! clock, the authoring role, or the newest file in `.canon/dispatch/` to
//! decide which run an artifact "probably" belongs to — an ingest outside
//! a dispatched run leaves the field absent and the mart falls back to
//! its labelled proxy. A `Some` is admitted against this repo's own
//! dispatch manifests ([`resolve_dispatched_run`]) before any adapter
//! read runs, so a typo'd run id fails the pass rather than stamping an
//! attribution that silently joins to nothing.
//!
//! Attribution is outside BOTH write-identity digests
//! ([`trajectory_content_digest`], [`trajectory_derivation_key`]) — it
//! describes the write, not what the trajectory says, and folding it in
//! would break the idempotence this module's own contract rests on.
//! [`plan_trajectory`] states that in full.

use std::collections::BTreeMap;
use std::path::Path;

use canon_ingest::artifact_adapter::{ArtifactEvent, ArtifactJoinKey, ArtifactSourceConfig, ArtifactSourceHandle};
use canon_ingest::artifact_registry::{ArtifactDispatchOutcome, ArtifactSourceKind};
use canon_ingest::normalize::content_digest;
use canon_ingest::verdict::{VerdictRow, attach_regime_key, derive_native_divergence_verdict, derive_native_review_verdict, derive_verdict};
use canon_learn::{
    LearnConfig, LearnError, ParquetStrategyStore, ParquetTrajectoryStore, RewardRegistry, RoleRegistry, StrategyItem, StrategyStore, Trajectory,
    TrajectoryId, TrajectoryStore, TrajectoryVerdict, VerdictOutcome, distill_namespace, mark_trajectory_verdict, store_trajectory,
};
use canon_model::envelope::RecordKind;
use canon_model::evidence::RawRecord;
use canon_model::ids::{ProjectId, RegimeKey, RunId};
use canon_model::records::{DivergenceStatus, RunStatus};
use canon_store::fold_latest_by_key;
use canon_store::policy::{BackendConfig, Rung, TierPolicy};
use canon_store::registry::TierRegistry;
use canon_store::tier::{StoreError, TierQuery};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::context::resolve_repo_root;
use crate::dispatch::DispatchError;
use crate::tiers::{self, TierCliError};

#[derive(Debug, thiserror::Error)]
pub enum ArtifactIngestError {
    #[error(transparent)]
    Learn(#[from] LearnError),
    #[error(transparent)]
    RegimeKey(#[from] canon_model::ids::JoinKeyError),
    /// s29 design D6: the up-front kind-scoped lenient tier build
    /// failed with something OTHER than "canon.yaml missing/
    /// unreadable" (handled as a graceful per-adapter degrade, see the
    /// module doc's "documented seam") — a genuinely malformed config
    /// (bad YAML/policy syntax, an invalid pg schema, a non-forward
    /// aging rule, …), surfaced by `main.rs` as a nonzero exit exactly
    /// like `Learn`/`RegimeKey`.
    #[error(transparent)]
    Tiers(#[from] TierCliError),
    /// `artifacts.native_records: true` configured together with ANY
    /// raw-artifact path field (`ledger_root`/`divergences_root`/
    /// `openspec_root`) — design D7's XOR, rejected by
    /// [`validate_artifact_source_config`] BEFORE any adapter read
    /// runs (spec `native-record-flywheel` Requirement 3), surfaced by
    /// `main.rs` as a nonzero exit exactly like `Learn`/`RegimeKey`.
    #[error("canon.yaml artifacts config: {0}")]
    ConfigXor(String),
    /// `--run <RunId>` (s42 (`close-the-open-loops`) task 3.2) names a
    /// run this repo has no dispatch manifest for — rejected by
    /// [`resolve_dispatched_run`] BEFORE any adapter read runs.
    ///
    /// The same admission bar, against the same path, that `canon
    /// dispatch end` applies (`crate::dispatch::DispatchError::
    /// NoSuchRun`): only a run this repo actually dispatched can be
    /// attributed. Accepting a well-formed-but-unknown ULID would let a
    /// typo stamp every trajectory of the pass with an attribution
    /// pointing at nothing, which `mart_flywheel_funnel` would then
    /// silently drop on the join — a wrong claim that reads exactly like
    /// no claim, and the one failure mode this whole field exists to
    /// avoid.
    #[error(
        "--run `{run_id}` has no dispatch manifest at {path} — only a run begun by `canon dispatch begin` on this repo can be attributed; omit --run to ingest without attribution"
    )]
    UnknownRun { run_id: String, path: String },
    /// `--run <RunId>` names a manifest that exists but is not a `Run`
    /// record this driver can read — s42 (`close-the-open-loops`)
    /// re-review (`ReviewAttribution` finding 1). Covers every way
    /// `crate::dispatch::read_dispatched_manifest`'s first two rungs can
    /// fail: an entry at that path that is a DIRECTORY, a permission or
    /// I/O failure, and bytes that do not deserialize as a `Run`
    /// (truncated, hand-edited, or a foreign JSON document that merely
    /// happens to be named `<run_id>.json`).
    ///
    /// Deliberately NOT folded into [`ArtifactIngestError::UnknownRun`]:
    /// "this repo never began that run" and "the record of that run is
    /// damaged" are different operator problems with different fixes,
    /// and the pre-hardening code could not tell them apart at all
    /// because it probed with `metadata` and never opened the file.
    #[error(
        "--run `{run_id}`'s dispatch manifest at {path} is not a readable Run record ({detail}) — attribution is only ever taken from the manifest `canon dispatch begin` wrote; repair or remove it, or omit --run to ingest without attribution"
    )]
    UnreadableRunManifest { run_id: String, path: String, detail: String },
    /// `--run <RunId>` names a manifest whose OWN `run_id` is a
    /// different run — `crate::dispatch::read_dispatched_manifest`'s
    /// third rung, the same `DispatchError::RunIdMismatch` `canon
    /// dispatch end` refuses a close on.
    ///
    /// The filename is the dispatch side-channel's only index, so a
    /// copied or hand-renamed manifest would otherwise let one run's
    /// identity answer for another's — and here that answer is stamped
    /// onto EVERY trajectory the pass writes, which is a false
    /// attribution rather than a failed one.
    #[error(
        "--run `{run_id}`'s dispatch manifest at {path} records run_id `{found}` — the filename is that side-channel's only index, so admitting it would attribute every trajectory of this pass to a run you did not name"
    )]
    MisfiledRunManifest { run_id: String, path: String, found: String },
    /// `--run <RunId>` names a readable, correctly-filed manifest whose
    /// run was never DISPATCHED: `canon_model::records::RunStatus::
    /// Pending`, the one state that means no execution ever started.
    ///
    /// This driver's lifecycle rung is deliberately WEAKER than `canon
    /// dispatch end`'s, which admits only `(Running, ended_at: None)`.
    /// `end` MUTATES the run, so it needs the single closeable state;
    /// this pass only reads an attribution target, and a run that
    /// already closed (or was aborted) genuinely executed — trajectories
    /// derived inside it belong to it, and refusing them would make a
    /// wrapper that closes its run before a final ingest unable to
    /// attribute anything. `Pending` is the one status where attributing
    /// would assert an execution that never happened.
    #[error(
        "--run `{run_id}` records status `pending` at {path} — a pending run was never dispatched, so no execution exists for this pass's trajectories to belong to; begin the run with `canon dispatch begin`, or omit --run to ingest without attribution"
    )]
    RunNeverDispatched { run_id: String, path: String },
}

/// One registered adapter's contribution to this pass — mirrors
/// `crate::ingest::AdapterSummary`'s per-adapter shape, generalized
/// with a `status` field so a `Records`-source read failure is a
/// visible, distinct outcome (module doc's "documented seam"), never
/// collapsed into the same zero-events shape an unconfigured
/// `Path`-source adapter reports.
#[derive(Debug, Clone, Serialize)]
pub struct ArtifactAdapterSummary {
    pub adapter_id: &'static str,
    /// `"path"` | `"records"` (mirrors [`ArtifactSourceKind`]).
    pub source_kind: &'static str,
    /// `"read"` (this adapter's source was actually reached, whether
    /// or not it was configured/had records) | `"unavailable"` (a
    /// `Records`-source read failed before `parse` ever ran) |
    /// `"disabled"` (a `native_verdict: true` entry, S15 P4, whose
    /// `ArtifactSourceConfig::native_records` switch is off — never
    /// `"unavailable"`, which is reserved for a genuine read failure).
    pub status: &'static str,
    pub events_parsed: usize,
    pub malformed: usize,
    /// `Some(reason)` only when `status == "unavailable"`.
    pub unavailable_reason: Option<String>,
}

/// One regime-keyed trajectory this pass persisted.
#[derive(Debug, Clone, Serialize)]
pub struct PersistedTrajectory {
    pub regime_key: String,
    pub verdict_count: usize,
    /// `true` when this write REPLACED this driver's own earlier
    /// derivation of the same logical verdict set in place
    /// ([`plan_trajectory`]) rather than appending a new row — the
    /// upgrade path the module doc describes. Reported per row rather
    /// than as a second top-level counter, so the tally
    /// [`format_human`] prints can never desync from the rows it
    /// describes.
    pub superseded: bool,
}

/// One superseded copy the raw convergence KEPT rather than deleted,
/// because it carries an independently-written field the canonical row
/// does not (module doc, "The derivation key is NOT enough to authorize
/// that delete"). s42 (`close-the-open-loops`) re-review, `ReviewRuns`
/// finding 1.
///
/// A report, never a resolution. Both rows stay exactly as they are, so
/// `canon learn promote` keeps reading the contradiction off
/// `query_by_regime_key` — which is the whole point: the alternative
/// this replaced silently deleted the loser and changed what the gate
/// saw.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConvergenceConflict {
    /// The namespace both rows sit under.
    pub regime_key: String,
    /// The row the smallest-id rule keeps
    /// ([`canonical_driver_row`]) — the one whose provenance the kept
    /// row was compared against.
    pub canonical_id: String,
    /// The superseded row that survived the delete. Still withheld from
    /// the distiller exactly as s41 left it: kept is not served.
    pub retained_id: String,
    /// Every claim `retained_id` makes that `canonical_id` does not,
    /// one line per disagreeing field in [`RowProvenance`]'s fixed field
    /// order ([`RowProvenance::claims_beyond`]). Never empty — an empty
    /// list IS the deletable case, and no conflict is constructed for it.
    pub claims: Vec<String>,
}

/// One `canon ingest artifacts` pass's outcome.
#[derive(Debug, Clone, Serialize)]
pub struct ArtifactIngestOutcome {
    pub adapters: Vec<ArtifactAdapterSummary>,
    pub verdicts_derived: usize,
    pub trajectories_persisted: Vec<PersistedTrajectory>,
    /// A trajectory whose `regime_key` role is not registered in this
    /// repo's `canon_learn::RoleRegistry` (`canon.yaml` `learn.roles`,
    /// or the built-in set) — skipped and counted (module doc's
    /// "documented seam"), never a fatal error for the rest of the
    /// batch.
    pub trajectories_skipped_unregistered_role: usize,
    /// A trajectory whose FULL identity (identity-shape version +
    /// `regime_key` + ordered `VerdictRow` contents + the rendered
    /// `task`/`context`) already matches a trajectory this exact
    /// `regime_key` holds ([`trajectory_content_digest`]) — skipped and
    /// counted (module doc's write-time idempotence), never a
    /// double-write of an unchanged corpus. A row whose verdict set
    /// matches but whose TEXT does not is not this: it is superseded in
    /// place, see [`PersistedTrajectory::superseded`].
    pub trajectories_skipped_duplicate: usize,
    /// Raw `Trajectory` rows this pass DELETED as this driver's own
    /// superseded copies whose independently-written provenance the
    /// canonical row also carries ([`RawConvergence::deletable`]) — s42
    /// (`close-the-open-loops`).
    ///
    /// Reported rather than left silent, on the same "reported, never
    /// silent" discipline every other degrade and skip in this outcome
    /// follows. It is the only counter here that names bytes that no
    /// longer exist, so an operator seeing it non-zero should read the
    /// module doc's account of what a deleted row makes unresolvable. It
    /// is non-zero at most ONCE per namespace per upgrade: a namespace a
    /// post-s38 canon produced on its own has no superseded copies to
    /// remove, and one converging pass empties an upgraded one of every
    /// copy it is allowed to remove — the rest are reported in
    /// [`ArtifactIngestOutcome::convergence_conflicts`] and never
    /// counted here.
    pub trajectories_converged_deleted: usize,
    /// Superseded copies this pass REFUSED to delete because they carry
    /// a `verdict_record` or `run_id` the canonical row does not — s42
    /// (`close-the-open-loops`) re-review (`ReviewRuns` finding 1). One
    /// entry per kept row, ordered by regime key then by the kept row's
    /// own id, so a repeat pass prints byte-identical lines.
    ///
    /// Empty for every store this driver produced on its own, and for
    /// every ordinary upgrade: the losing copies a pre-s38 pass appended
    /// were never marked and never attributed, so they claim nothing.
    /// A non-empty entry means two stored rows disagree about a fact
    /// `canon learn promote` reads (module doc), and the disagreement is
    /// STANDING — it is re-reported every pass until an operator removes
    /// the row they judge wrong, because this driver will not pick for
    /// them.
    pub convergence_conflicts: Vec<ConvergenceConflict>,
    /// Sum of every regime's freshly-distilled `StrategyItem` count
    /// ([`NamespaceConvergence::items`]) — the count that actually lands
    /// in `stg_strategy_items`, i.e. what makes S9's `mart_role_memory`
    /// non-empty.
    pub strategy_items_rebuilt: usize,
    /// Trajectories whose covering verdict+reward was RESOLVED and
    /// written back in this same pass
    /// ([`canon_learn::RewardRegistry::compute_for_trajectory`] +
    /// [`canon_learn::mark_trajectory_verdict`]). Every freshly
    /// constructed `Trajectory` starts `VerdictOutcome::Pending` (S7
    /// design D2's two-phase reward-write model), and S7's promotion
    /// gates read `verdict_record.outcome`, NOT the raw `verdicts`
    /// list — so a trajectory left `Pending` is invisible to
    /// `OccurrencePromotionGate`/`CrnPromotionGate` and can never
    /// corroborate a promotion. This counter is what makes the
    /// artifact-ingest half of the flywheel observable.
    pub trajectories_marked: usize,
    /// Trajectories deliberately LEFT `Pending`: the role's own
    /// `RewardFn` resolved to [`VerdictOutcome::Pending`], which
    /// `mark_trajectory_verdict` rejects outright
    /// ([`canon_learn::LearnError::CannotMarkVerdictPending`]) because
    /// `Pending` is the unset default, never a covering-verdict write.
    /// The `dev` role reaches this legitimately: `compute_dev_reward`'s
    /// additive triad stays `Pending` below a full `1.0` (a partial
    /// `pr-merged` + `ci-pass` without `no-rollback`), waiting for the
    /// S7 webhook receiver's no-rollback timer to resolve it. Counted,
    /// never an error.
    pub trajectories_left_pending: usize,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ArtifactsSectionManifest {
    #[serde(default)]
    artifacts: ArtifactSourceConfig,
}

/// Parse `canon.yaml`'s `artifacts:` top-level section into an
/// [`ArtifactSourceConfig`] — the CLI-local wiring
/// `crate::artifact_adapter`'s own doc comment names as the future step
/// ("a future `serde_yaml::from_str::<ArtifactSourceConfig>` ... needs
/// no bespoke parser"); this IS that wiring, mirroring
/// `crate::tiers::build_tiers`'s "wire canon.yaml to a live type" role
/// for `canon-store`'s `TierPolicy`. A missing `artifacts:` section, or
/// an unreadable/unparseable `canon.yaml`, degrades to
/// `ArtifactSourceConfig::default()` (every field `None`, no source
/// scanned) rather than an error — matches every field's own "an
/// unconfigured source is never scanned" contract, and
/// `crate::retrieve::open_strategy_store`'s identical degrade-to-
/// default posture for this same file. Every configured path is
/// resolved relative to `repo` (never the process CWD), mirroring
/// `build_tiers`'s `tiers.git.root` resolution.
fn load_artifact_source_config(repo: &Path, canon_yaml_text: &str) -> ArtifactSourceConfig {
    let parsed = serde_yaml::from_str::<ArtifactsSectionManifest>(canon_yaml_text).map(|m| m.artifacts).unwrap_or_default();
    ArtifactSourceConfig {
        ledger_root: parsed.ledger_root.map(|p| repo.join(p)),
        divergences_root: parsed.divergences_root.map(|p| repo.join(p)),
        openspec_root: parsed.openspec_root.map(|p| repo.join(p)),
        native_records: parsed.native_records,
    }
}

/// The design D7 XOR: `native_records: true` together with ANY
/// raw-artifact path field is rejected BEFORE any adapter read runs
/// (spec `native-record-flywheel` Requirement 3 — the raw and native
/// paths' verdict rows differ slightly, so
/// [`trajectory_content_digest`] would not dedupe them, silently
/// double-counting the same underlying evidence). [`run`] calls this
/// immediately after [`load_artifact_source_config`], before touching
/// any adapter.
fn validate_artifact_source_config(config: &ArtifactSourceConfig) -> Result<(), ArtifactIngestError> {
    if config.native_records && (config.ledger_root.is_some() || config.divergences_root.is_some() || config.openspec_root.is_some()) {
        return Err(ArtifactIngestError::ConfigXor(
            "artifacts.native_records: true is XOR-exclusive with ledger_root/divergences_root/openspec_root — set at most one of them".to_string(),
        ));
    }
    Ok(())
}

/// The `canon-store` `RecordKind` a `Records`-source adapter reads —
/// `handoff` (S4), `review`/`divergence-native` (S15 P4). A future
/// `Records`-source adapter this registry gains without a matching arm
/// here reports itself `"unavailable"` with an explicit reason (never
/// a silent zero), so growing the registry can never regress into the
/// same silent-drop `ArtifactDispatchOutcome::UnsupportedSource`
/// prevents one layer down.
fn record_kind_for_records_adapter(adapter_id: &str) -> Result<RecordKind, String> {
    match adapter_id {
        "handoff" => Ok(RecordKind::Handoff),
        "review" => Ok(RecordKind::Review),
        "divergence-native" => Ok(RecordKind::Divergence),
        other => Err(format!("no canon-store RecordKind mapping registered in canon-cli for records-source adapter `{other}`")),
    }
}

/// Reads every `RawRecord` of `adapter_id`'s mapped `RecordKind` off
/// `store`/`policy` -- a [`canon_store::registry::TierRegistry`] +
/// its own [`canon_store::policy::TierPolicy`] built ONCE, up front,
/// by [`run`]'s own kind-scoped [`crate::tiers::build_lenient_tiers_for_kinds`]
/// call (s29 design D6) -- the module doc's "records-source adapters
/// ... fed by THIS canon-cli driver" step. `Err` (a `String` reason,
/// never a panic) when `store` is `None` (`canon.yaml` missing/
/// unreadable -- `missing_config_reason` names why), this adapter has
/// no `RecordKind` mapping, or the mapped kind's routed rung is
/// unrouted/unattached -- the caller reports this as `status:
/// "unavailable"` rather than a fatal whole-pass error.
///
/// `unavailable_reasons` (from the SAME up-front build) is checked
/// BEFORE `TierRegistry::query` runs: a routed rung that was
/// ATTEMPTED and degraded carries its build-time reason (the
/// configured `dsn_env`/`bucket_env` name) there, which is a more
/// SPECIFIC message than `TierRegistry::query`'s own generic
/// `Backend::default_unattached_reason()` fallback (s29 design D6) --
/// reusing `StoreError::tier_unavailable`'s canonical Display so the
/// wording matches the rest of the codebase. An UNROUTED kind (no
/// entry in `unavailable_reasons` at all, since the build step never
/// attempted a rung for it) falls through to `TierRegistry::query`'s
/// own `StoreError::UnroutedKind`, unchanged.
///
/// s21 P4 (design.md D5): `handoff` is routed to `PgTier`, whose `read`
/// now returns every retained historical version (s21 P3), not one row
/// per `handoff_id` — folded here, BEFORE `HandoffAdapter::parse` ever
/// sees the records, via the SAME shared `fold_latest_by_key` every
/// other multi-version reader applies, so `HandoffAdapter`'s own
/// idempotence contract ("one snapshot, several transitions" — its
/// module doc) keeps holding: it still receives exactly one CURRENT row
/// per `handoff_id`, never N historical rows misread as N independent
/// current ones. `review`/`divergence-native` are intentionally NOT
/// folded — both are git-routed, multi-row-per-key BY DESIGN (S15 P4's
/// native-verdict contract), the opposite of `handoff`'s contract.
fn read_records_for(
    adapter_id: &str,
    store: Option<&TierRegistry>,
    policy: Option<&TierPolicy>,
    unavailable_reasons: &BTreeMap<Rung, String>,
    missing_config_reason: Option<&str>,
) -> Result<Vec<RawRecord>, String> {
    let kind = record_kind_for_records_adapter(adapter_id)?;
    let records = read_kind_records(kind, store, policy, unavailable_reasons, missing_config_reason)?;
    Ok(if adapter_id == "handoff" { fold_handoff_records(kind, records) } else { records })
}

/// The kind-level read core [`read_records_for`] is a thin,
/// adapter-keyed wrapper around — extracted by s39
/// (`joined-evidence-grounding`) so its second reader, the
/// [`ScenarioTitleIndex`] join, reuses the EXACT same read path
/// (up-front `unavailable_reasons` precheck, then
/// [`TierRegistry::query`]) instead of growing a second one beside it
/// with its own subtly different degrade rules. Every mechanic and
/// every failure mode is documented on [`read_records_for`] above; the
/// only thing that moved down here is the part that never depended on
/// an `adapter_id` at all.
fn read_kind_records(
    kind: RecordKind,
    store: Option<&TierRegistry>,
    policy: Option<&TierPolicy>,
    unavailable_reasons: &BTreeMap<Rung, String>,
    missing_config_reason: Option<&str>,
) -> Result<Vec<RawRecord>, String> {
    let Some(store) = store else {
        return Err(missing_config_reason.map(str::to_string).unwrap_or_else(|| "canon.yaml is missing or unreadable — no live tiers configured".to_string()));
    };
    let policy = policy.expect("policy is always Some whenever store is Some -- built together in run()");
    if let Ok(rung) = policy.tier_for(kind) {
        if let Some(reason) = unavailable_reasons.get(&rung) {
            let backend = policy.tiers.get(&rung).map(BackendConfig::backend);
            return Err(StoreError::tier_unavailable(rung, backend, reason.clone()).to_string());
        }
    }
    store.query(&TierQuery::kind(kind)).map(|result| result.records).map_err(|e| e.to_string())
}

/// The `handoff`-only fold [`read_records_for`] applies — see its own
/// doc comment. Kept as a small, separately-named function (mirroring
/// `canon-cli::query::fold_pg_routed_kind`'s identical shape) rather
/// than inlined, so the ONE adapter this applies to stays visibly
/// distinct from `review`/`divergence-native`, which never call it.
fn fold_handoff_records(kind: RecordKind, records: Vec<RawRecord>) -> Vec<RawRecord> {
    struct Candidate {
        key: String,
        at: DateTime<Utc>,
        // s38 (`evidence-bearing-memory`): the record's own format
        // generation, the rung `fold_latest_by_key` now compares between
        // `at` and `digest`. Read through `raw_record_schema` rather than
        // a typed envelope because these candidates are bare `RawRecord`
        // JSON at this point; a malformed record missing `schema`
        // degrades to `0`, which sorts strictly below every real
        // generation (kinds start at `1`), so it can never out-rank a
        // well-formed row on a tie.
        schema: u32,
        digest: String,
        record: RawRecord,
    }
    let candidates = records.into_iter().map(|record| {
        let key = canon_store::partition::resolve_partition(kind, &record.0).map(|p| p.natural_key).unwrap_or_default();
        let at = canon_store::tier::raw_record_at(&record);
        let schema = canon_store::tier::raw_record_schema(&record);
        let digest = canon_store::partition::content_digest12(&record.0);
        Candidate { key, at, schema, digest, record }
    });
    fold_latest_by_key(candidates, |c| c.key.clone(), |c| c.at, |c| c.schema, |c| c.digest.as_str()).into_values().map(|c| c.record).collect()
}

/// The char cap [`ScenarioTitleIndex`] applies to an indexed scenario
/// title (s39 `joined-evidence-grounding`), mirroring the role
/// `canon_ingest::artifact_adapter`'s own `EVIDENCE_TEXT_MAX_CHARS`
/// plays for mined `detail` prose: a joined title lands in a
/// trajectory's `task`, which becomes a distilled
/// `canon_learn::StrategyItem`'s TITLE, and several of those are
/// injected into a dispatched agent's context at once — so one
/// pathological record must not be able to bloat every retrieval that
/// touches its regime.
///
/// 160 is measured, not guessed: the 16 `Scenario` records in this
/// repo's own `.canon/ledger/kind=scenario` carry titles of 33–63 chars
/// (one Gherkin `Scenario:` line), so 160 leaves every genuine title
/// INTACT with better than 2x headroom while bounding a pasted
/// paragraph to roughly two terminal lines. Deliberately far tighter
/// than the 512 chars evidence prose gets: this is a TITLE, and the
/// full narrative already has a home in `context`.
const SCENARIO_TITLE_MAX_CHARS: usize = 160;

/// `(project_id, scenario_id) -> title`, read ONCE per `canon ingest
/// artifacts` pass off the `Scenario` ledger index (s39
/// `joined-evidence-grounding`).
///
/// This exists because canon's join spine means a record does not have
/// to carry every fact about itself: `canon_model::records::Review` is
/// exactly `{envelope, project_id, scenario_id, reviewer, pin,
/// provenance_ref}` — genuinely no prose field — while `Scenario`
/// carries a real sentence in `title`. A review attests
/// `(project_id, scenario_id)`, so joining that pair is what lets its
/// trajectory say WHAT was attested instead of only naming the
/// scenario id and its pin sha.
///
/// Keyed scenario-id-OUTER / project-id-inner, which looks inverted
/// against the `(project_id, scenario_id)` join pair and is deliberate:
/// an [`ArtifactEvent`]'s `join_key` carries the scenario id ALONE
/// (`ArtifactJoinKey::Scenario`), and its `project_id` is only
/// recoverable from adapter-emitted `detail` ([`event_project_id`]),
/// which not every adapter populates. Scenario-first therefore makes
/// the always-available half the lookup's first level, and keeps the
/// project level a real part of the key rather than dropping it: a
/// scenario id that two projects both define resolves only when the
/// event names its project, and otherwise contributes NO title (see
/// [`Self::title_for`]) instead of printing another project's sentence.
///
/// `BTreeMap` at both levels, so the index — and anything derived from
/// it — iterates in one total, data-derived order regardless of the
/// order the tier handed the records over.
#[derive(Debug, Default)]
struct ScenarioTitleIndex {
    titles: BTreeMap<String, BTreeMap<String, IndexedScenarioTitle>>,
}

/// One indexed title plus the `(at, schema, digest)` triple that earned
/// it its slot — see [`ScenarioTitleIndex::absorb_record`]'s
/// supersession rule.
#[derive(Debug)]
struct IndexedScenarioTitle {
    at: DateTime<Utc>,
    schema: u32,
    digest: String,
    title: String,
}

impl IndexedScenarioTitle {
    /// The total supersession order — the SAME `(at, schema, digest)`
    /// triple `canon_store::fold_latest_by_key` compares, spelled out
    /// here because this index folds into a two-level `BTreeMap` slot
    /// rather than that helper's flat one-winner-per-key map.
    fn order(&self) -> (DateTime<Utc>, u32, &str) {
        (self.at, self.schema, self.digest.as_str())
    }
}

impl ScenarioTitleIndex {
    /// Indexes every well-formed `Scenario` record, skipping the rest.
    ///
    /// A record missing `scenario_id`/`project_id`/`title`, or carrying
    /// a blank one, contributes NO entry — a strategy title must never
    /// be enriched with an empty sentence, and a partially-formed record
    /// is not evidence (the same "malformed evidence is no evidence"
    /// discipline `canon_ingest::ArtifactParseOutcome` applies one layer
    /// out). Reads the fields off the bare `RawRecord` JSON rather than
    /// deserializing `canon_model::records::Scenario`, so one
    /// unparseable record — or a future schema generation this build
    /// does not know — degrades to a missing title rather than aborting
    /// the whole index.
    fn from_records(records: &[RawRecord]) -> Self {
        let mut index = Self::default();
        for record in records {
            index.absorb_record(record);
        }
        index
    }

    /// Folds one record in, the greatest `(at, schema, digest)` triple
    /// winning its `(project_id, scenario_id)` slot.
    ///
    /// The ledger's own file naming (`<project>__<scenario>__<digest>`)
    /// means a RETITLED scenario lands as a SECOND record beside its
    /// predecessor rather than replacing it, and a `LiveDb`-class rung
    /// retains historical versions outright (s21 P3) — so "which title
    /// is current" is a real question, not a hypothetical. Determinism
    /// is load-bearing: the title reaches `Trajectory::task`, and the
    /// write-time idempotence skip only fires when a second pass derives
    /// byte-identical text.
    ///
    /// `at` ALONE does not answer it. A ledger record's `at` is
    /// deliberately byte-stable rather than wall-clock, so two versions
    /// of one `(project_id, scenario_id)` routinely carry the IDENTICAL
    /// timestamp — and a strictly-greater-`at` rule then leaves the slot
    /// to whichever version the tier happened to return first, which for
    /// a live database read is unordered. This is precisely the
    /// supersession case `s38-evidence-bearing-memory` fixed for every
    /// other multi-version reader, so it takes the same triple:
    /// `Envelope.schema` (the per-kind FORMAT GENERATION, so the newer
    /// parser's row wins a tie by stating so) between `at` and the
    /// content `digest`, which remains the final tie-break. All three
    /// rungs are the record's OWN data, so the resolved title is a pure
    /// function of the input SET (ReviewCore).
    ///
    /// Reads that timestamp through [`raw_record_at_or_min`] rather than
    /// `canon_store::tier::raw_record_at`, which `expect`s a
    /// well-formed `at` and would PANIC on a hand-edited ledger file —
    /// unacceptable for a join whose whole contract is that it can only
    /// ever fail to enrich, never fail a run. `raw_record_schema`
    /// already takes that posture for ITS field (a missing/invalid
    /// `schema` floors to `0`, below every real generation).
    fn absorb_record(&mut self, record: &RawRecord) {
        let (Some(scenario_id), Some(project_id), Some(title)) =
            (raw_field(record, "scenario_id"), raw_field(record, "project_id"), raw_field(record, "title").map(compact_scenario_title))
        else {
            return;
        };
        let candidate = IndexedScenarioTitle {
            at: raw_record_at_or_min(record),
            schema: canon_store::tier::raw_record_schema(record),
            digest: canon_store::partition::content_digest12(&record.0),
            title,
        };
        match self.titles.entry(scenario_id.to_string()).or_default().entry(project_id.to_string()) {
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(candidate);
            }
            std::collections::btree_map::Entry::Occupied(mut slot) if slot.get().order() < candidate.order() => {
                slot.insert(candidate);
            }
            std::collections::btree_map::Entry::Occupied(_) => {}
        }
    }

    /// The title `join_key` names, or `None` — which is the ONLY
    /// outcome for a non-scenario join key (a `handoff:`/`task:` id has
    /// no scenario to name), an id this index never saw, or an id two
    /// projects both define that `project_id` does not disambiguate.
    ///
    /// `None` is not a degraded rendering, it is the s38 rendering:
    /// [`RegimeEvidence::trajectory_text`] then emits its pre-s39 `task`
    /// byte for byte, so this whole join is purely additive.
    fn title_for(&self, join_key: &ArtifactJoinKey, project_id: Option<&str>) -> Option<&str> {
        let ArtifactJoinKey::Scenario(scenario_id) = join_key else { return None };
        let by_project = self.titles.get(scenario_id.as_str())?;
        let indexed = match project_id {
            Some(project) => by_project.get(project)?,
            // Exactly one project defines this scenario id, so naming it
            // is unambiguous even though the event never said which
            // project it belongs to. Two or more and there is no honest
            // answer: pick nothing rather than another project's
            // sentence.
            None if by_project.len() == 1 => by_project.values().next()?,
            None => return None,
        };
        Some(indexed.title.as_str())
    }
}

/// One trimmed, non-blank string field off a bare `RawRecord`'s JSON —
/// the shape [`ScenarioTitleIndex::absorb_record`] needs for all three
/// of its fields, where a present-but-blank value must read exactly like
/// an absent one.
fn raw_field<'a>(record: &'a RawRecord, field: &str) -> Option<&'a str> {
    record.0.get(field)?.as_str().map(str::trim).filter(|value| !value.is_empty())
}

/// A bare `RawRecord`'s `at`, or `DateTime::<Utc>::MIN_UTC` when it is
/// absent or not RFC-3339 — the first rung of the `(at, schema,
/// digest)` triple [`ScenarioTitleIndex::absorb_record`] compares.
///
/// `canon_store::tier::raw_record_at` is the shared accessor for this
/// field, but it `expect`s the parse to succeed ("already passed
/// validate_envelope_shape") and therefore PANICS on a record that
/// reached this index without that guarantee — a hand-edited ledger
/// file is enough. The scenario-title join must degrade, never abort a
/// `canon ingest artifacts` run, so it takes the same posture
/// `canon_store::tier::raw_record_schema` already takes for ITS field:
/// a malformed value falls back to a floor that sorts strictly below
/// every well-formed record, so it can never out-rank one.
fn raw_record_at_or_min(record: &RawRecord) -> DateTime<Utc> {
    record
        .0
        .get("at")
        .and_then(|value| value.as_str())
        .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
        .map(|at| at.with_timezone(&Utc))
        .unwrap_or(DateTime::<Utc>::MIN_UTC)
}

/// Collapses whitespace runs to single spaces, trims, and caps at
/// [`SCENARIO_TITLE_MAX_CHARS`] CHARS (never bytes — canon's corpora
/// carry Korean prose and a byte cut would split a codepoint), marking a
/// cut with a trailing `…`.
///
/// Deliberately a canon-cli-local twin of
/// `canon_ingest::artifact_adapter`'s private `compact_evidence_text`
/// rather than a new public export from that crate: the two caps answer
/// different questions (a strategy TITLE versus its CONTENT, see
/// [`SCENARIO_TITLE_MAX_CHARS`]), and widening `canon-ingest`'s API to
/// share fifteen lines would couple them into moving together. The twin
/// tracks its original in shape too: it appends only THROUGH the budget
/// instead of normalizing the whole field first, so an artifact-supplied
/// title of unbounded size costs no copy of its discarded tail
/// (ReviewCore).
fn compact_scenario_title(raw: &str) -> String {
    // At most `SCENARIO_TITLE_MAX_CHARS` chars (4 bytes each at worst)
    // plus the 3-byte cut marker can ever land here, so the reserve is
    // an upper bound and no growth reallocation happens.
    let mut out = String::with_capacity(raw.len().min(SCENARIO_TITLE_MAX_CHARS * 4 + 4));
    let mut taken = 0usize;
    let mut cut = false;
    for word in raw.split_whitespace() {
        if !out.is_empty() {
            if taken == SCENARIO_TITLE_MAX_CHARS {
                cut = true;
                break;
            }
            out.push(' ');
            taken += 1;
        }
        for c in word.chars() {
            if taken == SCENARIO_TITLE_MAX_CHARS {
                cut = true;
                break;
            }
            out.push(c);
            taken += 1;
        }
        if cut {
            break;
        }
    }
    if cut {
        // A separator pushed just before the budget ran out is the one
        // way `out` can end in whitespace; `…` must never follow it.
        let trimmed = out.trim_end().len();
        out.truncate(trimmed);
        out.push('…');
    }
    out
}

/// The `project_id` an event's own adapter recorded in `detail`, if any
/// — the second half of [`ScenarioTitleIndex`]'s join key (s39
/// `joined-evidence-grounding`).
///
/// The native `review`/`divergence-native` adapters emit it explicitly,
/// and the raw `ledger`/`divergence` adapters copy their source JSON
/// verbatim, so it is usually present; `None` when it is not, which
/// [`ScenarioTitleIndex::title_for`] resolves through its
/// single-project fallback rather than failing. Read from `detail` —
/// artifact-supplied content — is safe here precisely because a wrong
/// value can only ever cost a title lookup a hit: unlike
/// `derive_verdict_for_event`'s `adapter_id` dispatch, nothing routes on
/// it, and no verdict, reward, or trajectory identity depends on it.
fn event_project_id(event: &ArtifactEvent) -> Option<&str> {
    event.detail.get("project_id")?.as_str().map(str::trim).filter(|value| !value.is_empty())
}

/// The resolved repo's `regime_key` `<repo>` segment — its directory
/// basename, canonicalized downstream by `canon_model::ids::regime_key`
/// itself (lowercased, whitespace/`/` collapsed to `-`), so this needs
/// no normalization of its own.
fn repo_label(repo: &Path) -> String {
    repo.file_name().and_then(|s| s.to_str()).unwrap_or("repo").to_string()
}

/// The token an event identity uses in place of a scenario's
/// `project_id` when the event does not name one, or names one that is
/// not a well-formed [`ProjectId`] (ReviewCore).
///
/// `-` is unreachable as a real project id by grammar, not by
/// convention: `ProjectId` is `[a-z0-9][a-z0-9-]*`, so its FIRST
/// character can never be a hyphen. A missing project therefore cannot
/// silently collide with a real one.
const UNKNOWN_PROJECT: &str = "-";

/// A stable, source-kind-tagged identity string for one
/// [`ArtifactJoinKey`] plus — for a `Scenario` key — the `project_id`
/// of the event that carried it. This is the input to [`regime_hash`]'s
/// digest and the [`AntecedentIndex`]'s bucket key; it is never itself
/// the `regime_key` hash (module doc: two events sharing an identity
/// fold onto the SAME trajectory).
///
/// The project half is REQUIRED for correctness, not enrichment
/// (ReviewCore). A `Scenario`'s identity in canon is the PAIR
/// `(project_id, scenario_id)` — `Review`, `Divergence`, and this
/// index's own two-level key all say so, and this module already
/// supports one scenario id being defined by several projects
/// ([`ScenarioTitleIndex::title_for`]). Keyed on the bare scenario id,
/// project A's `open` finding attached itself to project B's later
/// `resolved` verdict, and `regime_hash` folded both projects' events
/// onto ONE regime — distilling a claim neither corpus supports.
///
/// Exact forms, and why they cannot be confused with each other:
/// - `scenario:<scenario_id>@<project_id>`
/// - `scenario:<scenario_id>@-` — no project, or one that is not a
///   well-formed [`ProjectId`] ([`UNKNOWN_PROJECT`])
/// - `handoff:<handoff_id>` / `task:<task_id>` — neither key names a
///   project, so neither form changes
///
/// `project_id` reaches here from adapter-emitted `detail`
/// ([`event_project_id`]), i.e. artifact-supplied text of arbitrary
/// shape, so it is admitted only after [`ProjectId::parse`] accepts it.
/// That is what makes the `@` split total: `ScenarioId`'s grammar
/// (`[a-z0-9-]+\.[a-z0-9-]+\.\d{2,}`) contains no `@`, and a parsed
/// `ProjectId` contains none either, so no pair of distinct
/// `(project, scenario)` inputs can render the same string. A malformed
/// project id reads as UNKNOWN rather than being pasted in verbatim:
/// malformed evidence is no evidence, and the cost of the stricter
/// reading is a missed fold, never a wrong one. (The title lookup keeps
/// the raw string — a wrong title costs prose, a wrong identity costs a
/// join.)
fn join_key_identity(key: &ArtifactJoinKey, project_id: Option<&str>) -> String {
    match key {
        ArtifactJoinKey::Scenario(id) => {
            let parsed = project_id.and_then(|raw| ProjectId::parse(raw).ok());
            let project = parsed.as_ref().map_or(UNKNOWN_PROJECT, ProjectId::as_str);
            format!("scenario:{}@{project}", id.as_str())
        }
        ArtifactJoinKey::Handoff(id) => format!("handoff:{}", id.as_str()),
        ArtifactJoinKey::Task(id) => format!("task:{}", id.as_str()),
    }
}

/// ONE event's [`join_key_identity`] — the single place both the
/// antecedent lookup and [`regime_hash`] resolve an event to its
/// identity string, so the two can never be computed differently
/// (ReviewCore).
fn event_identity(event: &ArtifactEvent) -> String {
    join_key_identity(&event.join_key, event_project_id(event))
}

/// `regime_key`'s `<hash>` component (module doc): reuses S3's
/// `content_digest` primitive over an [`event_identity`] string, never
/// a new hashing scheme.
fn regime_hash(identity: &str) -> String {
    content_digest(&serde_json::json!(identity))
}

/// The generation of [`trajectory_content_digest`]'s input SHAPE.
///
/// `1` was the implicit pre-s38 shape (`regime_key` + ordered verdict
/// rows only). `2` adds the rendered `task`/`context`. Carried in the
/// digest input rather than left implicit so a future change to what
/// "the same trajectory" means is a deliberate, greppable bump that
/// reconverges every stored row exactly once — instead of depending on
/// the new renderer happening to emit different bytes for every corpus.
const TRAJECTORY_IDENTITY_VERSION: u32 = 2;

/// The tag every trajectory THIS driver writes carries, and the marker
/// [`plan_trajectory`] uses to scope supersession to rows it is
/// entitled to replace: a trajectory seeded by a fixture, a webhook, or
/// any future writer is never overwritten by an artifact-ingest pass,
/// however closely its verdict rows match.
const ARTIFACT_INGEST_TAG: &str = "artifact-ingest";

/// The ordered `VerdictRow` contents both digests below fold — the only
/// three fields the type carries.
fn verdict_rows_json(verdicts: &[VerdictRow]) -> Vec<serde_json::Value> {
    verdicts.iter().map(|v| serde_json::json!({"role": v.role.as_str(), "polarity": v.polarity.as_str(), "becomes": v.becomes.as_str()})).collect()
}

/// The LOGICAL verdict set a trajectory records: `regime_key` + the
/// ORDERED `VerdictRow` contents. NOT an identity — two trajectories
/// sharing this key are two derivations of the same question, whose
/// answers may differ in TEXT ([`trajectory_content_digest`] is what
/// tells those apart).
///
/// [`plan_trajectory`] uses it to find the row a rebuild supersedes.
/// Deliberately UNVERSIONED, and deliberately byte-identical to the
/// pre-s38 digest's input shape: it is computed from a STORED row's own
/// `regime_key`/`verdicts` at read time, and its whole job is to
/// recognize a row written by an older canon. Changing this shape would
/// orphan every trajectory an upgrading user already holds.
fn trajectory_derivation_key(regime_key: &RegimeKey, verdicts: &[VerdictRow]) -> String {
    content_digest(&serde_json::json!({"regime_key": regime_key.as_str(), "verdicts": verdict_rows_json(verdicts)}))
}

/// The FULL identity of one regime-keyed trajectory — the identity
/// shape version, `regime_key`, the ordered `VerdictRow` contents, and
/// the RENDERED `task`/`context` — reusing the SAME [`content_digest`]
/// primitive [`regime_hash`] already uses (not a new hashing scheme).
///
/// The rendered text is in here because it is what a trajectory
/// actually SAYS, and what `canon_learn::distill_trajectory` turns into
/// a retrieved strategy's title and content. An identity that ignored
/// it (the pre-s38 shape) declared a stored row "already persisted"
/// while this pass had derived materially different prose for it —
/// which is exactly how an upgrade silently kept re-distilling
/// pre-s38 ingest-driver text (module doc, ReviewCore).
///
/// [`plan_trajectory`] calls this identically for a freshly-derived
/// candidate and for every already-persisted trajectory under the same
/// `regime_key`: an unchanged corpus re-derives the identical identity
/// (write-time idempotence), a changed corpus — or a changed RENDERER —
/// a different one.
fn trajectory_content_digest(regime_key: &RegimeKey, verdicts: &[VerdictRow], task: &str, context: &str) -> String {
    content_digest(&serde_json::json!({
        "identity_version": TRAJECTORY_IDENTITY_VERSION,
        "regime_key": regime_key.as_str(),
        "verdicts": verdict_rows_json(verdicts),
        "task": task,
        "context": context,
    }))
}

/// One derived verdict PLUS the salient evidence of the event it came
/// from (`s38-evidence-bearing-memory`).
///
/// [`run`]'s accumulator used to collapse to a bare
/// `(RegimeKey, VerdictRow, DateTime<Utc>)` tuple and drop the event on
/// the floor — so the only material left for the trajectory's
/// `task`/`context` (which become a distilled
/// `canon_learn::StrategyItem`'s TITLE and CONTENT, and from there the
/// guidance `canon retrieve` injects into a dispatched agent) was a
/// description of this driver's own plumbing. A named struct rather
/// than a wider tuple because six positional fields at one call site is
/// how the wrong two get swapped.
struct DerivedVerdict {
    regime_key: RegimeKey,
    row: VerdictRow,
    at: DateTime<Utc>,
    /// The event's [`ArtifactJoinKey::as_str`] — the CONCRETE artifact
    /// id (`platformer.hud.01`) the trajectory names.
    join_key: String,
    /// The joined `Scenario.title` for that id, when this pass's
    /// [`ScenarioTitleIndex`] holds one (s39 `joined-evidence-grounding`)
    /// — resolved in [`run`]'s accumulation loop, the one place that has
    /// both the index and the event's own `detail` `project_id` in hand.
    /// `None` for a non-scenario join key, an unindexed id, or an
    /// unreadable/unrouted scenario tier, and then the rendered `task`
    /// stays exactly what s38 produced.
    scenario_title: Option<String>,
    kind_label: &'static str,
    evidence_line: String,
    /// The prose of the non-verdict events that preceded this one on
    /// the same join key (s39 `joined-evidence-grounding`), already
    /// `(at, line)`-ordered and deduped by [`antecedents_before`].
    antecedents: Vec<String>,
}

impl DerivedVerdict {
    /// A TOTAL, data-derived order over every field of this struct that
    /// can reach persisted bytes or rendered text — the sort
    /// [`group_by_regime`] applies before any absorption runs
    /// (ReviewCore).
    ///
    /// `at` alone is not enough. A `Records`-source read orders rows by
    /// `at` only (that is all the Pg/SQLite read path asks for), and a
    /// ledger record's `at` is byte-stable rather than wall-clock, so
    /// two records for one regime routinely carry the IDENTICAL
    /// timestamp and can arrive in either order. Everything downstream
    /// is order-sensitive — `rows` is an ORDERED list inside
    /// [`trajectory_content_digest`], and `kind_labels`/`evidence_lines`
    /// render in first-seen order — so arrival order would decide the
    /// persisted bytes, and a repeat ingest of an unchanged record set
    /// would append another trajectory instead of skipping a duplicate.
    ///
    /// Spelled out rung by rung rather than returned as a wide tuple:
    /// the rungs' ORDER is the contract, and a ten-element positional
    /// key is exactly the shape [`DerivedVerdict`]'s own doc comment
    /// says this struct exists to avoid. Every rung is the verdict's own
    /// DATA, never its provenance, so the result is a pure function of
    /// the input SET (the discipline `canon_store::fold_latest_by_key`
    /// states for its own triple). The last rungs are total by
    /// construction: two verdicts equal through `antecedents` render
    /// byte-identically and dedupe into one line anyway.
    fn cmp_stable(&self, other: &Self) -> std::cmp::Ordering {
        self.at
            .cmp(&other.at)
            .then_with(|| self.regime_key.as_str().cmp(other.regime_key.as_str()))
            .then_with(|| self.join_key.cmp(&other.join_key))
            .then_with(|| self.row.role.as_str().cmp(other.row.role.as_str()))
            .then_with(|| self.row.polarity.as_str().cmp(other.row.polarity.as_str()))
            .then_with(|| self.row.becomes.as_str().cmp(other.row.becomes.as_str()))
            .then_with(|| self.kind_label.cmp(other.kind_label))
            .then_with(|| self.scenario_title.cmp(&other.scenario_title))
            .then_with(|| self.evidence_line.cmp(&other.evidence_line))
            .then_with(|| self.antecedents.cmp(&other.antecedents))
    }
}

/// One regime group's accumulated verdicts and evidence — the value
/// side of [`group_by_regime`]'s map (`s38-evidence-bearing-memory`).
struct RegimeEvidence {
    rows: Vec<VerdictRow>,
    /// The newest source-record timestamp in this group (the
    /// trajectory's `recorded_at`).
    latest_at: DateTime<Utc>,
    /// `Some(id)` while every absorbed verdict shared ONE join key —
    /// which is the invariant by construction, since `regime_key`'s
    /// `<hash>` component IS [`regime_hash`] of the join key, so one
    /// regime group can only ever hold one key. `None` is the honest
    /// degrade if that ever stops holding: [`Self::trajectory_text`]
    /// then names the regime key rather than picking one member's id
    /// and printing a wrong one.
    join_key: Option<String>,
    /// The joined `Scenario.title` this group's `join_key` names (s39
    /// `joined-evidence-grounding`) — the only field
    /// [`Self::trajectory_text`]'s `task` half gained since s38, and
    /// `None` whenever no title was joined, which renders the s38 text
    /// byte for byte.
    scenario_title: Option<String>,
    /// Deduped [`ArtifactEvent::display_label`]s in first-seen order —
    /// the SAME strings [`ArtifactEvent::evidence_line`] prefixes its
    /// lines with, so a trajectory's title and its content always name
    /// the same things.
    kind_labels: Vec<&'static str>,
    /// Deduped [`ArtifactEvent::evidence_line`]s in first-seen order.
    evidence_lines: Vec<String>,
    /// The antecedent findings this regime's verdicts absorbed (s39
    /// `joined-evidence-grounding`) — deduped, in the `(at, line)` order
    /// [`antecedents_before`] hands them over, and capped by
    /// [`MAX_ANTECEDENT_LINES`]/[`MAX_ANTECEDENT_CHARS`].
    antecedent_lines: Vec<String>,
    /// The DISTINCT eligible antecedents those caps turned away, in
    /// first-seen order. Rendered as one short marker line (its
    /// `.len()`) rather than vanishing: this module reports every
    /// degrade it performs (module doc's "documented seam"), and a
    /// strategy quoting three of five findings without saying so reads
    /// as though there were only three.
    ///
    /// The rejected LINES are kept, not just a count (ReviewCore). A
    /// regime holding several verdicts absorbs the same indexed
    /// findings once per verdict; the kept ones dedupe against
    /// `antecedent_lines`, so without the same memory for rejections a
    /// counter re-counted the identical drop on every later verdict and
    /// the marker read "+4", then "+6", for two distinct omissions.
    antecedents_omitted: Vec<String>,
}

impl Default for RegimeEvidence {
    /// `latest_at` seeds `DateTime::<Utc>::MIN_UTC` so the first
    /// absorbed verdict always wins the running max — no `Option`
    /// dance, and no unreachable `unwrap` at the one place the
    /// timestamp is read. (`chrono` implements no `Default` for
    /// `DateTime<Utc>`, hence the hand-written impl.)
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            latest_at: DateTime::<Utc>::MIN_UTC,
            join_key: None,
            scenario_title: None,
            kind_labels: Vec::new(),
            evidence_lines: Vec::new(),
            antecedent_lines: Vec::new(),
            antecedents_omitted: Vec::new(),
        }
    }
}

impl RegimeEvidence {
    /// Folds one [`DerivedVerdict`] in, keeping the running max
    /// timestamp and both first-seen-ordered dedup lists.
    fn absorb(&mut self, derived: DerivedVerdict) {
        let DerivedVerdict { regime_key: _, row, at, join_key, scenario_title, kind_label, evidence_line, antecedents } = derived;
        if self.rows.is_empty() {
            self.join_key = Some(join_key);
        } else {
            // Unreachable by construction (see `join_key`'s doc): a
            // group's `regime_key` hash IS this key's digest. Asserted
            // in debug rather than assumed silently — and in release it
            // degrades to naming the regime key, never a wrong id.
            debug_assert_eq!(
                self.join_key.as_deref(),
                Some(join_key.as_str()),
                "a regime group must hold exactly one join key — regime_hash is content_digest of it"
            );
            if self.join_key.as_deref() != Some(join_key.as_str()) {
                self.join_key = None;
            }
        }
        // s39: every verdict in a group shares one join key, so they
        // resolve the SAME title — except one whose adapter recorded no
        // `project_id` in `detail`, which resolves `None` where its
        // siblings resolve `Some`. First non-`None` wins, so the
        // rendered title never depends on which member happened to be
        // absorbed first.
        if self.scenario_title.is_none() {
            self.scenario_title = scenario_title;
        }
        self.rows.push(row);
        if at > self.latest_at {
            self.latest_at = at;
        }
        push_first_seen(&mut self.kind_labels, kind_label);
        push_first_seen(&mut self.evidence_lines, evidence_line);
        // Strictly AFTER this verdict's own line lands in
        // `evidence_lines` — see `absorb_antecedents`' dedupe contract.
        self.absorb_antecedents(antecedents);
    }

    /// Folds one verdict's antecedent lines into the regime's shared
    /// block (s39 `joined-evidence-grounding`).
    ///
    /// Called from [`Self::absorb`] AFTER that verdict's own
    /// `evidence_line` is in `evidence_lines`, which is what lets the
    /// dedupe consult BOTH lists: a line already rendered as an outcome
    /// must not reappear under the "preceded by" heading, where it would
    /// read as a distinct earlier finding rather than the same sentence
    /// twice.
    ///
    /// The caps live here rather than in [`antecedents_before`] because
    /// what they bound is the RENDERED block, and that block is
    /// per-regime: a regime holding six verdicts would otherwise
    /// accumulate six separately-capped lists. Both bounds are checked
    /// before the push, so the block never transiently exceeds them, and
    /// a rejection is COUNTED rather than dropped.
    fn absorb_antecedents(&mut self, antecedents: Vec<String>) {
        for line in antecedents {
            // All THREE lists: a line already rendered as an outcome,
            // already kept as an antecedent, or already rejected once is
            // the same sentence in every case, and the marker counts
            // DISTINCT omissions.
            if self.evidence_lines.contains(&line) || self.antecedent_lines.contains(&line) || self.antecedents_omitted.contains(&line) {
                continue;
            }
            // Recomputed rather than carried as a running field: at
            // `MAX_ANTECEDENT_LINES` entries this sums at most four
            // short strings, and a cached counter is one more piece of
            // state that can desync from the vector it describes.
            let budget: usize = self.antecedent_lines.iter().map(|kept| kept.chars().count()).sum::<usize>() + line.chars().count();
            if self.antecedent_lines.len() >= MAX_ANTECEDENT_LINES || budget > MAX_ANTECEDENT_CHARS {
                self.antecedents_omitted.push(line);
                continue;
            }
            self.antecedent_lines.push(line);
        }
    }

    /// This group's `(task, context)` — the trajectory fields
    /// `canon_learn::distill_trajectory` turns into a strategy's TITLE
    /// and CONTENT (`s38-evidence-bearing-memory`):
    ///
    /// - `task` names the concrete artifact plus its deduped kind
    ///   labels (`platformer.hud.01: review promotion`), so a retrieved
    ///   `avoid: …` guardrail says what to avoid on WHAT — followed,
    ///   when s39's [`ScenarioTitleIndex`] joined one, by the
    ///   scenario's own `title` after an em dash
    ///   (`platformer.moving.01: review attestation — A moving platform
    ///   carries the standing player`). The id stays first and stays
    ///   verbatim: it is the join key a reader needs to find the
    ///   artifact, and the sentence after it is what makes the id mean
    ///   something. With no joined title the string is byte-identical
    ///   to s38's, so the enrichment is purely additive.
    /// - `context` is the deduped evidence lines, newline-joined — real
    ///   reviewer/divergence/task prose, never a `detail` blob (see
    ///   [`ArtifactEvent::evidence_line`]) — followed, when s39's
    ///   antecedent join found any, by a `preceded on this artifact by:`
    ///   heading and one bullet-indented line per finding. The heading
    ///   plus the indent are what make a finding tellable from the
    ///   outcome that closed it: both are one line of the same kind of
    ///   prose, and an undifferentiated list reads as N independent
    ///   outcomes rather than one outcome and its antecedents.
    ///
    /// Both are pure functions of the accumulated, first-seen-ordered
    /// input, and `canon_ingest::scanner::scan_dir` hands this driver a
    /// byte-lexically deterministic event order — so two passes over an
    /// unchanged corpus produce byte-identical strings and
    /// [`trajectory_content_digest`]'s duplicate skip still fires.
    fn trajectory_text(&self, regime_key: &RegimeKey) -> (String, String) {
        let subject = self.join_key.as_deref().unwrap_or_else(|| regime_key.as_str());
        let labelled = if self.kind_labels.is_empty() { subject.to_string() } else { format!("{subject}: {}", self.kind_labels.join(", ")) };
        // One line, plain text: this lands in a distilled strategy's
        // title, which `canon retrieve` injects into a dispatched
        // agent's context — no markup to render, nothing to parse.
        let task = match &self.scenario_title {
            Some(title) => format!("{labelled} — {title}"),
            None => labelled,
        };
        let mut context = self.evidence_lines.join("\n");
        if !self.antecedent_lines.is_empty() {
            if !context.is_empty() {
                context.push('\n');
            }
            context.push_str("preceded on this artifact by:");
            for line in &self.antecedent_lines {
                context.push_str("\n  - ");
                context.push_str(line);
            }
            if !self.antecedents_omitted.is_empty() {
                context.push_str(&format!("\n  (+{} more omitted at the antecedent cap)", self.antecedents_omitted.len()));
            }
        }
        (task, context)
    }
}

/// Appends `item` only if absent, preserving FIRST-SEEN order
/// (`s38-evidence-bearing-memory`). A `BTreeSet` would dedupe too, but
/// would re-sort the labels/lines alphabetically — and the order a
/// regime's evidence was observed in is the order that reads as a
/// narrative. Linear `contains` is right at this size: a regime group
/// holds a handful of events, and the allocation a hash set would cost
/// exceeds the scan it saves.
fn push_first_seen<T: PartialEq>(seen: &mut Vec<T>, item: T) {
    if !seen.contains(&item) {
        seen.push(item);
    }
}

/// The most antecedent findings one regime's rendered `context` block
/// carries (s39 `joined-evidence-grounding`).
///
/// Four is sized off the corpus this join exists for: canon's own
/// `.canon/ledger/kind=divergence` records carry at most two `open`
/// findings on a scenario before the `resolved` record that closed
/// them, so four leaves headroom for an artifact that took two review
/// rounds while keeping the block scannable at a glance. The bound
/// matters because `canon retrieve` injects SEVERAL strategies into one
/// dispatched agent's context at once — unbounded, a single
/// heavily-reviewed artifact would crowd out every other strategy
/// retrieved for that role.
const MAX_ANTECEDENT_LINES: usize = 4;

/// The char budget that SAME block gets, enforced independently of
/// [`MAX_ANTECEDENT_LINES`] (s39 `joined-evidence-grounding`).
///
/// A count cap alone does not bound length. `ArtifactEvent::evidence_line`
/// caps each line at 512 chars, so four of them is ~2 KiB — several
/// times a typical trajectory's entire content. 1536 (three full-cap
/// lines) passes the real corpus untouched, whose findings run 157–485
/// chars so four of them fit comfortably, while bounding a pathological
/// record — a pasted stack trace, a diff — to roughly three lines'
/// worth. Dropping is whole-line by construction: half a finding is
/// worse than none, because its locator and its remedy sit at opposite
/// ends of the sentence.
const MAX_ANTECEDENT_CHARS: usize = 1536;

/// One prose-bearing NON-verdict event reduced to exactly what the
/// antecedent join reads (s39 `joined-evidence-grounding`): its
/// timestamp, to decide whether it PRECEDES a given verdict, and its
/// already-rendered [`ArtifactEvent::evidence_line`].
struct AntecedentEvent {
    at: DateTime<Utc>,
    line: String,
}

/// Prose-bearing non-verdict events bucketed by [`join_key_identity`],
/// each bucket totally ordered by `(at, line)` and deduped by line —
/// [`index_antecedents`]'s output and [`antecedents_before`]'s input
/// (s39 `joined-evidence-grounding`).
///
/// Keyed by the SOURCE-KIND-TAGGED identity rather than the bare
/// [`ArtifactJoinKey::as_str`], so a `task:x` finding can never attach
/// itself to a `scenario:x` verdict. It is the same string
/// [`regime_hash`] digests, which makes "shares my join key" and "would
/// have folded onto my regime" one question instead of two that can
/// disagree. `BTreeMap` (not a hash map) because a bucket's contents
/// reach rendered, agent-facing text.
type AntecedentIndex = BTreeMap<String, Vec<AntecedentEvent>>;

/// Indexes the events [`run`]'s accumulation loop is about to DROP but
/// which still carry real evidence (s39 `joined-evidence-grounding`) —
/// extracted from `run` for the same reason [`group_by_regime`] was:
/// the join it drives is then unit-testable without a live store.
///
/// Two filters, both required:
///
/// - **No verdict.** An event that derives one becomes a trajectory
///   line in its own right; indexing it too would print it twice, once
///   as the outcome and once as its own antecedent.
/// - **Real prose**, via [`ArtifactEvent::has_salient_prose`]. s38's
///   `evidence_line` degrades to the bare `display_label` when a record
///   carries no narrative, and to a `status <token>` marker one step
///   before that; neither is evidence, both are canon's own vocabulary,
///   and padding every retrieved strategy with it is the exact
///   regression `s38-evidence-bearing-memory` removed. The predicate
///   lives beside `salient_prose` in `canon-ingest` so this filter
///   cannot drift from the field-priority list it depends on — and so
///   it is not approximated by `evidence_line() != display_label()`,
///   which silently passes the status-token line.
///
/// Each bucket is then sorted by `(at, line)` — a TOTAL, data-derived
/// order, never the adapter scan order — and deduped by line text
/// keeping the EARLIEST occurrence. Earliest is not a coin flip: it is
/// the only choice that keeps [`antecedents_before`]'s cutoff honest,
/// since if the earliest copy of a line is too late to attach, every
/// copy is.
fn index_antecedents(events: &[ArtifactEvent]) -> AntecedentIndex {
    let mut index = AntecedentIndex::new();
    for event in events {
        if derive_verdict_for_event(event).is_some() || !event.has_salient_prose() {
            continue;
        }
        index.entry(event_identity(event)).or_default().push(AntecedentEvent { at: event.at, line: event.evidence_line() });
    }
    for bucket in index.values_mut() {
        bucket.sort_by(|a, b| (a.at, &a.line).cmp(&(b.at, &b.line)));
        let mut deduped: Vec<AntecedentEvent> = Vec::with_capacity(bucket.len());
        for candidate in std::mem::take(bucket) {
            if !deduped.iter().any(|kept| kept.line == candidate.line) {
                deduped.push(candidate);
            }
        }
        *bucket = deduped;
    }
    index
}

/// The antecedents one verdict-bearing event absorbs (s39
/// `joined-evidence-grounding`): the indexed events on its OWN join key
/// whose timestamp is at or before its own.
///
/// The cutoff is what makes this a JOIN rather than a bag. A finding
/// filed AFTER a resolution is a different, still-open problem that the
/// resolution demonstrably did not fix, so attaching it would distill a
/// claim the corpus does not support. Ties are inclusive on purpose: an
/// artifact whose finding and remediation were recorded in one batch
/// carries one timestamp for both, and excluding equality there would
/// lose exactly the pairing this join exists for.
///
/// The bucket is already `(at, line)`-ordered ascending, so the eligible
/// set is a prefix and a `take_while` suffices — no re-sort, and no
/// allocation at all for a key that has no antecedents.
fn antecedents_before(index: &AntecedentIndex, identity: &str, at: DateTime<Utc>) -> Vec<String> {
    let Some(bucket) = index.get(identity) else { return Vec::new() };
    bucket.iter().take_while(|antecedent| antecedent.at <= at).map(|antecedent| antecedent.line.clone()).collect()
}

/// Groups every derived verdict onto its `regime_key`
/// (`s38-evidence-bearing-memory`) — extracted from [`run`] so the
/// evidence accumulation this drives is unit-testable without a live
/// store. `BTreeMap` (not a hash map) keeps the persist loop's regime
/// order deterministic, as it was before.
///
/// The input is sorted by [`DerivedVerdict::cmp_stable`] FIRST — the one
/// choke point every verdict passes through on its way to
/// [`RegimeEvidence::absorb`], which is why the guarantee lives here
/// rather than at each call site (ReviewCore). Absorption preserves
/// arrival order for the ordered `rows` list and both first-seen dedup
/// lists, and all three reach persisted bytes; a records-source read
/// orders only by `at`, so without this the bytes would depend on how
/// a database happened to return two same-`at` rows.
fn group_by_regime(mut derived: Vec<DerivedVerdict>) -> BTreeMap<RegimeKey, RegimeEvidence> {
    derived.sort_by(DerivedVerdict::cmp_stable);
    let mut by_regime: BTreeMap<RegimeKey, RegimeEvidence> = BTreeMap::new();
    for verdict in derived {
        // `entry` needs an owned key while `absorb` consumes the rest
        // of the struct, so the key is cloned once per verdict — a
        // short `String`, and the alternative is a five-positional-
        // argument `absorb` whose call site invites exactly the
        // parameter mix-up `DerivedVerdict` exists to remove.
        by_regime.entry(verdict.regime_key.clone()).or_default().absorb(verdict);
    }
    by_regime
}

/// Derives one `VerdictRow` from an `ArtifactEvent` — dispatching to
/// the S15 P4 native helpers
/// ([`derive_native_review_verdict`]/[`derive_native_divergence_verdict`],
/// design D7) for an event emitted by the native `review`/
/// `divergence-native` records-source adapters, identified by the
/// ADAPTER-CONTROLLED `event.adapter_id` (`"review"` |
/// `"divergence-native"`); every other event (the four S4 adapters,
/// including `handoff`) still goes through the frozen [`derive_verdict`]
/// table, UNCHANGED.
///
/// Dispatch is gated on `adapter_id`, NEVER on `detail["native_kind"]`
/// alone (ReviewP4): the S4 ledger/divergence raw-path adapters copy
/// raw artifact JSON verbatim into `detail`, so a raw record that
/// happens to carry a stray `native_kind` field must NOT hijack the
/// native branch and silently drop the verdict the frozen S4 table
/// would have scored. `adapter_id` is a `&'static str` each adapter
/// sets on its own events (never copied from source content), so it
/// cannot be spoofed by artifact data. The `divergence-native` arm
/// still reads the record's own `detail["status"]` (an
/// adapter-emitted payload, not a routing key) to recover the typed
/// `DivergenceStatus`.
///
/// The role is ALWAYS `event.authoring_role` (`envelope.actor.role`)
/// for a native event — never `derive_verdict`'s hard-coded
/// constants (spec `native-record-flywheel` Requirement 2). An event
/// with no derivable `authoring_role`, or a `divergence-native` event
/// whose `detail["status"]` fails to round-trip through
/// `DivergenceStatus` (a caller-contract violation, never expected
/// from `crate::artifact_ingest`'s own adapters), yields `None` —
/// skipped, never a fabricated role or status.
fn derive_verdict_for_event(event: &ArtifactEvent) -> Option<VerdictRow> {
    match event.adapter_id {
        "review" => event.authoring_role.as_ref().map(derive_native_review_verdict),
        "divergence-native" => {
            let role = event.authoring_role.as_ref()?;
            let status = event.detail.get("status").and_then(|v| serde_json::from_value::<DivergenceStatus>(v.clone()).ok())?;
            derive_native_divergence_verdict(&status, role)
        }
        _ => derive_verdict(event.kind, event.authoring_role.as_ref()),
    }
}

/// What [`run`] does with one freshly-derived regime group — resolved
/// BEFORE any store write by [`plan_trajectory`], so the decision is
/// unit-testable against a plain slice of already-stored trajectories.
enum PersistPlan {
    /// The row this pass would write to already carries the identical
    /// FULL identity ([`trajectory_content_digest`]), and this driver has
    /// no superseded duplicate left in the namespace: the module doc's
    /// write-time idempotence, an unchanged corpus writing nothing at all
    /// — no raw write, no distilled-tier re-derivation.
    SkipDuplicate,
    /// Same: nothing to WRITE, the canonical row is already current. But
    /// this driver's own superseded rows ([`superseded_driver_ids`]) still
    /// sit in the namespace, so the pass must still converge it
    /// ([`rebuild_namespace_converged`]) — otherwise a stale sibling's
    /// `StrategyItem` keeps being served beside the fresh one until the
    /// corpus happens to change (s41 (`review-hardening`), re-review
    /// finding 1). Counted as a skipped duplicate exactly like
    /// [`PersistPlan::SkipDuplicate`], never as a persist: no `Trajectory`
    /// row is written.
    ///
    /// Since s42 (`close-the-open-loops`) this arm is normally ONE-SHOT
    /// per namespace rather than a standing state. Convergence now
    /// DELETES the superseded rows ([`rebuild_namespace_converged`])
    /// instead of stepping around them, so the next pass over an
    /// unchanged corpus finds [`superseded_driver_ids`] empty and takes
    /// [`PersistPlan::SkipDuplicate`] — writing nothing and deleting
    /// nothing.
    ///
    /// With ONE deliberate exception: a superseded row whose
    /// independently-written provenance the canonical row does not carry
    /// is KEPT ([`RowProvenance`], module doc), so
    /// [`superseded_driver_ids`] stays non-empty and this arm stands
    /// until an operator resolves the disagreement. That is the intended
    /// reading — the condition really has not been converged — and it
    /// costs nothing: the rebuild it drives is a fixpoint that rewrites
    /// byte-identical strategy rows, and the pass re-reports the conflict
    /// ([`ArtifactIngestOutcome::convergence_conflicts`]) instead of
    /// quietly forgetting it.
    ConvergeDistilled,
    /// A first derivation of this logical verdict set — a new row
    /// beside whatever the namespace already holds.
    Fresh(Trajectory),
    /// A RE-derivation of a logical verdict set this driver already
    /// wrote, whose text has changed — the SAME row, rewritten.
    Supersede(Trajectory),
}

/// The row this driver treats as CANONICAL for one logical verdict set
/// under one regime: the smallest [`canon_learn::TrajectoryId`] among the
/// rows in `existing` that
///
/// 1. sit under the SAME `regime_key` — `existing` is exactly that
///    namespace's rows, from
///    [`canon_learn::TrajectoryStore::query_by_regime_key`];
/// 2. carry the [`ARTIFACT_INGEST_TAG`]: this driver overwrites only its
///    OWN derivations, never a fixture, a webhook write, or a future
///    writer's row, however closely the verdicts match; and
/// 3. share `derivation` ([`trajectory_derivation_key`]) — the same
///    logical verdict set, i.e. an answer to the same question.
///
/// `None` when this driver has never written this verdict set into the
/// namespace: nothing of its own to replace, so the plan mints a row.
///
/// Rung 3's tie-break — smallest id — keys on the ID rather than
/// `recorded_at` because the id is IMMUTABLE across rewrites while
/// `recorded_at` is overwritten with each pass's `latest_at`: a timestamp
/// rule could hand the slot to a DIFFERENT row next pass and oscillate
/// between two, each keeping stale text forever, whereas picking the
/// smallest id is a fixed point by construction. No tie is possible — a
/// `TrajectoryId` identifies the row.
///
/// A pure function of the STORED rows, never of the pass's freshly
/// rendered text. That is what lets the write side ([`plan_trajectory`])
/// and the distill side ([`rebuild_namespace_converged`]) agree on which
/// row is canonical without passing any state between them — and what
/// makes the choice recomputable by any later reader of the namespace.
fn canonical_driver_row(existing: &[Trajectory], derivation: &str) -> Option<TrajectoryId> {
    existing
        .iter()
        .filter(|stored| stored.tags.iter().any(|tag| tag == ARTIFACT_INGEST_TAG))
        .filter(|stored| trajectory_derivation_key(&stored.regime_key, &stored.verdicts) == derivation)
        .map(|stored| stored.id)
        .min()
}

/// Every row in `existing` the canonical rule SUPERSEDES: a row this
/// driver wrote that is not the canonical member of its own logical
/// verdict set. Expressed as the exact negation of
/// [`canonical_driver_row`] rather than by restating the smallest-id rule,
/// so the row the plan writes to and the rows the distiller is denied can
/// never drift apart.
///
/// Empty for every namespace a post-s38 canon produced on its own — this
/// driver writes ONE row per `(regime_key, derivation key)` and supersedes
/// it in place — so the ordinary path pays one `is_empty` check. It is
/// non-empty for an UPGRADED store: a pre-s38 pass compared a digest that
/// ignored rendered text, so a renderer change appended a second row for
/// one verdict set instead of replacing the first (s41
/// (`review-hardening`), re-review finding 1 — the earlier slice assumed
/// that set was always a singleton, which no such store honours).
///
/// s42 (`close-the-open-loops`): these ids are the CANDIDATES for
/// deletion from the raw tier ([`plan_raw_convergence`]), not merely rows
/// kept out of the distiller's input. The set this function names is
/// unchanged — rows carrying [`ARTIFACT_INGEST_TAG`] that lost the
/// smallest-id tie-break inside their own derivation group — and it is
/// still exactly the set the distiller is denied. What deletion needs on
/// top of it is [`RowProvenance`]'s bar: sharing a derivation key proves
/// the rows answer the same question, never that they carry the same
/// independently-written verdict or attribution.
///
/// Quadratic in a namespace's row count, over a handful of rows per
/// regime and only inside a persist step that already does file I/O per
/// row — cheaper than the `BTreeMap` that would avoid it, and it keeps
/// ONE statement of the rule.
fn superseded_driver_ids(existing: &[Trajectory]) -> Vec<TrajectoryId> {
    existing
        .iter()
        .filter(|stored| stored.tags.iter().any(|tag| tag == ARTIFACT_INGEST_TAG))
        .filter(|stored| canonical_driver_row(existing, &trajectory_derivation_key(&stored.regime_key, &stored.verdicts)) != Some(stored.id))
        .map(|stored| stored.id)
        .collect()
}

/// The two independently-written fields a stored
/// [`canon_learn::Trajectory`] carries that this driver's DERIVATION does
/// not determine — written after it, by someone else, and therefore free
/// to differ between two rows [`trajectory_derivation_key`] calls equal.
///
/// - `verdict_record`: S7's rolled-up outcome+reward. Every freshly
///   constructed row starts `TrajectoryVerdict::pending()`; a covering
///   value arrives later through
///   [`canon_learn::mark_trajectory_verdict`] — [`run`]'s own persist
///   loop, or the S7 webhook receiver, aimed at ONE id.
/// - `run_id`: the dispatched run a `--run` pass was executing inside,
///   stamped by [`plan_trajectory`] onto the row that pass writes and
///   onto no other. Two passes under two runs therefore attribute two
///   different rows.
///
/// A named pair rather than two comparisons at the call site, and
/// [`RowProvenance::of`] destructures `Trajectory` EXHAUSTIVELY, so a
/// field added to that type later fails to compile here until someone
/// answers "is this independently written, and must the delete bar see
/// it?". The bar it feeds is irreversible, and silently defaulting that
/// question to "no" is exactly how this defect class was introduced.
#[derive(Debug, Clone, PartialEq)]
struct RowProvenance {
    verdict_record: TrajectoryVerdict,
    run_id: Option<RunId>,
}

impl RowProvenance {
    fn of(row: &Trajectory) -> Self {
        // Exhaustive on purpose (see this type's doc) — never a
        // two-field pick that a new `Trajectory` field slips past.
        let Trajectory { id: _, regime_key: _, task: _, context: _, verdicts: _, recorded_at: _, tags: _, verdict_record, run_id } = row;
        Self { verdict_record: *verdict_record, run_id: *run_id }
    }

    /// Every claim `self` makes that `canonical` does not also make —
    /// empty exactly when deleting the row `self` describes destroys
    /// nothing the canonical row still carries.
    ///
    /// A field is NOT a claim when it holds its unset value: the exact
    /// `TrajectoryVerdict::pending()` default (`Pending` at reward 0.5,
    /// which is what a row nobody ever marked reads back as — see
    /// `ParquetTrajectoryStore`'s `None`-outcome decode), or no `run_id`
    /// at all. Otherwise it must equal the canonical row's value
    /// EXACTLY, reward included: `CrnPromotionGate` folds
    /// `verdict_record.reward` as its sample value, so two rows agreeing
    /// on `Success` at different rewards are two different pieces of
    /// promotion evidence.
    ///
    /// The comparison is deliberately asymmetric. A canonical row that
    /// carries MORE than the loser is fine — the loser then claims
    /// nothing the store loses — while a loser carrying more is the
    /// conflict. That asymmetry is the whole safety property:
    /// `claims_beyond` is empty only when the canonical row subsumes the
    /// row about to be deleted.
    ///
    /// Rendered as lines rather than a bool so the report names WHICH
    /// fact disagrees and what each row holds, in this fixed field order
    /// — the output is operator-facing bytes, so its order is the
    /// source's, never a map's iteration.
    fn claims_beyond(&self, canonical: &Self) -> Vec<String> {
        let mut claims = Vec::new();
        if self.verdict_record != TrajectoryVerdict::pending() && self.verdict_record != canonical.verdict_record {
            claims.push(format!(
                "verdict_record `{}` at reward {:.3}, where the canonical row records `{}` at reward {:.3}",
                self.verdict_record.outcome, self.verdict_record.reward, canonical.verdict_record.outcome, canonical.verdict_record.reward
            ));
        }
        match (self.run_id, canonical.run_id) {
            (Some(mine), Some(theirs)) if mine != theirs => {
                claims.push(format!("run_id `{mine}`, where the canonical row records `{theirs}`"));
            }
            (Some(mine), None) => claims.push(format!("run_id `{mine}`, where the canonical row records no run")),
            // Agreeing ids, or no attribution of my own to lose.
            (Some(_), Some(_)) | (None, _) => {}
        }
        claims
    }
}

/// Decides — without writing anything — what the raw tier's convergence
/// for one regime should do: which rows the distiller may see, which
/// superseded copies may be deleted, and which must be kept and reported.
///
/// s42 (`close-the-open-loops`) task 2.2, hardened by its own re-review
/// (`ReviewRuns` finding 1). s41 computed [`superseded_driver_ids`] and
/// only filtered it out of the distiller's input, because
/// `canon_learn::TrajectoryStore` held no deletion primitive; now that
/// `delete_by_id` exists the raw layer can converge too — but supersession
/// alone does not authorize a delete. The module doc's "The derivation key
/// is NOT enough to authorize that delete" states why; [`RowProvenance`]
/// states the bar; this function applies it.
///
/// PURE with respect to the store: the only call it makes is the read.
/// Separating the decision from the writes is what lets
/// [`rebuild_namespace_converged`] order those writes safely — a planner
/// that deleted as it went could not be run before the strategy rebuild.
///
/// Every s41 invariant survives, because the superseded SET is unchanged
/// and the new bar only ever shrinks what is deleted:
///
/// 1. a row this driver did not write is NEVER deleted and NEVER
///    withheld — an untagged row, or one whose
///    [`trajectory_derivation_key`] differs, is not in the set at all
///    ([`canonical_driver_row`] rungs 2 and 3);
/// 2. the canonical id is stable across passes — deletion only ever
///    removes NON-minimal ids from a derivation group, so the group's
///    minimum, which is exactly what [`canonical_driver_row`] returns, is
///    the same id before and after, and a KEPT conflict cannot move it
///    either (it too is non-minimal); and
/// 3. a converged namespace deletes nothing — one pass empties the
///    deletable set, and the pass after it takes
///    [`PersistPlan::SkipDuplicate`] unless a conflict was kept, in which
///    case [`PersistPlan::ConvergeDistilled`] stands and this function
///    re-reports the same conflict over a byte-identical rebuild.
///
/// Both output lists are sorted by [`canon_learn::TrajectoryId`], which
/// identifies a row and so is a TOTAL order over a namespace.
/// `query_by_regime_key` reads a directory, whose order is the
/// filesystem's: unsorted, the reported conflict lines would differ
/// between two passes over identical bytes, and a partially-failed delete
/// would leave a different survivor set each attempt.
fn plan_raw_convergence(trajectory_store: &dyn TrajectoryStore, regime_key: &RegimeKey) -> Result<RawConvergence, LearnError> {
    let mut rows = trajectory_store.query_by_regime_key(regime_key)?;
    let superseded = superseded_driver_ids(&rows);
    let mut deletable = Vec::new();
    let mut conflicts = Vec::new();
    for losing in rows.iter().filter(|row| superseded.contains(&row.id)) {
        let derivation = trajectory_derivation_key(&losing.regime_key, &losing.verdicts);
        // Unreachable by construction: `superseded_driver_ids` selected
        // this row precisely BECAUSE `canonical_driver_row` over this same
        // slice returned a different id for this same derivation, so both
        // lookups resolve. Written as a skip rather than an `expect`
        // because `rows` is store-supplied data, and the safe reading of
        // "I cannot find this row's canonical partner" is to leave the row
        // alone — it stays withheld from the distiller either way.
        let Some(canonical) = canonical_driver_row(&rows, &derivation).and_then(|id| rows.iter().find(|row| row.id == id)) else {
            continue;
        };
        let claims = RowProvenance::of(losing).claims_beyond(&RowProvenance::of(canonical));
        if claims.is_empty() {
            deletable.push(losing.id);
        } else {
            conflicts.push(ConvergenceConflict {
                regime_key: regime_key.as_str().to_string(),
                canonical_id: canonical.id.to_string(),
                retained_id: losing.id.to_string(),
                claims,
            });
        }
    }
    deletable.sort_unstable();
    conflicts.sort_by(|left, right| left.retained_id.cmp(&right.retained_id));
    rows.retain(|row| !superseded.contains(&row.id));
    Ok(RawConvergence { retained: rows, deletable, conflicts })
}

/// What [`plan_raw_convergence`] decided, as a named triple rather than a
/// positional tuple: the three lists mean entirely different things to
/// the caller — one is distiller input, one is a delete list, one is an
/// operator report — and a bare tuple would let a caller distill the
/// wrong one.
struct RawConvergence {
    /// The regime's rows the distiller may see: every row that is NOT
    /// superseded, in `query_by_regime_key` order. Superseded rows are
    /// excluded whether or not they were deletable — s41's rule, unchanged
    /// by s42: a kept conflict is kept, not served.
    retained: Vec<Trajectory>,
    /// The superseded ids whose provenance the canonical row subsumes, so
    /// deleting them destroys nothing the store still holds
    /// ([`RowProvenance::claims_beyond`] empty). Sorted by id. Empty for
    /// every namespace a post-s38 canon produced on its own.
    deletable: Vec<TrajectoryId>,
    /// The superseded rows the bar REFUSED to delete, one report each,
    /// sorted by the kept row's id.
    conflicts: Vec<ConvergenceConflict>,
}

/// The same, one tier up: what a full [`rebuild_namespace_converged`]
/// pass left in each tier.
#[cfg_attr(test, derive(Debug))]
struct NamespaceConvergence {
    /// The freshly distilled items now in the strategy tier.
    items: Vec<StrategyItem>,
    /// How many raw rows the pass actually deleted
    /// ([`RawConvergence::deletable`]'s length, reached only after the
    /// strategy rebuild committed) — the raw tier's own convergence is
    /// not a distilled-tier concern, but this is the boundary `run`
    /// reports from.
    rows_deleted: usize,
    /// [`RawConvergence::conflicts`], passed straight through to
    /// [`ArtifactIngestOutcome::convergence_conflicts`].
    conflicts: Vec<ConvergenceConflict>,
}

/// `canon_learn::rebuild_namespace` over the CONVERGED row set
/// ([`plan_raw_convergence`]), so one logical verdict set distills into
/// one strategy set instead of one per stored copy — and then, and only
/// then, removes the raw copies that convergence made redundant.
///
/// Why the convergence lives here rather than in `canon-learn`: it is
/// stated in [`ARTIFACT_INGEST_TAG`] and [`trajectory_derivation_key`],
/// which are THIS driver's semantics. `canon-learn` cannot name the rows
/// one of its callers superseded without importing them, so its own
/// `rebuild_namespace` necessarily distills every row it is given; this
/// function is that same delete-rebuild with the caller's own convergence
/// applied first — to its own input, and then to the raw tier.
///
/// # The order of the three writes is the retry contract
/// Plan, then rebuild the strategy namespace, then delete the raw
/// duplicates. s42 shipped that last step FIRST and made the operation
/// non-retriable (`ReviewRuns` finding 2): the raw duplicate is the ONLY
/// condition that selects [`PersistPlan::ConvergeDistilled`], so deleting
/// it before the rebuild committed meant a failed
/// `delete_for_regime_key` or `append` returned with the trigger already
/// gone — an unchanged retry then took [`PersistPlan::SkipDuplicate`] and
/// left the strategy namespace empty or half-rebuilt forever, with
/// nothing in the store to say so.
///
/// In this order every failure point leaves a store an unchanged retry
/// still converges:
///
/// - the read or the plan fails: nothing was written at all;
/// - `delete_for_regime_key` or any `append` fails: the raw duplicates
///   are all still there, so the next pass plans the identical
///   convergence and rebuilds from scratch (the rebuild is a
///   delete-then-append over the whole namespace, so a half-written
///   strategy tier is fully replaced, not appended to);
/// - a `delete_by_id` fails partway: the strategy tier is already correct
///   and the remaining duplicates keep selecting `ConvergeDistilled`, so
///   the retry re-runs a byte-identical rebuild and deletes the rest.
///
/// Both halves are fixpoints, which is what makes those retries free:
/// `delete_for_regime_key` then re-append, with
/// `canon_learn::distill_namespace` deriving every field including the
/// `canon_learn::StrategyId`, rewrites byte-identical `<id>.parquet`
/// rows; and the raw half converges one step later — the first call
/// deletes the deletable superseded rows, every call after it finds
/// nothing left to delete (a kept conflict is re-reported, never
/// re-deleted).
///
/// The `?` on each delete is a genuine failure, never a routine miss:
/// every id came out of `query_by_regime_key` for THIS `regime_key` in
/// this same call, so `delete_by_id`'s loud
/// `canon_learn::LearnError::UnknownTrajectoryRow` can only mean the store
/// changed underneath the pass.
///
/// A `StrategyRef` some `Run` recorded against an item that leaves the
/// distilled tier stops resolving — the same, already-documented
/// consequence any re-derivation from changed evidence has
/// (`canon-report`'s `mart_flywheel_funnel` notes it), and the price of
/// not serving stale guidance.
fn rebuild_namespace_converged(
    trajectory_store: &dyn TrajectoryStore,
    strategy_store: &dyn StrategyStore,
    regime_key: &RegimeKey,
) -> Result<NamespaceConvergence, LearnError> {
    let raw = plan_raw_convergence(trajectory_store, regime_key)?;

    strategy_store.delete_for_regime_key(regime_key)?;
    let items = distill_namespace(regime_key, &raw.retained);
    for item in &items {
        strategy_store.append(item)?;
    }

    // Last, and only now. Until this loop runs, the duplicates are what
    // makes an unchanged retry converge again; after it, the strategy
    // namespace they were needed for is already committed.
    for id in &raw.deletable {
        trajectory_store.delete_by_id(regime_key, id)?;
    }
    Ok(NamespaceConvergence { items, rows_deleted: raw.deletable.len(), conflicts: raw.conflicts })
}

/// Renders one regime group's text and decides how it lands in the
/// store (ReviewCore).
///
/// The rendering happens HERE, before the duplicate check, which is
/// the half of the upgrade fix that is easy to lose: the pre-s38 code
/// compared digests first and returned early, so the text it was about
/// to write never entered the comparison at all.
///
/// # The supersession rule
/// A candidate supersedes the CANONICAL row for its logical verdict set
/// ([`canonical_driver_row`] states the three rungs and why the tie-break
/// is the smallest id), and the duplicate skip is scoped to that same row:
/// a pass writes nothing only when the row it WOULD have rewritten already
/// carries the freshly rendered identity.
///
/// Scoping it that way is the whole of s41's re-review fix. Asking
/// whether ANY stored row carried the identity let a sibling that already
/// held the current text answer for a canonical row that did not — so the
/// stale canonical row was never rewritten, and (since the skip returns
/// before any rebuild) the distilled tier kept both. When this driver has
/// no row of its own for the verdict set, the question stays "any row",
/// deliberately: a row it may NOT replace (a fixture, a webhook write)
/// already saying exactly this is still a duplicate, and appending a
/// second copy beside it would distill the same strategy twice.
///
/// Reusing the canonical id is what makes the write REPLACE instead of
/// append: the shipped `canon_learn::ParquetTrajectoryStore` keys one file
/// per `<regime_key>/<id>.parquet`, so `canon_learn::store_trajectory`
/// lands on the same row. The raw tier still stays append-only across
/// DISTINCT derivations (design decision 3) — a genuinely new verdict set
/// is always a new row, and the only bytes a write here overwrites are
/// this driver's own previous answer to the identical question. What s42
/// (`close-the-open-loops`) changed is the other direction: the extra
/// copies an OLDER canon appended for a SINGLE verdict set are now removed
/// by [`rebuild_namespace_converged`] rather than left in place and
/// stepped around, so after one pass this plan runs against a namespace
/// holding at most one row of its own per verdict set — except for a copy
/// the delete refused because it carries provenance the canonical row
/// does not ([`RowProvenance`]), which stays until an operator resolves
/// it. Either way the plan is unaffected: it only ever reads and writes
/// the CANONICAL row.
///
/// # `run_id` is stamped here, and only from `run_id`
/// `run_id` is the DISPATCHED RUN this pass was told it is executing
/// inside (s42 (`close-the-open-loops`) task 3.2, `canon ingest
/// artifacts --run`), stamped onto the candidate and onto nothing else.
/// It is never derived from a timestamp, a role, or the newest manifest
/// in `.canon/dispatch/`: `mart_flywheel_funnel`'s attribution stage
/// reads this field as a claim that the run carried the guidance this
/// trajectory answers, and a guessed run would make the mart assert a
/// relationship the corpus does not hold.
///
/// It is deliberately absent from both digests above, so it changes
/// NOTHING about this function's decision. Two consequences, both
/// intended:
///
/// - Write-time idempotence is unaffected. A pass inside a dispatch and
///   a pass outside one, over the same corpus, still recognize each
///   other's rows as duplicates — if attribution joined
///   [`trajectory_content_digest`], every dispatched ingest would derive
///   a fresh identity for unchanged evidence and rewrite the whole
///   namespace. [`trajectory_derivation_key`] must exclude it for a
///   second, stronger reason: that key is UNVERSIONED so it can still
///   recognize rows written by any earlier canon, and widening it would
///   orphan every trajectory an upgrading user holds.
/// - Only a row this pass actually WRITES gets stamped. A
///   [`PersistPlan::SkipDuplicate`]/[`PersistPlan::ConvergeDistilled`]
///   pass returns before the candidate is built, so a stored row keeps
///   whatever attribution it already had. That is the honest reading: a
///   pass that derived nothing new did not produce that trajectory, and
///   overwriting a real earlier attribution with the current run would
///   be a fabrication in the opposite direction.
fn plan_trajectory(
    existing: &[Trajectory],
    regime_key: &RegimeKey,
    evidence: &RegimeEvidence,
    run_id: Option<&RunId>,
) -> Result<PersistPlan, LearnError> {
    let (task, context) = evidence.trajectory_text(regime_key);
    let identity = trajectory_content_digest(regime_key, &evidence.rows, &task, &context);
    let carries_identity =
        |stored: &Trajectory| trajectory_content_digest(&stored.regime_key, &stored.verdicts, &stored.task, &stored.context) == identity;

    let derivation = trajectory_derivation_key(regime_key, &evidence.rows);
    let canonical = canonical_driver_row(existing, &derivation);
    let already_persisted = match canonical {
        Some(id) => existing.iter().any(|stored| stored.id == id && carries_identity(stored)),
        None => existing.iter().any(|stored| carries_identity(stored)),
    };
    if already_persisted {
        // Two arms, because the namespace can still hold this driver's own
        // superseded copies even when the canonical row is already
        // current: a pre-s38 pass appended a second row for one verdict
        // set whenever the renderer changed under it. Those copies are
        // DELETED by the convergence `ConvergeDistilled` asks for (s42
        // (`close-the-open-loops`)), so this arm fires at most once per
        // namespace — the pass after it sees a clean set, returns
        // `SkipDuplicate`, and writes and deletes nothing.
        //
        // The trigger asks about the whole NAMESPACE, not just this verdict
        // set, because the convergence it asks for is regime-wide:
        // whichever of a regime's groups notices the duplicates converges
        // all of them, and a group whose own set is clean pays one
        // `is_empty` check.
        return Ok(if superseded_driver_ids(existing).is_empty() { PersistPlan::SkipDuplicate } else { PersistPlan::ConvergeDistilled });
    }

    let trajectory = Trajectory::new(
        canonical.unwrap_or_else(TrajectoryId::new),
        regime_key.clone(),
        task,
        context,
        evidence.rows.clone(),
        evidence.latest_at,
        vec![ARTIFACT_INGEST_TAG.to_string()],
    )?
    .with_run_id(run_id.copied());
    Ok(if canonical.is_some() { PersistPlan::Supersede(trajectory) } else { PersistPlan::Fresh(trajectory) })
}

/// Admits `--run <RunId>` (s42 (`close-the-open-loops`) task 3.2) by
/// resolving the dispatch manifest [`crate::dispatch::begin`] wrote for
/// it, at the SAME `<repo>/.canon/dispatch/<run_id>.json` path
/// [`crate::dispatch::end`] resolves a close through, and holding it to
/// the SAME typed admission contract — literally the same function,
/// [`crate::dispatch::read_dispatched_manifest`], not a second statement
/// of it here.
///
/// That function's three rungs, restated only in terms of what each one
/// means for ATTRIBUTION (it states them in full):
///
/// 1. the manifest must be readable — absence alone is
///    [`ArtifactIngestError::UnknownRun`] ("this repo never began that
///    run"); a DIRECTORY at that path, a permission failure, or bytes
///    that are not a `Run` document are
///    [`ArtifactIngestError::UnreadableRunManifest`];
/// 2. it must deserialize as a typed
///    [`canon_model::records::Run`] — same variant; and
/// 3. its OWN `run_id` must equal the id the operator typed, else
///    [`ArtifactIngestError::MisfiledRunManifest`].
///
/// Then ONE rung of this driver's own: the run must have been DISPATCHED,
/// i.e. anything but [`RunStatus::Pending`]
/// ([`ArtifactIngestError::RunNeverDispatched`]). Deliberately weaker
/// than `dispatch end`'s `(Running, ended_at: None)` predicate, and the
/// variant's own doc says why: `end` mutates the run and so needs the
/// single closeable state, while this pass only reads an attribution
/// target, and a closed or aborted run genuinely executed.
///
/// Until s42's re-review (`ReviewAttribution` finding 1) this was a bare
/// `std::fs::metadata` probe, which succeeds for all three of a
/// directory, a truncated JSON file, and a valid `Run` filed under
/// somebody else's id — and the unchecked id was then stamped onto every
/// trajectory the pass wrote. A false attribution is worse than none:
/// `mart_flywheel_funnel` would assert a relationship the corpus does not
/// hold, which is the one failure mode this whole field exists to avoid.
/// Reading the file is affordable precisely because it is read once, per
/// pass, before any adapter runs.
///
/// Still a filesystem read rather than a tier read, for the reason it
/// always was: the manifest is written unconditionally by `dispatch
/// begin` and survives `dispatch end` (which rewrites it in place),
/// whereas the tier write s42 task 1.1 adds is documented as degrading to
/// manifest-only when the hot rung is down. Gating attribution on the
/// tier would make a correct `--run` fail on exactly the repos that
/// degraded.
fn resolve_dispatched_run(repo: &Path, run_id: Option<&RunId>) -> Result<(), ArtifactIngestError> {
    let Some(run_id) = run_id else { return Ok(()) };
    let manifest_path = repo.join(crate::dispatch::DISPATCH_DIR).join(format!("{run_id}.json"));
    let run = match crate::dispatch::read_dispatched_manifest(&manifest_path, *run_id) {
        Ok(run) => run,
        Err(DispatchError::NoSuchRun { run_id: named, path }) => return Err(ArtifactIngestError::UnknownRun { run_id: named, path }),
        Err(DispatchError::RunIdMismatch { path, expected, found }) => {
            return Err(ArtifactIngestError::MisfiledRunManifest { run_id: expected, path, found });
        }
        Err(DispatchError::Unreadable { path, detail }) => {
            return Err(ArtifactIngestError::UnreadableRunManifest { run_id: run_id.to_string(), path, detail });
        }
        // `Io` — a directory at the manifest path, a permission failure —
        // plus, exhaustively, any variant `read_dispatched_manifest` does
        // not construct today. A manifest whose bytes cannot be turned
        // into a `Run` is unusable for attribution however it failed, and
        // the wrapped error carries the diagnosis verbatim.
        Err(other) => {
            return Err(ArtifactIngestError::UnreadableRunManifest {
                run_id: run_id.to_string(),
                path: manifest_path.display().to_string(),
                detail: other.to_string(),
            });
        }
    };
    if run.status == RunStatus::Pending {
        return Err(ArtifactIngestError::RunNeverDispatched { run_id: run_id.to_string(), path: manifest_path.display().to_string() });
    }
    Ok(())
}

/// One scan -> derive-verdict -> persist pass over every registered
/// `ArtifactAdapter` (module doc).
///
/// `run_id` is the dispatched run this pass is executing inside, from
/// `canon ingest artifacts --run` (s42 (`close-the-open-loops`) task
/// 3.2). `None` — the overwhelming case, and every pre-s42 invocation —
/// means the trajectories this pass writes record NO run, rather than one
/// guessed from the clock or from `.canon/dispatch/`'s newest entry. A
/// `Some` is admitted against this repo's own dispatch manifests
/// ([`resolve_dispatched_run`], which reads and validates the manifest as
/// a typed `Run` through `dispatch end`'s own contract) BEFORE any
/// adapter read runs, so an unknown, damaged, misfiled or never-dispatched
/// run fails the whole pass loud instead of stamping an attribution that
/// joins to nothing — or, worse, one that joins to the wrong run.
/// [`plan_trajectory`] states where the value lands and why it is outside
/// both write-identity digests.
pub fn run(repo: &Path, run_id: Option<&RunId>) -> Result<ArtifactIngestOutcome, ArtifactIngestError> {
    let repo = resolve_repo_root(repo);
    let canon_yaml_path = repo.join("canon.yaml");
    let canon_yaml_text = std::fs::read_to_string(&canon_yaml_path).unwrap_or_default();

    resolve_dispatched_run(&repo, run_id)?;
    let artifact_config = load_artifact_source_config(&repo, &canon_yaml_text);
    validate_artifact_source_config(&artifact_config)?;
    // `LearnConfig::from_manifest`'s own contract (crates/canon-learn/
    // src/config.rs): a genuinely ABSENT `learn:` section (or an empty
    // `canon.yaml`) already resolves to `Ok(LearnConfig::default())`
    // inside `from_manifest` itself — that clean-default case is NOT
    // touched here. Only a malformed `learn:` section (bad YAML, an
    // invalid `roles:`/`promotion:` kebab-slug `RoleId`, a non-positive
    // `promotion.<role>.n_min`/`window_days`) reaches `Err`, and that
    // MUST fail this whole pass loud (`?` into `ArtifactIngestError::
    // Learn`, surfaced by `main.rs` as a nonzero exit) rather than
    // silently falling back to `LearnConfig::default()` and persisting
    // this run's trajectories into `<repo>/.canon/learn` — the wrong
    // store whenever the repo configured a different `learn.root`.
    let learn_config = LearnConfig::from_manifest(&canon_yaml_text)?;

    // s29 design D6: build ONE kind-scoped lenient tier set up front,
    // covering the union of every `Records`-source adapter's mapped
    // `RecordKind` that will actually run this pass (a `native_verdict:
    // true` entry with the switch off needs no live tier at all) --
    // malformed config (bad YAML/policy syntax, an invalid pg schema,
    // a non-forward aging rule, …) fails the WHOLE command loud here,
    // matching `crate::ingest`'s own contract; `canon.yaml` missing/
    // unreadable degrades every `Records`-source adapter to
    // "unavailable" (the pre-existing "documented seam" posture); an
    // individually unrouted kind (e.g. `handoff` with no
    // `routing.handoff`) is untouched by this step and surfaces via
    // `TierRegistry::query`'s own `UnroutedKind`, exactly as before.
    let records_kinds: Vec<RecordKind> = canon_ingest::artifact_registry::registry()
        .iter()
        .filter(|entry| entry.source_kind == ArtifactSourceKind::Records)
        .filter(|entry| !(entry.native_verdict && !artifact_config.native_records))
        .filter_map(|entry| record_kind_for_records_adapter(entry.adapter_id()).ok())
        .collect();

    // s39 (`joined-evidence-grounding`): the scenario-title join reads
    // `RecordKind::Scenario` too, so its own rung must be attached by
    // the SAME up-front build -- no adapter maps to that kind, so
    // without this the read would ask a `TierRegistry` that never
    // attempted the rung. Widening the kind set cannot change any
    // adapter's reported status: `unavailable_reasons` is keyed by RUNG,
    // and a rung some adapter's kind also routes to was already being
    // attempted for that adapter's sake. An UNROUTED `scenario` kind
    // contributes no rungs and is not an error here
    // (`build_lenient_tiers_for_kinds`' own contract).
    let tier_kinds: Vec<RecordKind> = records_kinds.iter().copied().chain([RecordKind::Scenario]).collect();

    let (store, policy_for_reason, unavailable_reasons, missing_config_reason): (Option<TierRegistry>, Option<TierPolicy>, BTreeMap<Rung, String>, Option<String>) =
        match tiers::build_lenient_tiers_for_kinds(&canon_yaml_path, &tier_kinds) {
            Ok(loaded) => {
                let policy = loaded.policy.clone();
                let reasons = loaded.unavailable_reasons.clone();
                (Some(TierRegistry::new(loaded.policy, loaded.git, loaded.pg, loaded.r2, loaded.sqlite)), Some(policy), reasons, None)
            }
            Err(TierCliError::ReadCanonYaml { path, source }) => (None, None, BTreeMap::new(), Some(format!("reading `{path}`: {source}"))),
            Err(other) => return Err(other.into()),
        };

    // s39: ONE read of the `Scenario` ledger index for the whole pass —
    // never one per event — through the exact read path every
    // `Records`-source adapter above uses ([`read_kind_records`]).
    //
    // A missing/unreadable `canon.yaml`, an unrouted `scenario` kind, or
    // an unreachable routed rung degrades to an EMPTY index and a
    // NORMAL run: this join only ever enriches text a trajectory would
    // have carried anyway, so its absence cannot add, drop, or alter a
    // single verdict or trajectory. That is what makes it different from
    // an adapter read failure, which loses evidence outright and
    // therefore MUST surface as `status: "unavailable"` in
    // `ArtifactIngestOutcome::adapters` (module doc's "documented
    // seam") — there is no adapter here to report, and inventing one
    // would misreport a read no registry entry owns. A genuinely
    // MALFORMED `canon.yaml` still fails the whole command loud, above,
    // before this line is ever reached.
    let scenario_titles = read_kind_records(RecordKind::Scenario, store.as_ref(), policy_for_reason.as_ref(), &unavailable_reasons, missing_config_reason.as_deref())
        .map(|records| ScenarioTitleIndex::from_records(&records))
        .unwrap_or_default();

    let mut adapters = Vec::new();
    let mut all_events: Vec<ArtifactEvent> = Vec::new();

    for entry in canon_ingest::artifact_registry::registry() {
        match entry.source_kind {
            ArtifactSourceKind::Path => match canon_ingest::artifact_registry::resolve_and_parse(entry, &artifact_config) {
                ArtifactDispatchOutcome::Parsed(parsed) => {
                    adapters.push(ArtifactAdapterSummary {
                        adapter_id: entry.adapter_id(),
                        source_kind: "path",
                        status: "read",
                        events_parsed: parsed.events.len(),
                        malformed: parsed.skipped,
                        unavailable_reason: None,
                    });
                    all_events.extend(parsed.events);
                }
                // Never reachable for a `Path`-kind entry today
                // (`resolve_and_parse` only returns this for
                // `Records`-kind entries) — handled anyway so a future
                // registry change can never silently regress into the
                // exact zero-events collapse this type exists to
                // prevent.
                ArtifactDispatchOutcome::UnsupportedSource { adapter_id, reason } => {
                    adapters.push(ArtifactAdapterSummary {
                        adapter_id,
                        source_kind: "path",
                        status: "unavailable",
                        events_parsed: 0,
                        malformed: 0,
                        unavailable_reason: Some(reason.to_string()),
                    });
                }
            },
            ArtifactSourceKind::Records => {
                if entry.native_verdict && !artifact_config.native_records {
                    // A native-verdict adapter (`review`/`divergence-native`,
                    // S15 P4) with the switch off is DISABLED, not
                    // unavailable — `"unavailable"` is reserved for a
                    // genuine read failure below.
                    adapters.push(ArtifactAdapterSummary {
                        adapter_id: entry.adapter_id(),
                        source_kind: "records",
                        status: "disabled",
                        events_parsed: 0,
                        malformed: 0,
                        unavailable_reason: None,
                    });
                    continue;
                }
                match read_records_for(entry.adapter_id(), store.as_ref(), policy_for_reason.as_ref(), &unavailable_reasons, missing_config_reason.as_deref()) {
                    Ok(raws) => {
                        let parsed = entry.adapter.parse(&ArtifactSourceHandle::Records(raws));
                        adapters.push(ArtifactAdapterSummary {
                            adapter_id: entry.adapter_id(),
                            source_kind: "records",
                            status: "read",
                            events_parsed: parsed.events.len(),
                            malformed: parsed.skipped,
                            unavailable_reason: None,
                        });
                        all_events.extend(parsed.events);
                    }
                    Err(reason) => {
                        adapters.push(ArtifactAdapterSummary {
                            adapter_id: entry.adapter_id(),
                            source_kind: "records",
                            status: "unavailable",
                            events_parsed: 0,
                            malformed: 0,
                            unavailable_reason: Some(reason),
                        });
                    }
                }
            }
        }
    }

    let label = repo_label(&repo);
    // Built BEFORE the accumulation loop over the SAME `all_events`:
    // that loop's `else { continue }` is precisely where a prose-bearing
    // non-verdict event would otherwise be dropped unread (s39
    // `joined-evidence-grounding`, module doc).
    let antecedent_index = index_antecedents(&all_events);
    let mut derived: Vec<DerivedVerdict> = Vec::new();
    for event in &all_events {
        let Some(row) = derive_verdict_for_event(event) else { continue };
        let area = event.area.clone().unwrap_or_else(|| "unscoped".to_string());
        let identity = event_identity(event);
        let hash = regime_hash(&identity);
        let verdict = attach_regime_key(row, event.join_key.clone(), &label, &area, &hash, event.trust_level.clone())?;
        derived.push(DerivedVerdict {
            regime_key: verdict.regime_key,
            row: verdict.row,
            at: event.at,
            join_key: event.join_key.as_str().to_string(),
            // s39: resolved HERE, the one place holding both the
            // once-read index and the event's own `detail` `project_id`.
            scenario_title: scenario_titles.title_for(&event.join_key, event_project_id(event)).map(str::to_string),
            kind_label: event.display_label(),
            evidence_line: event.evidence_line(),
            antecedents: antecedents_before(&antecedent_index, &identity, event.at),
        });
    }
    let verdicts_derived = derived.len();
    let by_regime = group_by_regime(derived);

    let learn_root = repo.join(&learn_config.root);
    let trajectory_store = ParquetTrajectoryStore::open(learn_root.join("trajectories"));
    let strategy_store = ParquetStrategyStore::open(learn_root.join("strategies"));
    let role_registry = RoleRegistry::from_config(&learn_config);
    // S7's per-role reward table (`dev`'s ported weighted composite plus
    // the five provisional roles); an unregistered role falls back to
    // `default_reward_fn` rather than erroring, so every trajectory
    // always has SOME reward function.
    let reward_registry = RewardRegistry::builtin();

    let mut trajectories_persisted = Vec::new();
    let mut trajectories_skipped_unregistered_role = 0usize;
    let mut trajectories_skipped_duplicate = 0usize;
    let mut trajectories_converged_deleted = 0usize;
    // `by_regime` is a `BTreeMap`, so this accumulates in regime-key
    // order, and each regime's own contribution is already sorted by the
    // kept row's id (`plan_raw_convergence`) — the whole list is
    // data-ordered, and two passes over one unchanged store print
    // byte-identical lines.
    let mut convergence_conflicts: Vec<ConvergenceConflict> = Vec::new();
    let mut strategy_items_rebuilt = 0usize;
    let mut trajectories_marked = 0usize;
    let mut trajectories_left_pending = 0usize;
    for (regime_key, evidence) in by_regime {
        let verdict_count = evidence.rows.len();
        let existing = trajectory_store.query_by_regime_key(&regime_key)?;
        let (trajectory, superseded) = match plan_trajectory(&existing, &regime_key, &evidence, run_id)? {
            PersistPlan::SkipDuplicate => {
                trajectories_skipped_duplicate += 1;
                continue;
            }
            PersistPlan::ConvergeDistilled => {
                // Nothing to write — the row this pass would have rewritten
                // already carries this exact text — but this driver's own
                // superseded copies are still in the namespace, so the pass
                // converges it anyway: the copies whose provenance the
                // canonical row also carries are deleted from the raw
                // tier, the rest are kept and reported, and the distilled
                // tier is re-derived from what supersession left. Without
                // this the stale sibling's strategy keeps being served for
                // as long as the corpus happens not to change, and its
                // bytes never go away at all.
                //
                // Deliberately NOT behind `store_trajectory`'s role-registry
                // gate, unlike the persist path below. That gate exists to
                // reject WRITING a trajectory for an unregistered role
                // ("fail loud at write time", `canon_learn::RoleRegistry`),
                // and this arm writes none. The deletion s42
                // (`close-the-open-loops`) added is not a write either: it
                // removes rows the store ALREADY accepted, which this arm
                // can only reach when ≥2 of them carry this driver's own
                // tag under this regime — so the namespace is demonstrably
                // one it wrote, and the role in question is the one those
                // rows were already stored under. Gating garbage collection
                // on a registry the rows predate would strand exactly the
                // upgraded namespaces this path exists for.
                trajectories_skipped_duplicate += 1;
                let converged = rebuild_namespace_converged(&trajectory_store, &strategy_store, &regime_key)?;
                trajectories_converged_deleted += converged.rows_deleted;
                convergence_conflicts.extend(converged.conflicts);
                strategy_items_rebuilt += converged.items.len();
                continue;
            }
            PersistPlan::Fresh(trajectory) => (trajectory, false),
            PersistPlan::Supersede(trajectory) => (trajectory, true),
        };

        match store_trajectory(&role_registry, &trajectory_store, &trajectory) {
            Ok(()) => {
                trajectories_persisted.push(PersistedTrajectory { regime_key: regime_key.as_str().to_string(), verdict_count, superseded });
                // Resolve the covering verdict+reward from the SAME
                // `VerdictRow`(s) this trajectory was just built from,
                // then write it back — closing S7 design D2's two-phase
                // reward-write model at the one call site that has both
                // halves in hand. `compute_for_trajectory` is documented
                // as "the shape `mark_trajectory_verdict`'s caller
                // typically wants" (`reward.rs`); before this wiring
                // existed every artifact-derived trajectory stayed
                // `Pending` forever, so no promotion gate could ever see
                // a corroborating sample. The role is known-registered
                // here — `store_trajectory` above already rejected an
                // unregistered one — so the `?` is a genuine
                // store/parse failure, not a routine miss.
                match reward_registry.compute_for_trajectory(&trajectory)? {
                    // `Pending` is the unset default; marking it is
                    // rejected by design. Leave the trajectory pending
                    // for a later covering signal (the S7 webhook
                    // receiver's PR/CI path) and count it.
                    (VerdictOutcome::Pending, _) => trajectories_left_pending += 1,
                    (outcome, reward) => {
                        // Marked BEFORE the distilled-tier rebuild
                        // deliberately: `mark_verdict` is the ONLY path
                        // allowed to rewrite a stored trajectory, and a
                        // rebuild must leave those bytes untouched
                        // (asserted by `parquet_trajectory`'s own
                        // round-trip test).
                        mark_trajectory_verdict(&trajectory_store, &trajectory.id, outcome, reward)?;
                        trajectories_marked += 1;
                    }
                }
                let converged = rebuild_namespace_converged(&trajectory_store, &strategy_store, &regime_key)?;
                trajectories_converged_deleted += converged.rows_deleted;
                convergence_conflicts.extend(converged.conflicts);
                strategy_items_rebuilt += converged.items.len();
            }
            Err(LearnError::UnregisteredRole(_)) => {
                trajectories_skipped_unregistered_role += 1;
            }
            Err(err) => return Err(err.into()),
        }
    }

    Ok(ArtifactIngestOutcome {
        adapters,
        verdicts_derived,
        trajectories_persisted,
        trajectories_skipped_unregistered_role,
        trajectories_skipped_duplicate,
        trajectories_converged_deleted,
        convergence_conflicts,
        strategy_items_rebuilt,
        trajectories_marked,
        trajectories_left_pending,
    })
}

/// Human-readable run summary — one line per adapter (its `status`
/// spelled out, never silently folded into a bare count), then the
/// verdict/persistence tallies.
pub fn format_human(outcome: &ArtifactIngestOutcome) -> String {
    let mut out = String::new();
    for adapter in &outcome.adapters {
        match adapter.status {
            "read" => out.push_str(&format!(
                "{} ({}): {} event(s) parsed, {} malformed\n",
                adapter.adapter_id, adapter.source_kind, adapter.events_parsed, adapter.malformed
            )),
            "disabled" => out.push_str(&format!(
                "{} ({}): disabled — artifacts.native_records is off\n",
                adapter.adapter_id, adapter.source_kind
            )),
            _ => out.push_str(&format!(
                "{} ({}): records source unavailable — {}\n",
                adapter.adapter_id,
                adapter.source_kind,
                adapter.unavailable_reason.as_deref().unwrap_or("unknown reason")
            )),
        }
    }
    out.push_str(&format!("verdicts derived: {}\n", outcome.verdicts_derived));
    out.push_str(&format!("trajectories persisted: {}\n", outcome.trajectories_persisted.len()));
    for trajectory in &outcome.trajectories_persisted {
        let how = if trajectory.superseded { ", superseded in place" } else { "" };
        out.push_str(&format!("  - {} ({} verdict(s){how})\n", trajectory.regime_key, trajectory.verdict_count));
    }
    // Derived from the rows above rather than carried as its own
    // counter, so the tally and the list it summarizes cannot disagree.
    let superseded = outcome.trajectories_persisted.iter().filter(|t| t.superseded).count();
    out.push_str(&format!("trajectories superseded (re-derived onto the existing row): {superseded}\n"));
    out.push_str(&format!("trajectories skipped (unregistered role): {}\n", outcome.trajectories_skipped_unregistered_role));
    out.push_str(&format!("trajectories skipped (duplicate, already persisted): {}\n", outcome.trajectories_skipped_duplicate));
    out.push_str(&format!("trajectories deleted (superseded copies converged): {}\n", outcome.trajectories_converged_deleted));
    out.push_str(&format!(
        "trajectories retained (superseded copy carrying provenance the canonical row does not): {}\n",
        outcome.convergence_conflicts.len()
    ));
    // Every kept conflict is named in full, never summarized to a count:
    // the operator has to know WHICH rows disagree and about WHAT before
    // they can decide which one is wrong, and this driver deliberately
    // will not decide for them (module doc). One line per disagreeing
    // field, so a row conflicting on both prints both.
    for conflict in &outcome.convergence_conflicts {
        for claim in &conflict.claims {
            out.push_str(&format!("  - {} row {} records {}\n", conflict.regime_key, conflict.retained_id, claim));
        }
        out.push_str(&format!(
            "    kept beside canonical row {} and withheld from the distiller; remove the row you judge wrong to converge it\n",
            conflict.canonical_id
        ));
    }
    out.push_str(&format!("strategy items rebuilt (distilled): {}\n", outcome.strategy_items_rebuilt));
    out.push_str(&format!("trajectories marked (covering verdict resolved): {}\n", outcome.trajectories_marked));
    out.push_str(&format!("trajectories left pending (awaiting a covering signal): {}\n", outcome.trajectories_left_pending));
    out
}

/// `--json`: the full outcome, machine-readable.
pub fn format_json(outcome: &ArtifactIngestOutcome) -> String {
    serde_json::to_string_pretty(outcome).expect("ArtifactIngestOutcome always serializes")
}

/// canon artifact-ingest's shared-contract selftest entry point (Wave-3
/// `canon selftest` aggregator, per-crate registration — unblocks S4
/// 7.4). Wraps this driver's pure write-identity invariants —
/// `regime_hash` (12-hex, deterministic, sensitive to the whole
/// `event_identity` including its project half) and
/// `trajectory_content_digest` (deterministic, regime-key-sensitive,
/// verdict-content-and-order-sensitive, and rendered-text-sensitive) —
/// as in-memory checks over synthetic keys/verdicts. No filesystem or
/// network read, side-effect-free against the real repo.
///
/// `Ok(n)` = checks passed; `Err(_)` = one line per failure, never panics.
pub fn selftest() -> Result<usize, Vec<String>> {
    use canon_ingest::verdict::{Becomes, Polarity};
    use canon_model::ids::{RoleId, ScenarioId};

    let mut passed = 0;
    let mut failures = Vec::new();

    let scen_a = ArtifactJoinKey::Scenario(ScenarioId::parse("world.firstbuy-hotdeal.26").expect("valid scenario id"));
    let scen_b = ArtifactJoinKey::Scenario(ScenarioId::parse("world.firstbuy-hotdeal.27").expect("valid scenario id"));
    let ident = |key: &ArtifactJoinKey, project: Option<&str>| regime_hash(&join_key_identity(key, project));
    let h = ident(&scen_a, Some("world"));
    if h.len() == 12
        && h == ident(&scen_a, Some("world"))
        && h != ident(&scen_b, Some("world"))
        // Two projects defining one scenario id are two regimes, and a
        // project-less event is a third — never silently either.
        && h != ident(&scen_a, Some("acme"))
        && h != ident(&scen_a, None)
    {
        passed += 1;
    } else {
        failures.push("regime-hash: not a deterministic 12-hex digest sensitive to the full event identity".to_string());
    }

    let key = RegimeKey::parse(canon_model::ids::regime_key("dev", "acme-repo", "world", "abc123")).expect("valid regime key");
    let key2 = RegimeKey::parse(canon_model::ids::regime_key("dev", "acme-repo", "world", "def456")).expect("valid regime key");
    let vr = |role: &str, p: Polarity, b: Becomes| VerdictRow { role: RoleId::parse(role).expect("valid role"), polarity: p, becomes: b };
    let guardrail = || vec![vr("dev", Polarity::Failure, Becomes::GuardrailCandidate)];
    let strategy = || vec![vr("dev", Polarity::Success, Becomes::StrategyCandidate)];

    let guard = guardrail();
    let strat = strategy();
    let mut both = guardrail();
    both.extend(strategy());
    let mut reversed = strategy();
    reversed.extend(guardrail());

    if trajectory_content_digest(&key, &guard, "t", "c") == trajectory_content_digest(&key, &guard, "t", "c")
        && trajectory_content_digest(&key, &guard, "t", "c") != trajectory_content_digest(&key2, &guard, "t", "c")
    {
        passed += 1;
    } else {
        failures.push("trajectory-digest-determinism: not deterministic or not regime-key-sensitive".to_string());
    }

    let content_differs = trajectory_content_digest(&key, &guard, "t", "c") != trajectory_content_digest(&key, &strat, "t", "c");
    let order_differs = trajectory_content_digest(&key, &both, "t", "c") != trajectory_content_digest(&key, &reversed, "t", "c");
    // The upgrade half (ReviewCore): a rebuild that changes ONLY the
    // rendered text must mint a different identity, or the stale row is
    // skipped as a duplicate and re-distilled forever.
    let text_differs = trajectory_content_digest(&key, &guard, "t", "c") != trajectory_content_digest(&key, &guard, "t", "c2")
        && trajectory_content_digest(&key, &guard, "t", "c") != trajectory_content_digest(&key, &guard, "t2", "c");
    // …while the derivation key stays blind to it, which is what lets
    // the rebuild find the row it replaces.
    let derivation_stable =
        trajectory_derivation_key(&key, &guard) == trajectory_derivation_key(&key, &guard) && trajectory_derivation_key(&key, &guard) != trajectory_derivation_key(&key, &strat);
    if content_differs && order_differs && text_differs && derivation_stable {
        passed += 1;
    } else {
        failures.push("trajectory-digest-sensitivity: not sensitive to verdict content, order, or rendered text".to_string());
    }

    if failures.is_empty() { Ok(passed) } else { Err(failures) }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use canon_ingest::artifact_adapter::ArtifactEventKind as Kind;

    use super::*;

    #[test]
    fn repo_label_falls_back_to_repo_when_basename_is_unavailable() {
        assert_eq!(repo_label(Path::new("/")), "repo");
        assert_eq!(repo_label(Path::new("/tmp/acme-repo")), "acme-repo");
    }

    /// One scenario-keyed event identity's `regime_key` hash component
    /// — the exact composition [`run`] performs
    /// ([`event_identity`] -> [`regime_hash`]), over a
    /// [`scenario_key`] and the `project_id` the event carried.
    fn identity_hash(id: &str, project: Option<&str>) -> String {
        regime_hash(&join_key_identity(&scenario_key(id), project))
    }

    #[test]
    fn regime_hash_is_a_twelve_char_lowercase_hex_digest_and_is_deterministic() {
        let hash = identity_hash("world.firstbuy-hotdeal.26", Some("world"));
        assert_eq!(hash.len(), 12);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert_eq!(hash, identity_hash("world.firstbuy-hotdeal.26", Some("world")), "the same event identity must always hash to the same regime_key hash component");
    }

    #[test]
    fn regime_hash_differs_across_distinct_join_keys() {
        assert_ne!(identity_hash("world.firstbuy-hotdeal.26", Some("world")), identity_hash("world.firstbuy-hotdeal.27", Some("world")));
    }

    #[test]
    fn an_event_identity_names_its_project_and_cannot_collide_with_a_missing_one() {
        // ReviewCore: a `Scenario`'s identity is `(project_id,
        // scenario_id)`, and this module already supports one scenario
        // id being defined by several projects. Keyed on the bare id,
        // project A's finding attached to project B's verdict.
        let key = scenario_key("platformer.moving.01");
        assert_eq!(join_key_identity(&key, Some("platformer")), "scenario:platformer.moving.01@platformer");
        assert_eq!(join_key_identity(&key, Some("world")), "scenario:platformer.moving.01@world");
        assert_eq!(join_key_identity(&key, None), "scenario:platformer.moving.01@-");
        // `-` cannot begin a `ProjectId`, so the unknown form is
        // unreachable from any real project — and a project string that
        // is not a well-formed `ProjectId` reads as unknown rather than
        // being pasted in verbatim, which is what keeps the `@` split
        // total.
        assert_eq!(join_key_identity(&key, Some("-")), join_key_identity(&key, None));
        assert_eq!(join_key_identity(&key, Some("Not A Project")), join_key_identity(&key, None));
        assert_eq!(join_key_identity(&key, Some("a@b")), join_key_identity(&key, None));

        assert_ne!(identity_hash("platformer.moving.01", Some("platformer")), identity_hash("platformer.moving.01", Some("world")));
        assert_ne!(identity_hash("platformer.moving.01", Some("platformer")), identity_hash("platformer.moving.01", None));
    }

    #[test]
    fn a_non_scenario_identity_carries_no_project_segment() {
        // A handoff- or task-keyed event names no project, so its
        // identity must be byte-identical whatever `detail` happens to
        // carry — otherwise s37/s40 handoff regimes would re-key.
        let handoff = ArtifactJoinKey::Handoff(canon_model::ids::HandoffId::parse("20260710-1432-fix-a1b2").unwrap());
        let task = ArtifactJoinKey::Task(canon_model::ids::TaskId::parse("frozen-fixture-change#1.4").unwrap());
        assert_eq!(join_key_identity(&handoff, None), "handoff:20260710-1432-fix-a1b2");
        assert_eq!(join_key_identity(&handoff, Some("platformer")), "handoff:20260710-1432-fix-a1b2");
        assert_eq!(join_key_identity(&task, None), "task:frozen-fixture-change#1.4");
        assert_eq!(join_key_identity(&task, Some("platformer")), "task:frozen-fixture-change#1.4");
    }

    #[test]
    fn load_artifact_source_config_resolves_configured_paths_against_repo_and_leaves_others_unconfigured() {
        let repo = Path::new("/tmp/some-repo");
        let yaml = "artifacts:\n  ledger_root: fixtures/ledger\n";
        let config = load_artifact_source_config(repo, yaml);
        assert_eq!(config.ledger_root, Some(repo.join("fixtures/ledger")));
        assert_eq!(config.divergences_root, None);
        assert_eq!(config.openspec_root, None);
    }

    #[test]
    fn load_artifact_source_config_degrades_to_default_when_artifacts_section_is_absent() {
        let repo = Path::new("/tmp/some-repo");
        let config = load_artifact_source_config(repo, "handoff_templates:\n  - foo\n");
        assert_eq!(config, ArtifactSourceConfig::default());
    }

    fn verdict_row(role: &str, polarity: canon_ingest::verdict::Polarity, becomes: canon_ingest::verdict::Becomes) -> VerdictRow {
        VerdictRow { role: canon_model::ids::RoleId::parse(role).unwrap(), polarity, becomes }
    }

    fn regime(area: &str, hash: &str) -> RegimeKey {
        RegimeKey::parse(canon_model::ids::regime_key("dev", "acme-repo", area, hash)).unwrap()
    }

    /// The rendered `(task, context)` pair the digest tests hold
    /// constant while varying the verdict half.
    const TEXT: (&str, &str) = ("platformer.moving.01: review attestation", "review attestation: 9c93d024b");

    #[test]
    fn trajectory_content_digest_is_deterministic_for_identical_input() {
        let key = regime("world", "abc123");
        let rows = vec![verdict_row("dev", canon_ingest::verdict::Polarity::Failure, canon_ingest::verdict::Becomes::GuardrailCandidate)];
        assert_eq!(trajectory_content_digest(&key, &rows, TEXT.0, TEXT.1), trajectory_content_digest(&key, &rows, TEXT.0, TEXT.1));
    }

    #[test]
    fn trajectory_content_digest_differs_across_distinct_regime_keys() {
        let rows = vec![verdict_row("dev", canon_ingest::verdict::Polarity::Failure, canon_ingest::verdict::Becomes::GuardrailCandidate)];
        let a = regime("world", "abc123");
        let b = regime("world", "def456");
        assert_ne!(trajectory_content_digest(&a, &rows, TEXT.0, TEXT.1), trajectory_content_digest(&b, &rows, TEXT.0, TEXT.1));
    }

    #[test]
    fn trajectory_content_digest_differs_when_verdict_contents_differ() {
        let key = regime("world", "abc123");
        let a = vec![verdict_row("dev", canon_ingest::verdict::Polarity::Failure, canon_ingest::verdict::Becomes::GuardrailCandidate)];
        let b = vec![verdict_row("dev", canon_ingest::verdict::Polarity::Success, canon_ingest::verdict::Becomes::StrategyCandidate)];
        assert_ne!(trajectory_content_digest(&key, &a, TEXT.0, TEXT.1), trajectory_content_digest(&key, &b, TEXT.0, TEXT.1));
    }

    #[test]
    fn trajectory_content_digest_is_sensitive_to_verdict_order() {
        let key = regime("world", "abc123");
        let first = verdict_row("dev", canon_ingest::verdict::Polarity::Failure, canon_ingest::verdict::Becomes::GuardrailCandidate);
        let second = verdict_row("dev", canon_ingest::verdict::Polarity::Success, canon_ingest::verdict::Becomes::StrategyCandidate);
        let forward = vec![first.clone(), second.clone()];
        let reversed = vec![second, first];
        assert_ne!(
            trajectory_content_digest(&key, &forward, TEXT.0, TEXT.1),
            trajectory_content_digest(&key, &reversed, TEXT.0, TEXT.1),
            "the digest folds ORDERED verdict contents — module doc's write-time idempotence relies on \
             `scan_dir`'s deterministic file order plus `group_by_regime`'s total sort producing the SAME \
             sequence every pass, so two different orderings must never collide"
        );
    }

    #[test]
    fn trajectory_content_digest_is_sensitive_to_the_rendered_text_it_names() {
        // ReviewCore's BLOCKER: the pre-s38 identity folded only the
        // verdict rows, so a trajectory whose `task`/`context` this
        // release renders differently kept its old digest, was skipped
        // as a duplicate, and re-distilled pre-s38 ingest-driver prose
        // forever.
        let key = regime("world", "abc123");
        let rows = vec![verdict_row("dev", canon_ingest::verdict::Polarity::Failure, canon_ingest::verdict::Becomes::GuardrailCandidate)];
        let base = trajectory_content_digest(&key, &rows, TEXT.0, TEXT.1);
        assert_ne!(base, trajectory_content_digest(&key, &rows, TEXT.0, "review attestation: 9c93d024b\npreceded on this artifact by:\n  - open divergence: SHIP-BLOCKER"));
        assert_ne!(base, trajectory_content_digest(&key, &rows, "platformer.moving.01: review attestation — A moving platform carries the standing player", TEXT.1));
    }

    #[test]
    fn the_derivation_key_names_the_verdict_set_alone_and_ignores_the_text() {
        // The other half of the same fix: the key that FINDS the row a
        // rebuild replaces must be blind to exactly the thing that
        // changed, or supersession could never match anything.
        let key = regime("world", "abc123");
        let rows = vec![verdict_row("dev", canon_ingest::verdict::Polarity::Failure, canon_ingest::verdict::Becomes::GuardrailCandidate)];
        let other = vec![verdict_row("dev", canon_ingest::verdict::Polarity::Success, canon_ingest::verdict::Becomes::StrategyCandidate)];
        assert_eq!(trajectory_derivation_key(&key, &rows), trajectory_derivation_key(&key, &rows));
        assert_ne!(trajectory_derivation_key(&key, &rows), trajectory_derivation_key(&key, &other));
        assert_ne!(trajectory_derivation_key(&key, &rows), trajectory_derivation_key(&regime("world", "def456"), &rows));
    }

    #[test]
    fn a_spoofed_native_kind_in_s4_detail_does_not_hijack_the_s4_verdict_path() {
        // ReviewP4 regression: the S4 raw-path adapters copy raw
        // artifact JSON verbatim into `detail`, so a raw record could
        // carry a stray `native_kind` field. Dispatch is gated on the
        // adapter-controlled `adapter_id`, NOT the detail tag — so this
        // S4 (`ledger`) event still derives its normal S4 verdict and is
        // never silently dropped into the native (`None`-role) branch.
        let event = ArtifactEvent {
            adapter_id: "ledger",
            join_key: ArtifactJoinKey::Scenario(canon_model::ids::ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap()),
            kind: canon_ingest::artifact_adapter::ArtifactEventKind::CodeReviewFinding,
            authoring_role: None,
            area: Some("world".to_string()),
            trust_level: None,
            at: Utc::now(),
            detail: serde_json::json!({"native_kind": "review"}),
        };
        let via_dispatch = derive_verdict_for_event(&event);
        assert_eq!(
            via_dispatch,
            derive_verdict(canon_ingest::artifact_adapter::ArtifactEventKind::CodeReviewFinding, None),
            "a spoofed `native_kind` in S4 `detail` must NOT hijack dispatch — the frozen S4 table still applies"
        );
        assert!(via_dispatch.is_some(), "the CodeReviewFinding verdict is derived, never silently skipped by the native branch");
    }

    #[test]
    fn a_native_review_event_routes_by_adapter_id_even_with_nonverdict_kind() {
        // The native adapters set `kind = NonVerdict` (derive_verdict
        // would return `None`), so routing MUST come from `adapter_id`,
        // not `kind`/`detail`: a `review`-adapter event still derives a
        // native verdict whose role is the record's own actor.role.
        let role = canon_model::ids::RoleId::parse("content").unwrap();
        let event = ArtifactEvent {
            adapter_id: "review",
            join_key: ArtifactJoinKey::Scenario(canon_model::ids::ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap()),
            kind: canon_ingest::artifact_adapter::ArtifactEventKind::NonVerdict,
            authoring_role: Some(role.clone()),
            area: Some("world".to_string()),
            trust_level: None,
            at: Utc::now(),
            detail: serde_json::json!({"native_kind": "review"}),
        };
        let row = derive_verdict_for_event(&event).expect("a native review event derives a verdict via adapter_id routing");
        assert_eq!(row, derive_native_review_verdict(&role), "role is the record's actor.role, routed by adapter_id despite NonVerdict kind");
    }

    /// One `DerivedVerdict` built the way [`run`] builds it — through a
    /// real `ArtifactEvent`, so these tests exercise the actual
    /// `join_key`/`display_label`/`evidence_line` accessors rather than
    /// hand-written strings.
    fn derived_with(adapter_id: &'static str, regime: &RegimeKey, join_key: &str, kind: Kind, detail: serde_json::Value, at: &str) -> DerivedVerdict {
        let event = ArtifactEvent {
            adapter_id,
            join_key: ArtifactJoinKey::Scenario(canon_model::ids::ScenarioId::parse(join_key.to_string()).unwrap()),
            kind,
            authoring_role: Some(canon_model::ids::RoleId::parse("dev").unwrap()),
            area: Some("world".to_string()),
            trust_level: None,
            at: at.parse().unwrap(),
            detail,
        };
        DerivedVerdict {
            regime_key: regime.clone(),
            row: verdict_row("dev", canon_ingest::verdict::Polarity::Failure, canon_ingest::verdict::Becomes::GuardrailCandidate),
            at: event.at,
            join_key: event.join_key.as_str().to_string(),
            kind_label: event.display_label(),
            evidence_line: event.evidence_line(),
            scenario_title: None,
            antecedents: Vec::new(),
        }
    }

    fn derived(regime: &RegimeKey, join_key: &str, kind: Kind, detail: serde_json::Value, at: &str) -> DerivedVerdict {
        derived_with("ledger", regime, join_key, kind, detail, at)
    }

    fn finding(text: &str) -> serde_json::Value {
        serde_json::json!({"detail": text})
    }

    #[test]
    fn a_regime_groups_every_verdict_and_keeps_the_newest_timestamp() {
        let key = regime("world", "abc123");
        let scenario = "world.firstbuy-hotdeal.14";
        let group = group_by_regime(vec![
            derived(&key, scenario, Kind::CodeReviewFinding, finding("reveal reads the client cart"), "2026-07-08T20:05:49Z"),
            derived(&key, scenario, Kind::RemediationResolved, finding("re-read the grant from the result"), "2026-07-09T10:15:00Z"),
        ]);
        let evidence = group.get(&key).expect("both verdicts fold onto the one regime");
        assert_eq!(evidence.rows.len(), 2);
        assert_eq!(evidence.latest_at, "2026-07-09T10:15:00Z".parse::<DateTime<Utc>>().unwrap(), "recorded_at is the newest source record");
    }

    #[test]
    fn trajectory_text_names_the_artifact_with_deduped_labels_and_evidence_in_first_seen_order() {
        // The whole point of s38-evidence-bearing-memory: the TITLE
        // names a real scenario and what happened to it, and the CONTENT
        // quotes the reviewer's actual findings — no describing the
        // ingest driver, no `detail` blob, no alphabetical re-sorting of
        // a narrative.
        let key = regime("world", "abc123");
        let scenario = "world.firstbuy-hotdeal.14";
        let group = group_by_regime(vec![
            derived(&key, scenario, Kind::CodeReviewFinding, finding("reveal reads the client cart"), "2026-07-08T20:05:49Z"),
            // Same kind AND same prose as the first: contributes neither
            // a duplicate label nor a duplicate line.
            derived(&key, scenario, Kind::CodeReviewFinding, finding("reveal reads the client cart"), "2026-07-08T21:00:00Z"),
            derived(&key, scenario, Kind::RemediationResolved, finding("re-read the grant from the result"), "2026-07-09T10:15:00Z"),
        ]);
        let (task, context) = group.get(&key).unwrap().trajectory_text(&key);
        assert_eq!(task, "world.firstbuy-hotdeal.14: code-review finding, remediation resolved");
        assert_eq!(context, "code-review finding: reveal reads the client cart\nremediation resolved: re-read the grant from the result");
        assert!(
            !context.contains("canon ingest artifacts") && !context.contains("VerdictRow"),
            "a strategy's content never describes the ingest driver: {context}"
        );
    }

    #[test]
    fn trajectory_text_is_byte_identical_across_two_passes_over_the_same_events() {
        // `trajectory_content_digest`'s duplicate skip only fires if the
        // text a second ingest pass derives matches the first exactly.
        let key = regime("world", "abc123");
        let scenario = "world.firstbuy-hotdeal.14";
        let events = || {
            vec![
                derived(&key, scenario, Kind::CodeReviewFinding, finding("reveal reads the client cart"), "2026-07-08T20:05:49Z"),
                derived(&key, scenario, Kind::ClearAfterFlagged, serde_json::json!({"pin": "9c93d024b"}), "2026-07-09T09:00:00Z"),
            ]
        };
        let first = group_by_regime(events());
        let second = group_by_regime(events());
        assert_eq!(first.get(&key).unwrap().trajectory_text(&key), second.get(&key).unwrap().trajectory_text(&key));
    }

    #[test]
    fn trajectory_text_degrades_to_the_regime_key_rather_than_printing_a_wrong_artifact_id() {
        // Unreachable by construction (`regime_hash` IS the join key's
        // digest, and `absorb` debug-asserts it), so this exercises the
        // release-build degrade directly: a group that somehow spanned
        // two keys names the regime, never one arbitrary member's id.
        let key = regime("world", "abc123");
        let mut evidence = RegimeEvidence::default();
        evidence.absorb(derived(&key, "world.firstbuy-hotdeal.14", Kind::CodeReviewFinding, finding("a finding"), "2026-07-08T20:05:49Z"));
        evidence.join_key = None;
        let (task, _) = evidence.trajectory_text(&key);
        assert_eq!(task, format!("{}: code-review finding", key.as_str()));
    }

    #[test]
    fn push_first_seen_dedupes_without_re_sorting() {
        let mut seen = Vec::new();
        for item in ["remediation resolved", "code-review finding", "remediation resolved"] {
            push_first_seen(&mut seen, item);
        }
        assert_eq!(seen, vec!["remediation resolved", "code-review finding"]);
    }

    #[test]
    fn a_native_verdict_regime_titles_itself_by_the_record_never_non_verdict() {
        // The shape `canon retrieve` returns for a repo configured
        // `artifacts.native_records: true` (this one): the native
        // adapters set `kind = NonVerdict` — their verdicts come from the
        // native derivation path — so labelling off `kind` alone would
        // title every strategy here "non-verdict".
        let key = regime("platformer", "41fdd8c5");
        let native = |status: &str, prose: &str, at: &str| {
            let detail = serde_json::json!({"native_kind": "divergence", "status": status, "detail": prose});
            derived_with("divergence-native", &key, "platformer.session.04", Kind::NonVerdict, detail, at)
        };
        let group = group_by_regime(vec![
            native("still_divergent", "SHIP-BLOCKER App.tsx:45 calls sim.setPaused inside a React updater", "2026-07-14T20:50:34Z"),
            native("resolved", "Fixed and re-verified at 505a668e", "2026-07-14T21:38:36Z"),
        ]);
        let (task, context) = group.get(&key).unwrap().trajectory_text(&key);
        assert_eq!(task, "platformer.session.04: still-divergent divergence, resolved divergence");
        assert!(!task.contains("non-verdict"), "a record that DID score must not be titled non-verdict: {task}");
        assert_eq!(
            context,
            "still-divergent divergence: SHIP-BLOCKER App.tsx:45 calls sim.setPaused inside a React updater\nresolved divergence: Fixed and re-verified at 505a668e"
        );
    }

    /// The two real `SHIP-BLOCKER` findings canon's own divergence
    /// corpus carries on `platformer.session.04`, and the resolution
    /// record that closed them — the exact shape s39
    /// (`joined-evidence-grounding`) exists to distill as ONE strategy.
    const APP_FINDING: &str = "SHIP-BLOCKER examples/platformer/src/App.tsx:45-46 calls sim.setPaused() inside a React updater";
    const SIM_FINDING: &str = "SHIP-BLOCKER examples/platformer/src/engine/simulation.ts:151 clears jumpBuffered after consuming it";
    const RESOLUTION: &str = "Fixed and independently re-verified+attested at 505a668e";

    /// One `ArtifactEvent` shaped the way the `divergence-native`
    /// adapter emits them. `status` drives BOTH the display label and
    /// whether a verdict is derived at all (`open` derives none), which
    /// is the exact split the antecedent join keys off; `prose: None`
    /// produces the label-only event that must never be absorbed.
    fn native_divergence(join_key: &str, status: &str, prose: Option<&str>, at: &str) -> ArtifactEvent {
        let mut detail = serde_json::json!({"native_kind": "divergence", "status": status});
        if let Some(text) = prose {
            detail["detail"] = serde_json::json!(text);
        }
        ArtifactEvent {
            adapter_id: "divergence-native",
            join_key: ArtifactJoinKey::Scenario(canon_model::ids::ScenarioId::parse(join_key.to_string()).unwrap()),
            kind: Kind::NonVerdict,
            authoring_role: Some(canon_model::ids::RoleId::parse("dev").unwrap()),
            area: Some("platformer".to_string()),
            trust_level: None,
            at: at.parse().unwrap(),
            detail,
        }
    }

    /// [`run`]'s own per-event accumulation step minus the live store —
    /// the same `join_key`/`display_label`/`evidence_line`/antecedent
    /// wiring, so these tests exercise the shipped join rather than
    /// hand-written strings.
    fn derived_from(index: &AntecedentIndex, regime: &RegimeKey, event: &ArtifactEvent) -> DerivedVerdict {
        DerivedVerdict {
            regime_key: regime.clone(),
            row: verdict_row("dev", canon_ingest::verdict::Polarity::Success, canon_ingest::verdict::Becomes::StrategyCandidate),
            at: event.at,
            join_key: event.join_key.as_str().to_string(),
            scenario_title: None,
            kind_label: event.display_label(),
            evidence_line: event.evidence_line(),
            antecedents: antecedents_before(index, &event_identity(event), event.at),
        }
    }

    #[test]
    fn a_resolved_divergence_distills_together_with_the_open_findings_it_closed() {
        let key = regime("platformer", "41fdd8c5");
        let scenario = "platformer.session.04";
        let events = vec![
            native_divergence(scenario, "open", Some(APP_FINDING), "2026-07-14T20:50:34Z"),
            native_divergence(scenario, "open", Some(SIM_FINDING), "2026-07-14T20:51:02Z"),
            native_divergence(scenario, "resolved", Some(RESOLUTION), "2026-07-14T21:38:36Z"),
        ];
        // s39 changes nothing about verdict derivation: the two findings
        // still score no verdict of their own, exactly as
        // `derive_native_divergence_verdict` maps `Open => None`.
        assert!(derive_verdict_for_event(&events[0]).is_none() && derive_verdict_for_event(&events[1]).is_none());

        let index = index_antecedents(&events);
        let group = group_by_regime(vec![derived_from(&index, &key, &events[2])]);
        let (_, context) = group.get(&key).unwrap().trajectory_text(&key);
        assert_eq!(
            context,
            format!("resolved divergence: {RESOLUTION}\npreceded on this artifact by:\n  - open divergence: {APP_FINDING}\n  - open divergence: {SIM_FINDING}"),
            "the outcome AND both findings it closed, the findings visibly marked as antecedents"
        );
    }

    #[test]
    fn a_finding_on_a_different_artifact_is_never_absorbed() {
        // The join key is the whole join. A concurrent finding on a
        // sibling scenario is somebody else's problem, and quoting it
        // here would attribute it to this resolution.
        let key = regime("platformer", "41fdd8c5");
        let events = vec![
            native_divergence("platformer.session.05", "open", Some(APP_FINDING), "2026-07-14T20:50:34Z"),
            native_divergence("platformer.session.04", "resolved", Some(RESOLUTION), "2026-07-14T21:38:36Z"),
        ];
        let index = index_antecedents(&events);
        let group = group_by_regime(vec![derived_from(&index, &key, &events[1])]);
        let (_, context) = group.get(&key).unwrap().trajectory_text(&key);
        assert_eq!(context, format!("resolved divergence: {RESOLUTION}"));
        assert!(!context.contains("preceded"), "no heading at all when nothing attached: {context}");
    }

    #[test]
    fn a_finding_filed_after_the_resolution_is_not_absorbed_by_it() {
        let key = regime("platformer", "41fdd8c5");
        let scenario = "platformer.session.04";
        let resolution = native_divergence(scenario, "resolved", Some(RESOLUTION), "2026-07-14T21:38:36Z");
        let later = native_divergence(scenario, "open", Some(APP_FINDING), "2026-07-15T09:00:00Z");
        let index = index_antecedents(&[resolution.clone(), later]);
        let group = group_by_regime(vec![derived_from(&index, &key, &resolution)]);
        let (_, context) = group.get(&key).unwrap().trajectory_text(&key);
        assert_eq!(context, format!("resolved divergence: {RESOLUTION}"), "a later finding is a still-open problem this resolution did not fix");

        // The boundary is INCLUSIVE: a finding and its remediation
        // recorded in one batch share a timestamp, and that pairing is
        // exactly what this join exists for.
        let same_instant = native_divergence(scenario, "open", Some(APP_FINDING), "2026-07-14T21:38:36Z");
        let index = index_antecedents(&[resolution.clone(), same_instant]);
        let group = group_by_regime(vec![derived_from(&index, &key, &resolution)]);
        let (_, context) = group.get(&key).unwrap().trajectory_text(&key);
        assert!(context.contains(APP_FINDING), "an equal timestamp still attaches: {context}");
    }

    #[test]
    fn a_prose_free_non_verdict_event_contributes_no_antecedent() {
        let key = regime("platformer", "41fdd8c5");
        let scenario = "platformer.session.04";
        let bare = native_divergence(scenario, "open", None, "2026-07-14T20:50:34Z");
        assert_eq!(bare.evidence_line(), "open divergence", "the label-only shape the prose filter exists for");

        let events = vec![bare, native_divergence(scenario, "resolved", Some(RESOLUTION), "2026-07-14T21:38:36Z")];
        let index = index_antecedents(&events);
        assert!(index.is_empty(), "a bare kind label is canon's own vocabulary, not evidence — it is never indexed");
        let group = group_by_regime(vec![derived_from(&index, &key, &events[1])]);
        let (_, context) = group.get(&key).unwrap().trajectory_text(&key);
        assert_eq!(context, format!("resolved divergence: {RESOLUTION}"));
    }

    #[test]
    fn a_finding_repeated_across_records_contributes_one_antecedent_line() {
        let key = regime("platformer", "41fdd8c5");
        let scenario = "platformer.session.04";
        let events = vec![
            native_divergence(scenario, "open", Some(APP_FINDING), "2026-07-14T20:50:34Z"),
            // The same finding re-filed in a later review round.
            native_divergence(scenario, "open", Some(APP_FINDING), "2026-07-14T21:00:00Z"),
            native_divergence(scenario, "resolved", Some(RESOLUTION), "2026-07-14T21:38:36Z"),
        ];
        let index = index_antecedents(&events);
        assert_eq!(index[&format!("scenario:{scenario}@-")].len(), 1, "deduped by line text at index time, earliest copy kept");
        let grouped = group_by_regime(vec![derived_from(&index, &key, &events[2])]);
        let evidence = grouped.get(&key).unwrap();
        assert_eq!(evidence.antecedent_lines.len(), 1);
        assert!(evidence.antecedents_omitted.is_empty(), "a dedupe is not a cap drop and must never be reported as one");
    }

    #[test]
    fn the_antecedent_block_is_capped_by_line_count_and_says_how_many_it_dropped() {
        let key = regime("platformer", "41fdd8c5");
        let scenario = "platformer.session.04";
        let mut events: Vec<ArtifactEvent> =
            (0..7).map(|n| native_divergence(scenario, "open", Some(&format!("SHIP-BLOCKER finding {n}")), &format!("2026-07-14T20:5{n}:00Z"))).collect();
        events.push(native_divergence(scenario, "resolved", Some(RESOLUTION), "2026-07-14T21:38:36Z"));

        let index = index_antecedents(&events);
        assert_eq!(index[&format!("scenario:{scenario}@-")].len(), 7, "all seven are indexed — the cap is an absorb-time bound on the RENDERED block");
        let grouped = group_by_regime(vec![derived_from(&index, &key, events.last().unwrap())]);
        let evidence = grouped.get(&key).unwrap();
        assert_eq!(evidence.antecedent_lines.len(), MAX_ANTECEDENT_LINES);
        assert_eq!(evidence.antecedents_omitted.len(), 3);

        let (_, context) = evidence.trajectory_text(&key);
        assert!(context.ends_with("\n  (+3 more omitted at the antecedent cap)"), "a capped block says so rather than silently quoting four of seven: {context}");
        assert!(context.contains("finding 3") && !context.contains("finding 4"), "the four EARLIEST survive, in `(at, line)` order: {context}");
    }

    #[test]
    fn the_antecedent_char_budget_binds_before_the_line_cap_does() {
        let key = regime("platformer", "41fdd8c5");
        let scenario = "platformer.session.04";
        // Each renders as `"open divergence: "` plus s38's own 512-char
        // per-line cap, so three of them (~1590 chars) exceed
        // MAX_ANTECEDENT_CHARS while staying under MAX_ANTECEDENT_LINES.
        let mut events: Vec<ArtifactEvent> =
            (0..3).map(|n| native_divergence(scenario, "open", Some(&format!("{n}{}", "x".repeat(600))), &format!("2026-07-14T20:5{n}:00Z"))).collect();
        events.push(native_divergence(scenario, "resolved", Some(RESOLUTION), "2026-07-14T21:38:36Z"));

        let index = index_antecedents(&events);
        let grouped = group_by_regime(vec![derived_from(&index, &key, events.last().unwrap())]);
        let evidence = grouped.get(&key).unwrap();
        assert_eq!(evidence.antecedent_lines.len(), 2, "two full-cap lines fit the budget, the third does not");
        assert!(evidence.antecedent_lines.len() < MAX_ANTECEDENT_LINES, "the CHAR budget bound this block, not the line count");
        assert_eq!(evidence.antecedents_omitted.len(), 1);
        assert!(evidence.antecedent_lines.iter().map(|line| line.chars().count()).sum::<usize>() <= MAX_ANTECEDENT_CHARS);
        assert!(
            evidence.antecedent_lines.iter().all(|line| line.starts_with("open divergence: ")),
            "dropping is whole-line: the block budget never cuts a finding in half"
        );
    }

    #[test]
    fn trajectory_text_is_byte_identical_across_two_passes_with_antecedents_present() {
        // s38's idempotence guarantee has to survive the s39 join:
        // `trajectory_content_digest` now folds the rendered text, so a
        // trajectory whose text wobbled between passes would be written
        // afresh every pass — an ever-growing corpus of near-identical
        // rows in `.canon/learn`, each distilled beside the others.
        let key = regime("platformer", "41fdd8c5");
        let scenario = "platformer.session.04";
        let pass = || {
            // Deliberately handed over in an order that is NOT time
            // order, so a scan-order-dependent render shows up here.
            let events = vec![
                native_divergence(scenario, "open", Some(SIM_FINDING), "2026-07-14T20:51:02Z"),
                native_divergence(scenario, "open", Some(APP_FINDING), "2026-07-14T20:50:34Z"),
                native_divergence(scenario, "resolved", Some(RESOLUTION), "2026-07-14T21:38:36Z"),
            ];
            let index = index_antecedents(&events);
            let grouped = group_by_regime(vec![derived_from(&index, &key, &events[2])]);
            grouped.get(&key).unwrap().trajectory_text(&key)
        };
        let first = pass();
        assert_eq!(first, pass(), "two passes over the same events must derive byte-identical task/context");

        let (_, context) = first;
        assert!(context.contains(APP_FINDING) && context.contains(SIM_FINDING), "both antecedents are present: {context}");
        assert!(
            context.find(APP_FINDING) < context.find(SIM_FINDING),
            "antecedents render in `(at, line)` order, never the adapter scan order they arrived in: {context}"
        );
    }

    // ── s39 `joined-evidence-grounding`: the scenario-title join ──

    /// One `Scenario` ledger record shaped exactly like the ones in this
    /// repo's `.canon/ledger/kind=scenario` — a bare `RawRecord`, which
    /// is what the tier read hands over.
    fn scenario_record(project_id: &str, scenario_id: &str, title: &str, at: &str) -> RawRecord {
        RawRecord(serde_json::json!({
            "actor": {"agent_id": "canon-inventory-sync"},
            "at": at,
            "kind": "scenario",
            "project_id": project_id,
            "scenario_id": scenario_id,
            "schema": 1,
            "title": title,
        }))
    }

    /// The one real record this repo's own corpus carries for
    /// `platformer.moving.01`, verbatim.
    fn moving_platform_index() -> ScenarioTitleIndex {
        ScenarioTitleIndex::from_records(&[scenario_record(
            "platformer",
            "platformer.moving.01",
            "A moving platform carries the standing player",
            "2026-07-14T19:27:35.016340Z",
        )])
    }

    fn scenario_key(id: &str) -> ArtifactJoinKey {
        ArtifactJoinKey::Scenario(canon_model::ids::ScenarioId::parse(id.to_string()).unwrap())
    }

    /// [`derived_from`] plus [`run`]'s own title resolution — the SAME
    /// [`ScenarioTitleIndex::title_for`] + [`event_project_id`] pair over
    /// a real [`ArtifactEvent`], so these tests exercise the shipped join
    /// rather than a hand-set field. `project_id` is written into
    /// `detail` exactly as the emitting adapters write it; omitting it is
    /// the shape that drives `title_for`'s single-project fallback. The
    /// antecedent index is empty here on purpose: this is the `task` half
    /// of s39, and the `context` half has its own tests above.
    fn derived_joined(
        titles: &ScenarioTitleIndex,
        adapter_id: &'static str,
        regime: &RegimeKey,
        join_key: ArtifactJoinKey,
        project_id: Option<&str>,
        kind: Kind,
        at: &str,
    ) -> DerivedVerdict {
        let mut detail = finding("reveal reads the client cart");
        if let Some(project) = project_id {
            detail["project_id"] = serde_json::Value::String(project.to_string());
        }
        let event = ArtifactEvent {
            adapter_id,
            join_key,
            kind,
            authoring_role: Some(canon_model::ids::RoleId::parse("dev").unwrap()),
            area: Some("platformer".to_string()),
            trust_level: None,
            at: at.parse().unwrap(),
            detail,
        };
        DerivedVerdict {
            scenario_title: titles.title_for(&event.join_key, event_project_id(&event)).map(str::to_string),
            ..derived_from(&AntecedentIndex::new(), regime, &event)
        }
    }

    /// The `task` half of the text a group of exactly these verdicts
    /// renders.
    fn rendered_task(key: &RegimeKey, verdicts: Vec<DerivedVerdict>) -> String {
        group_by_regime(verdicts).get(key).expect("the group is keyed by the regime it was built with").trajectory_text(key).0
    }

    #[test]
    fn a_scenario_keyed_trajectory_names_its_joined_title() {
        // The exact shape s39 exists for: `records::Review` carries no
        // prose field at all, so pre-s39 a review-derived strategy was
        // titled by the scenario id and its kind label — nothing about
        // what was attested. The joined `Scenario.title` is the sentence
        // that fixes it, and the id stays FIRST because it is the join
        // key a reader follows back to the artifact.
        let key = regime("platformer", "41fdd8c5");
        let task = rendered_task(
            &key,
            vec![derived_joined(
                &moving_platform_index(),
                "review",
                &key,
                scenario_key("platformer.moving.01"),
                Some("platformer"),
                Kind::NonVerdict,
                "2026-07-14T21:38:36Z",
            )],
        );
        assert_eq!(task, "platformer.moving.01: review attestation — A moving platform carries the standing player");
    }

    #[test]
    fn an_unknown_scenario_id_renders_the_s38_task_byte_for_byte() {
        let key = regime("platformer", "41fdd8c5");
        let joined = derived_joined(
            &moving_platform_index(),
            "review",
            &key,
            scenario_key("platformer.hud.02"),
            Some("platformer"),
            Kind::NonVerdict,
            "2026-07-14T21:38:36Z",
        );
        assert_eq!(joined.scenario_title, None, "an id the index never saw joins nothing");
        assert_eq!(rendered_task(&key, vec![joined]), "platformer.hud.02: review attestation");
    }

    #[test]
    fn a_non_scenario_join_key_renders_the_s38_task_byte_for_byte() {
        // A handoff- or task-keyed trajectory has no scenario to name, so
        // the join must not merely MISS — it is never attempted, and the
        // rendered text stays indistinguishable from s38's.
        let key = regime("platformer", "41fdd8c5");
        let titles = moving_platform_index();
        let handoff = ArtifactJoinKey::Handoff(canon_model::ids::HandoffId::parse("20260710-1432-fix-a1b2").unwrap());
        let task_key = ArtifactJoinKey::Task(canon_model::ids::TaskId::parse("frozen-fixture-change#1.4").unwrap());
        assert_eq!(titles.title_for(&handoff, Some("platformer")), None);
        assert_eq!(titles.title_for(&task_key, None), None);

        let joined = derived_joined(&titles, "handoff", &key, handoff, Some("platformer"), Kind::PrMergeNoRevert, "2026-07-14T21:38:36Z");
        assert_eq!(rendered_task(&key, vec![joined]), "20260710-1432-fix-a1b2: PR merge with no revert");
    }

    #[test]
    fn an_empty_index_leaves_every_task_byte_identical_to_s38() {
        // The degrade path: a missing/unreadable `canon.yaml`, an
        // unrouted `scenario` kind, and an unreachable routed rung all
        // resolve to `ScenarioTitleIndex::default()` and a NORMAL run —
        // this join can only ever fail to ENRICH.
        let key = regime("platformer", "41fdd8c5");
        let empty = ScenarioTitleIndex::default();
        let joined = derived_joined(&empty, "review", &key, scenario_key("platformer.moving.01"), Some("platformer"), Kind::NonVerdict, "2026-07-14T21:38:36Z");
        assert_eq!(joined.scenario_title, None);
        assert_eq!(rendered_task(&key, vec![joined]), "platformer.moving.01: review attestation");
        assert_eq!(empty.title_for(&scenario_key("platformer.moving.01"), None), None, "and no project-less fallback invents one either");
    }

    #[test]
    fn a_blank_or_missing_title_contributes_no_entry() {
        // A partially-formed record is not evidence: enriching a strategy
        // title with an empty sentence (`<id>: <label> — `) is strictly
        // worse than leaving the s38 text alone.
        let index = ScenarioTitleIndex::from_records(&[
            scenario_record("platformer", "platformer.moving.01", "   \n\t ", "2026-07-14T19:27:35Z"),
            RawRecord(serde_json::json!({"at": "2026-07-14T19:27:35Z", "kind": "scenario", "project_id": "platformer", "scenario_id": "platformer.moving.02"})),
            RawRecord(serde_json::json!({"at": "2026-07-14T19:27:35Z", "kind": "scenario", "scenario_id": "platformer.moving.03", "title": "No project_id at all"})),
        ]);
        assert!(index.titles.is_empty(), "a blank title, an absent title, and an absent project_id each contribute nothing: {index:?}");
    }

    #[test]
    fn a_malformed_at_never_panics_and_never_outranks_a_well_formed_record() {
        // `canon_store::tier::raw_record_at` would PANIC on this record.
        // A hand-edited ledger file must cost a title lookup at most, so
        // the join reads `at` leniently and floors a malformed one.
        let mut broken = scenario_record("platformer", "platformer.moving.01", "Indexed from a record with a broken timestamp", "2026-07-01T09:00:00Z");
        broken.0["at"] = serde_json::Value::String("not-a-timestamp".to_string());
        let good = scenario_record("platformer", "platformer.moving.01", "A moving platform carries the standing player", "2026-07-14T19:27:35Z");

        let broken_only = ScenarioTitleIndex::from_records(&[broken.clone()]);
        assert_eq!(
            broken_only.title_for(&scenario_key("platformer.moving.01"), Some("platformer")),
            Some("Indexed from a record with a broken timestamp"),
            "a broken `at` is not a broken title — the record still indexes"
        );
        let both = ScenarioTitleIndex::from_records(&[broken, good]);
        assert_eq!(
            both.title_for(&scenario_key("platformer.moving.01"), Some("platformer")),
            Some("A moving platform carries the standing player"),
            "the floored timestamp sorts below every real one, so it can never win the slot"
        );
    }

    #[test]
    fn a_retitled_scenario_indexes_its_newest_title_regardless_of_read_order() {
        // The ledger names files `<project>__<scenario>__<digest>`, so a
        // RETITLED scenario lands beside its predecessor rather than
        // replacing it, and a `LiveDb`-class rung retains history
        // outright (s21 P3). Whichever order the tier yields them in, the
        // index must resolve the same current title — otherwise two
        // passes over an unchanged corpus could derive different `task`
        // strings and defeat the duplicate skip.
        let old = scenario_record("platformer", "platformer.moving.01", "An older sentence", "2026-07-01T09:00:00Z");
        let new = scenario_record("platformer", "platformer.moving.01", "A moving platform carries the standing player", "2026-07-14T19:27:35Z");
        let forward = ScenarioTitleIndex::from_records(&[old.clone(), new.clone()]);
        let reversed = ScenarioTitleIndex::from_records(&[new, old]);
        let key = scenario_key("platformer.moving.01");
        assert_eq!(forward.title_for(&key, Some("platformer")), Some("A moving platform carries the standing player"));
        assert_eq!(forward.title_for(&key, Some("platformer")), reversed.title_for(&key, Some("platformer")));
    }

    #[test]
    fn one_scenario_id_defined_by_two_projects_resolves_only_when_the_event_names_its_project() {
        // `ArtifactJoinKey::Scenario` carries the scenario id ALONE, so a
        // project-less event falls back to the id — honest only while
        // exactly one project defines it. With two, printing either
        // sentence would attribute one project's spec to another
        // project's strategy memory.
        let index = ScenarioTitleIndex::from_records(&[
            scenario_record("platformer", "platformer.moving.01", "A moving platform carries the standing player", "2026-07-14T19:27:35Z"),
            scenario_record("world", "platformer.moving.01", "A wholly different sentence", "2026-07-14T19:27:35Z"),
        ]);
        let key = scenario_key("platformer.moving.01");
        assert_eq!(index.title_for(&key, Some("world")), Some("A wholly different sentence"));
        assert_eq!(index.title_for(&key, None), None, "ambiguous without a project: name nothing rather than the wrong sentence");
        assert_eq!(index.title_for(&key, Some("absent-project")), None);
    }

    #[test]
    fn a_pathological_title_is_capped_before_it_reaches_a_strategy_title() {
        // Several distilled strategies are injected into a dispatched
        // agent's context at once, so one fat record must not be able to
        // bloat every retrieval that touches its regime. Multi-byte prose
        // (canon's corpora carry Korean) also proves the cap counts CHARS
        // — a byte cut would split a codepoint and panic in `truncate`.
        let fat = "가".repeat(SCENARIO_TITLE_MAX_CHARS + 40);
        let index = ScenarioTitleIndex::from_records(&[scenario_record("platformer", "platformer.moving.01", &fat, "2026-07-14T19:27:35Z")]);
        let key = regime("platformer", "41fdd8c5");
        let title = index.title_for(&scenario_key("platformer.moving.01"), Some("platformer")).expect("a long title is capped, never dropped");
        assert_eq!(title.chars().count(), SCENARIO_TITLE_MAX_CHARS + 1, "the cap plus its one-char cut marker");
        assert!(title.ends_with('…'), "a cut is marked, never silent");

        let joined = derived_joined(&index, "review", &key, scenario_key("platformer.moving.01"), Some("platformer"), Kind::NonVerdict, "2026-07-14T21:38:36Z");
        let task = rendered_task(&key, vec![joined]);
        assert!(
            task.chars().count() <= "platformer.moving.01: review attestation — ".chars().count() + SCENARIO_TITLE_MAX_CHARS + 1,
            "the cap bounds the RENDERED task, not just the index: {task}"
        );
    }

    #[test]
    fn a_joined_title_is_byte_identical_across_two_passes_over_the_same_events() {
        // s38's idempotence guarantee has to survive this join too:
        // `trajectory_content_digest` folds the rendered text, so a title
        // that wobbled between passes would supersede its own row on
        // every ingest instead of skipping as a duplicate.
        let key = regime("platformer", "41fdd8c5");
        let titles = moving_platform_index();
        let pass = || {
            vec![
                derived_joined(&titles, "review", &key, scenario_key("platformer.moving.01"), Some("platformer"), Kind::NonVerdict, "2026-07-14T21:38:36Z"),
                // No `project_id` in `detail`: resolves through the
                // single-project fallback to the SAME title, so the
                // rendered title never depends on absorb order.
                derived_joined(&titles, "ledger", &key, scenario_key("platformer.moving.01"), None, Kind::CodeReviewFinding, "2026-07-14T22:00:00Z"),
            ]
        };
        assert_eq!(rendered_task(&key, pass()), rendered_task(&key, pass()));
        assert_eq!(
            rendered_task(&key, pass()),
            "platformer.moving.01: review attestation, code-review finding — A moving platform carries the standing player"
        );
    }

    // ── ReviewCore: identity, supersession, and total order ──

    /// The exact shape a PRE-s38 `canon ingest artifacts` pass left in
    /// a store: this release's `regime_key` and its ordered verdict
    /// rows, under text that described the DRIVER rather than the
    /// evidence.
    ///
    /// `id` is a parameter because the canonical-row rule keys on the
    /// SMALLEST id: a test seeding two of these has to pin which one is
    /// canonical rather than leave it to `TrajectoryId::new()`'s clock.
    fn pre_s38_trajectory(id: TrajectoryId, key: &RegimeKey, rows: Vec<VerdictRow>) -> Trajectory {
        Trajectory::new(
            id,
            key.clone(),
            format!("{} verdict(s) derived from canon-ingest artifact adapters for regime {}", rows.len(), key.as_str()),
            format!("{} verdict(s) derived from canon-ingest artifact adapters", rows.len()),
            rows,
            "2026-07-14T21:38:36Z".parse().unwrap(),
            vec![ARTIFACT_INGEST_TAG.to_string()],
        )
        .expect("a dev-role trajectory under a dev regime")
    }

    #[test]
    fn an_upgraded_store_converges_onto_the_new_text_without_deleting_anything() {
        // The BLOCKER, end to end over a REAL store. `rm -rf
        // .canon/learn` was the development workaround for this; it is
        // not the contract, so an upgrade has to converge on its own.
        let dir = tempfile::tempdir().unwrap();
        let store = ParquetTrajectoryStore::open(dir.path().join("trajectories"));
        let key = regime("platformer", "41fdd8c5");
        let by_regime = resolved_divergence_evidence(&key);
        let evidence = by_regime.get(&key).expect("the group is keyed by the regime it was built with");

        let stale = pre_s38_trajectory(TrajectoryId::new(), &key, evidence.rows.clone());
        let stale_id = stale.id;
        store.append(&stale).unwrap();

        let plan = plan_trajectory(&store.query_by_regime_key(&key).unwrap(), &key, evidence, None).unwrap();
        let PersistPlan::Supersede(fresh) = plan else {
            panic!("a stored pre-s38 derivation of this exact verdict set must be superseded — not skipped as a duplicate, not appended beside")
        };
        assert_eq!(fresh.id, stale_id, "supersession reuses the stored row's own id: that is what makes the write REPLACE it");
        store.append(&fresh).unwrap();

        let after = store.query_by_regime_key(&key).unwrap();
        assert_eq!(after.len(), 1, "the namespace converged rather than doubling — a second row would be distilled beside the first: {after:#?}");
        assert!(
            after[0].context.contains(RESOLUTION) && after[0].context.contains(APP_FINDING),
            "the s38/s39 evidence text replaced the driver-describing prose: {}",
            after[0].context
        );
        assert!(
            !after[0].task.contains("canon-ingest artifact adapters") && !after[0].context.contains("canon-ingest artifact adapters"),
            "no pre-s38 prose survives to be re-distilled: {:?}",
            (&after[0].task, &after[0].context)
        );

        // …and the pass AFTER that writes nothing at all: supersession
        // is a one-time upgrade, never a rewrite on every ingest.
        assert!(matches!(plan_trajectory(&after, &key, evidence, None).unwrap(), PersistPlan::SkipDuplicate));
    }

    /// s42 (`close-the-open-loops`) task 3.2: a pass told which run it is
    /// executing inside stamps that run onto the trajectory it writes, and
    /// a pass told nothing stamps nothing. The absent case is the one that
    /// matters most — it is what makes the field a claim rather than
    /// decoration.
    #[test]
    fn a_dispatched_run_is_stamped_onto_the_written_trajectory_and_only_when_given() {
        let key = regime("platformer", "41fdd8c5");
        let by_regime = resolved_divergence_evidence(&key);
        let evidence = by_regime.get(&key).expect("the group is keyed by the regime it was built with");
        let run_id = RunId::new();

        let PersistPlan::Fresh(attributed) = plan_trajectory(&[], &key, evidence, Some(&run_id)).unwrap() else {
            panic!("an empty namespace mints a fresh row")
        };
        assert_eq!(attributed.run_id, Some(run_id));

        let PersistPlan::Fresh(unattributed) = plan_trajectory(&[], &key, evidence, None).unwrap() else {
            panic!("an empty namespace mints a fresh row")
        };
        assert_eq!(unattributed.run_id, None, "an ingest outside a dispatch must record no run, never a guessed one");
    }

    /// Attribution must not perturb write-time idempotence. If `run_id`
    /// reached [`trajectory_content_digest`], a dispatched pass over an
    /// UNCHANGED corpus would derive a fresh identity for evidence it had
    /// already persisted and rewrite the whole namespace on every
    /// dispatch — and a pass under a DIFFERENT run would keep doing so
    /// forever. Both digests are asserted directly, because that is where
    /// the mistake would live; the plan's own decision is asserted too,
    /// because that is what a reader cares about.
    #[test]
    fn attribution_is_outside_both_write_identity_digests() {
        let key = regime("platformer", "41fdd8c5");
        let by_regime = resolved_divergence_evidence(&key);
        let evidence = by_regime.get(&key).expect("the group is keyed by the regime it was built with");

        let PersistPlan::Fresh(stored) = plan_trajectory(&[], &key, evidence, Some(&RunId::new())).unwrap() else {
            panic!("an empty namespace mints a fresh row")
        };
        let unattributed = stored.clone().with_run_id(None);

        let digest = |t: &Trajectory| trajectory_content_digest(&t.regime_key, &t.verdicts, &t.task, &t.context);
        assert_eq!(digest(&stored), digest(&unattributed), "the full identity must not see run_id");
        assert_eq!(
            trajectory_derivation_key(&stored.regime_key, &stored.verdicts),
            trajectory_derivation_key(&unattributed.regime_key, &unattributed.verdicts),
            "the UNVERSIONED derivation key must not see run_id either — widening it would orphan every upgrading user's rows"
        );

        // The consequence, stated as behaviour: a second pass under a
        // DIFFERENT run recognizes the stored row and writes nothing.
        let existing = [stored];
        assert!(
            matches!(plan_trajectory(&existing, &key, evidence, Some(&RunId::new())).unwrap(), PersistPlan::SkipDuplicate),
            "a re-ingest of unchanged evidence under another run must stay a no-op"
        );
    }

    /// `--run` is admitted against this repo's own dispatch manifests, at
    /// the same path `canon dispatch end` resolves a close through. A
    /// well-formed ULID naming no dispatch is a LOUD failure: stamping it
    /// would produce an attribution `mart_flywheel_funnel` drops on the
    /// join, which reads exactly like no attribution at all.
    #[test]
    fn an_unknown_run_is_rejected_and_an_absent_flag_is_not() {
        let dir = tempfile::tempdir().unwrap();
        let unknown = RunId::new();

        let err = resolve_dispatched_run(dir.path(), Some(&unknown)).expect_err("no manifest exists for this run");
        let ArtifactIngestError::UnknownRun { run_id, path } = &err else { panic!("expected UnknownRun, got {err:?}") };
        assert_eq!(run_id, &unknown.to_string(), "the error names the run the operator typed");
        assert!(path.ends_with(&format!("{unknown}.json")), "and the exact path consulted: {path}");

        resolve_dispatched_run(dir.path(), None).expect("no --run at all is the ordinary, always-admissible case");

        // A manifest that really is this run's dispatch record admits it.
        // A bare `"{}"` used to be enough — the bar was `metadata`, so
        // any entry at that path passed — which is the hole
        // `a_run_manifest_is_admitted_only_after_it_deserializes_as_that_exact_dispatched_run`
        // covers.
        let dispatch_dir = dir.path().join(crate::dispatch::DISPATCH_DIR);
        std::fs::create_dir_all(&dispatch_dir).unwrap();
        std::fs::write(dispatch_dir.join(format!("{unknown}.json")), run_manifest(unknown, RunStatus::Running)).unwrap();
        resolve_dispatched_run(dir.path(), Some(&unknown)).expect("a dispatched run is admissible");
    }

    /// Two ids whose ORDER is pinned rather than left to the clock —
    /// `seeded_id(1) < seeded_id(2)`, so a test can say which row the
    /// canonical rule must pick.
    fn seeded_id(nth: u8) -> TrajectoryId {
        TrajectoryId::parse(&format!("01JZ{}{nth}", "0".repeat(21))).expect("a 26-char Crockford-base32 ULID")
    }

    /// The s39-joined evidence group every supersession test here derives
    /// from: one `divergence-native` resolution carrying its own antecedent.
    fn resolved_divergence_evidence(key: &RegimeKey) -> BTreeMap<RegimeKey, RegimeEvidence> {
        let scenario = "platformer.session.04";
        let events = vec![
            native_divergence(scenario, "open", Some(APP_FINDING), "2026-07-14T20:50:34Z"),
            native_divergence(scenario, "resolved", Some(RESOLUTION), "2026-07-14T21:38:36Z"),
        ];
        let index = index_antecedents(&events);
        group_by_regime(vec![derived_from(&index, key, &events[1])])
    }

    /// A row already carrying EXACTLY what this pass renders — the
    /// sibling whose presence used to short-circuit the whole plan.
    fn current_trajectory(id: TrajectoryId, key: &RegimeKey, evidence: &RegimeEvidence) -> Trajectory {
        let (task, context) = evidence.trajectory_text(key);
        Trajectory::new(id, key.clone(), task, context, evidence.rows.clone(), evidence.latest_at, vec![ARTIFACT_INGEST_TAG.to_string()])
            .expect("a dev-role trajectory under a dev regime")
    }

    /// Both stores read back in `read_dir` order, so every cross-pass
    /// comparison sorts by id first — an unsorted `assert_eq!` would fail
    /// on filesystem ordering rather than on content.
    fn rows_by_id(store: &ParquetTrajectoryStore, key: &RegimeKey) -> Vec<Trajectory> {
        let mut rows = store.query_by_regime_key(key).expect("a real parquet store reads back what it wrote");
        rows.sort_by_key(|row| row.id);
        rows
    }

    fn items_by_id(store: &ParquetStrategyStore, key: &RegimeKey) -> Vec<StrategyItem> {
        let mut items = store.query_by_regime_key(key).expect("a real parquet store reads back what it wrote");
        items.sort_by_key(|item| item.id);
        items
    }

    /// One persist pass of [`run`]'s own loop against a REAL store pair:
    /// plan against what the store actually holds, write only when the
    /// plan says to, then re-derive the distilled tier through
    /// [`rebuild_namespace_converged`] on exactly the branches `run` does.
    /// `append` stands in for `canon_learn::store_trajectory`, which only
    /// adds the role-registry check — identical for these registered-role
    /// rows, and it keeps the test free of a `canon.yaml`.
    fn drive_one_pass(
        trajectory_store: &ParquetTrajectoryStore,
        strategy_store: &ParquetStrategyStore,
        key: &RegimeKey,
        evidence: &RegimeEvidence,
        run_id: Option<&RunId>,
    ) -> PersistPlan {
        let existing = trajectory_store.query_by_regime_key(key).expect("a real parquet store reads back what it wrote");
        let plan = plan_trajectory(&existing, key, evidence, run_id).expect("a dev-role trajectory under a dev regime");
        match &plan {
            PersistPlan::SkipDuplicate => {}
            PersistPlan::ConvergeDistilled => {
                rebuild_namespace_converged(trajectory_store, strategy_store, key).expect("the distilled tier re-derives");
            }
            PersistPlan::Fresh(trajectory) | PersistPlan::Supersede(trajectory) => {
                trajectory_store.append(trajectory).expect("a real parquet store accepts the write");
                rebuild_namespace_converged(trajectory_store, strategy_store, key).expect("the distilled tier re-derives");
            }
        }
        plan
    }

    #[test]
    fn two_stale_copies_of_one_verdict_set_converge_to_one_current_distilled_strategy() {
        // ReReviewCore on s41: the smallest-id rule only ever converged a
        // SINGLE stale row. A pre-s38 pass appended a second row for one
        // verdict set whenever the renderer changed under it (its digest
        // ignored rendered text), and the pass after that rewrote the
        // smallest row and then skipped forever — leaving the sibling's
        // driver-describing prose to be distilled beside the fresh answer.
        let dir = tempfile::tempdir().unwrap();
        let trajectory_store = ParquetTrajectoryStore::open(dir.path().join("trajectories"));
        let strategy_store = ParquetStrategyStore::open(dir.path().join("strategies"));
        let key = regime("platformer", "41fdd8c5");
        let by_regime = resolved_divergence_evidence(&key);
        let evidence = by_regime.get(&key).expect("the group is keyed by the regime it was built with");

        let (canonical_id, losing_id) = (seeded_id(1), seeded_id(2));
        trajectory_store.append(&pre_s38_trajectory(canonical_id, &key, evidence.rows.clone())).unwrap();
        trajectory_store.append(&pre_s38_trajectory(losing_id, &key, evidence.rows.clone())).unwrap();

        let plan = drive_one_pass(&trajectory_store, &strategy_store, &key, evidence, None);
        assert!(
            matches!(&plan, PersistPlan::Supersede(fresh) if fresh.id == canonical_id),
            "the smallest-id row of the candidate set is the one rewritten"
        );

        let rows = rows_by_id(&trajectory_store, &key);
        assert_eq!(
            rows.len(),
            1,
            "the raw layer converges too (s42): the superseded copy is DELETED, not left beside the canonical row — {rows:#?}"
        );
        assert_eq!(rows[0].id, canonical_id, "and the row that survives is the canonical one, the smallest id");
        assert!(
            rows[0].context.contains(RESOLUTION) && rows[0].context.contains(APP_FINDING),
            "the canonical row carries this release's evidence text: {}",
            rows[0].context
        );
        assert!(
            trajectory_store.find_by_id(&losing_id).unwrap().is_none(),
            "the superseded sibling is gone from the store entirely, not merely withheld from the distiller"
        );

        // The distilled tier is the ONLY tier `canon retrieve`,
        // `mart_role_memory` and promotion read, and it converged: one
        // strategy per verdict, sourced from the canonical row, with no
        // pre-s38 prose anywhere in it.
        let distilled = items_by_id(&strategy_store, &key);
        assert_eq!(
            distilled.len(),
            evidence.rows.len(),
            "one distilled strategy per verdict — not one per stored copy of the verdict set: {distilled:#?}"
        );
        for item in &distilled {
            assert_eq!(item.source_trajectory_ids, vec![canonical_id], "every distilled item cites the canonical row");
            assert!(item.content.contains(RESOLUTION), "the distilled content is this release's evidence text: {}", item.content);
            assert!(
                !item.title.contains("canon-ingest artifact adapters") && !item.content.contains("canon-ingest artifact adapters"),
                "no pre-s38 prose reaches a retrieved strategy: {:?}",
                (&item.title, &item.content)
            );
        }

        // Two further passes write nothing, supersede nothing and delete
        // nothing: the canonical row already says this and the namespace
        // holds no superseded sibling any more, so the plan settles on
        // `SkipDuplicate` — the s42 convergence is one-shot, not a rebuild
        // this driver pays on every pass forever.
        for pass in 2..=3 {
            let plan = drive_one_pass(&trajectory_store, &strategy_store, &key, evidence, None);
            assert!(matches!(plan, PersistPlan::SkipDuplicate), "pass {pass} must write no raw row and converge nothing");
            assert_eq!(rows_by_id(&trajectory_store, &key), rows, "pass {pass} perturbed the raw tier");
            assert_eq!(items_by_id(&strategy_store, &key), distilled, "pass {pass} perturbed the distilled tier");
        }
    }

    #[test]
    fn a_sibling_carrying_the_fresh_text_no_longer_shadows_a_stale_canonical_row() {
        // The half of ReReviewCore's finding the duplicate check caused
        // directly: with one of the two copies already current, `any stored
        // row carries this identity` returned before supersession ran, so
        // the STALE row was never rewritten and no rebuild ever ran.
        let dir = tempfile::tempdir().unwrap();
        let trajectory_store = ParquetTrajectoryStore::open(dir.path().join("trajectories"));
        let strategy_store = ParquetStrategyStore::open(dir.path().join("strategies"));
        let key = regime("platformer", "41fdd8c5");
        let by_regime = resolved_divergence_evidence(&key);
        let evidence = by_regime.get(&key).expect("the group is keyed by the regime it was built with");

        let (canonical_id, current_sibling_id) = (seeded_id(1), seeded_id(2));
        trajectory_store.append(&pre_s38_trajectory(canonical_id, &key, evidence.rows.clone())).unwrap();
        trajectory_store.append(&current_trajectory(current_sibling_id, &key, evidence)).unwrap();

        let plan = drive_one_pass(&trajectory_store, &strategy_store, &key, evidence, None);
        assert!(
            matches!(&plan, PersistPlan::Supersede(fresh) if fresh.id == canonical_id),
            "a sibling's current text answers for itself, never for the canonical row"
        );

        let rows = rows_by_id(&trajectory_store, &key);
        assert_eq!(rows.len(), 1, "the current-text sibling is this driver's own superseded copy, so it is deleted too: {rows:#?}");
        assert_eq!(rows[0].id, canonical_id, "the smallest id survives whichever copy happened to hold the fresh text");
        assert!(
            rows[0].context.contains(RESOLUTION),
            "no stale text is left under this verdict set: {}",
            rows[0].context
        );

        let distilled = items_by_id(&strategy_store, &key);
        assert_eq!(
            distilled.len(),
            evidence.rows.len(),
            "the logical verdict set distills ONCE even though two copies of it were stored: {distilled:#?}"
        );
        assert_eq!(distilled[0].source_trajectory_ids, vec![canonical_id]);

        let plan = drive_one_pass(&trajectory_store, &strategy_store, &key, evidence, None);
        assert!(matches!(plan, PersistPlan::SkipDuplicate), "the converged store persists, supersedes and deletes nothing");
        assert_eq!(rows_by_id(&trajectory_store, &key), rows);
        assert_eq!(items_by_id(&strategy_store, &key), distilled);
    }

    #[test]
    fn an_already_distilled_stale_sibling_is_deleted_and_leaves_the_strategy_tier_with_no_raw_write() {
        // The seeding where nothing needs WRITING and everything still needs
        // converging: the canonical row is already current (a store whose
        // ids are not in append order, or one an earlier pass already
        // rewrote), while a stale sibling's strategy is ALREADY sitting in
        // the distilled tier. The pre-fix skip returned before any rebuild,
        // so that strategy was served until the corpus happened to change.
        let dir = tempfile::tempdir().unwrap();
        let trajectory_store = ParquetTrajectoryStore::open(dir.path().join("trajectories"));
        let strategy_store = ParquetStrategyStore::open(dir.path().join("strategies"));
        let key = regime("platformer", "41fdd8c5");
        let by_regime = resolved_divergence_evidence(&key);
        let evidence = by_regime.get(&key).expect("the group is keyed by the regime it was built with");

        let (canonical_id, stale_sibling_id) = (seeded_id(1), seeded_id(2));
        trajectory_store.append(&current_trajectory(canonical_id, &key, evidence)).unwrap();
        trajectory_store.append(&pre_s38_trajectory(stale_sibling_id, &key, evidence.rows.clone())).unwrap();

        // What an earlier canon's unfiltered rebuild left behind: BOTH rows
        // distilled, side by side.
        let rows_before = rows_by_id(&trajectory_store, &key);
        for item in distill_namespace(&key, &rows_before) {
            strategy_store.append(&item).unwrap();
        }
        assert_eq!(items_by_id(&strategy_store, &key).len(), 2, "the tier starts polluted, which is the whole premise");

        let plan = drive_one_pass(&trajectory_store, &strategy_store, &key, evidence, None);
        assert!(matches!(plan, PersistPlan::ConvergeDistilled), "there is nothing to write — and still something to converge");
        assert_eq!(
            rows_by_id(&trajectory_store, &key),
            vec![rows_before[0].clone()],
            "the stale sibling is deleted; the canonical row's own bytes are neither written nor rewritten on this path"
        );

        let distilled = items_by_id(&strategy_store, &key);
        assert_eq!(distilled.len(), evidence.rows.len(), "the stale sibling's strategy left the tier: {distilled:#?}");
        assert_eq!(distilled[0].source_trajectory_ids, vec![canonical_id]);
        assert!(
            !distilled[0].title.contains("canon-ingest artifact adapters") && !distilled[0].content.contains("canon-ingest artifact adapters"),
            "no pre-s38 prose is retrievable any more: {:?}",
            (&distilled[0].title, &distilled[0].content)
        );
    }

    #[test]
    fn convergence_never_deletes_or_withholds_a_row_this_driver_did_not_write() {
        // The filter's boundary, and since s42 (`close-the-open-loops`) the
        // boundary of an IRREVERSIBLE operation: convergence touches only
        // THIS driver's own non-canonical copies. An untagged row sharing
        // the verdict set is someone else's evidence — never rewritten (the
        // plan's rung 2), never dropped from the distilled tier, and never
        // deleted from the raw one.
        let dir = tempfile::tempdir().unwrap();
        let trajectory_store = ParquetTrajectoryStore::open(dir.path().join("trajectories"));
        let strategy_store = ParquetStrategyStore::open(dir.path().join("strategies"));
        let key = regime("platformer", "41fdd8c5");
        let by_regime = resolved_divergence_evidence(&key);
        let evidence = by_regime.get(&key).expect("the group is keyed by the regime it was built with");

        let (canonical_id, losing_id) = (seeded_id(1), seeded_id(2));
        trajectory_store.append(&pre_s38_trajectory(canonical_id, &key, evidence.rows.clone())).unwrap();
        trajectory_store.append(&pre_s38_trajectory(losing_id, &key, evidence.rows.clone())).unwrap();
        let foreign = Trajectory::new(
            TrajectoryId::new(),
            key.clone(),
            "seeded by a fixture",
            "prose this driver never wrote",
            evidence.rows.clone(),
            "2026-07-14T21:38:36Z".parse().unwrap(),
            vec!["fixture".to_string()],
        )
        .unwrap();
        trajectory_store.append(&foreign).unwrap();

        drive_one_pass(&trajectory_store, &strategy_store, &key, evidence, None);

        let distilled = items_by_id(&strategy_store, &key);
        assert_eq!(distilled.len(), 2, "one strategy for the canonical row and one for the foreign row, and nothing else: {distilled:#?}");
        let sources: Vec<&TrajectoryId> = distilled.iter().flat_map(|item| &item.source_trajectory_ids).collect();
        assert!(sources.contains(&&foreign.id), "the untagged row still distills: {distilled:#?}");
        assert!(sources.contains(&&canonical_id), "the canonical row still distills: {distilled:#?}");
        assert!(!sources.contains(&&losing_id), "only this driver's superseded copy is withheld: {distilled:#?}");

        let rows = rows_by_id(&trajectory_store, &key);
        assert!(rows.contains(&foreign), "the untagged row's own bytes survive the pass byte for byte: {rows:#?}");
        assert!(
            !rows.iter().any(|row| row.id == losing_id),
            "…while this driver's own superseded copy is deleted, so the two outcomes are genuinely distinguished: {rows:#?}"
        );
    }

    /// Every `*.parquet` file under a store root, as `(path, bytes)`
    /// sorted by path. The RAW-TIER observable s42
    /// (`close-the-open-loops`) task 2.3 is about: decoded rows cannot
    /// tell a namespace that DELETED its duplicates from one that merely
    /// withheld them from the distiller, and neither can a distilled-item
    /// count. Files can.
    fn parquet_files(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut out = Vec::new();
        collect_parquet_files(root, &mut out);
        out.sort_by(|left, right| left.0.cmp(&right.0));
        out
    }

    fn collect_parquet_files(dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries {
            let path = entry.expect("a readable tempdir entry").path();
            if path.is_dir() {
                collect_parquet_files(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "parquet") {
                let bytes = std::fs::read(&path).expect("a readable parquet file");
                out.push((path, bytes));
            }
        }
    }

    #[test]
    fn two_stored_copies_of_one_verdict_set_converge_to_exactly_one_raw_file() {
        // s42 (`close-the-open-loops`) task 2.3. s41 converged every tier a
        // reader consumes while the raw layer kept N files for N copies
        // forever, because `TrajectoryStore` had no deletion primitive at
        // all. The observable that separates "withheld" from "removed" is
        // the file count, not the distilled-item count — so this asserts on
        // files, against a real `ParquetTrajectoryStore`.
        let dir = tempfile::tempdir().unwrap();
        let traj_root = dir.path().join("trajectories");
        let trajectory_store = ParquetTrajectoryStore::open(&traj_root);
        let strategy_store = ParquetStrategyStore::open(dir.path().join("strategies"));
        let key = regime("platformer", "41fdd8c5");
        let by_regime = resolved_divergence_evidence(&key);
        let evidence = by_regime.get(&key).expect("the group is keyed by the regime it was built with");

        let (canonical_id, losing_id) = (seeded_id(1), seeded_id(2));
        trajectory_store.append(&pre_s38_trajectory(canonical_id, &key, evidence.rows.clone())).unwrap();
        trajectory_store.append(&pre_s38_trajectory(losing_id, &key, evidence.rows.clone())).unwrap();
        assert_eq!(parquet_files(&traj_root).len(), 2, "two files is the premise: an upgraded store really does hold both copies");

        drive_one_pass(&trajectory_store, &strategy_store, &key, evidence, None);

        let converged = parquet_files(&traj_root);
        assert_eq!(converged.len(), 1, "the raw layer converges to ONE file per logical verdict set, not merely to one distilled item");
        let canonical_stem = canonical_id.to_string();
        assert_eq!(
            converged[0].0.file_stem().and_then(|stem| stem.to_str()),
            Some(canonical_stem.as_str()),
            "and the surviving file is the canonical row's — the smallest id, unchanged by the deletion"
        );

        // A repeat pass over an unchanged corpus writes and deletes NOTHING:
        // byte-identical files under identical names. Comparing the bytes
        // rather than the count is what makes "writes nothing" mean it — a
        // rewrite with fresh content would keep the count at one.
        let plan = drive_one_pass(&trajectory_store, &strategy_store, &key, evidence, None);
        assert!(matches!(plan, PersistPlan::SkipDuplicate), "a converged namespace has nothing left to converge");
        assert_eq!(parquet_files(&traj_root), converged, "a repeat pass perturbed the raw layer");
    }

    #[test]
    fn a_convergence_pass_reports_the_raw_rows_it_deleted_rather_than_destroying_them_silently() {
        // Deletion is the one thing this driver does that an operator
        // cannot undo, so it is counted and printed on the same "reported,
        // never silent" discipline every other skip and degrade here
        // follows (s42 (`close-the-open-loops`)).
        let dir = tempfile::tempdir().unwrap();
        let trajectory_store = ParquetTrajectoryStore::open(dir.path().join("trajectories"));
        let strategy_store = ParquetStrategyStore::open(dir.path().join("strategies"));
        let key = regime("platformer", "41fdd8c5");
        let by_regime = resolved_divergence_evidence(&key);
        let evidence = by_regime.get(&key).expect("the group is keyed by the regime it was built with");

        for nth in 1..=3 {
            trajectory_store.append(&pre_s38_trajectory(seeded_id(nth), &key, evidence.rows.clone())).unwrap();
        }

        let first = rebuild_namespace_converged(&trajectory_store, &strategy_store, &key).expect("the namespace converges");
        assert_eq!(first.rows_deleted, 2, "three copies of one verdict set converge to one, and the two removals are reported");

        let second = rebuild_namespace_converged(&trajectory_store, &strategy_store, &key).expect("the namespace converges");
        assert_eq!(second.rows_deleted, 0, "a converged namespace reports no further deletion — the count is not a running total");
        assert_eq!(second.items, first.items, "…and re-derives a byte-identical distilled tier, so the raw delete is a fixpoint too");
    }

    #[test]
    fn a_trajectory_this_driver_did_not_write_is_never_superseded() {
        // Rung 2 of the supersession rule. A fixture-seeded or
        // webhook-written row carrying the same verdict set belongs to
        // someone else; overwriting it would destroy evidence this
        // driver never produced.
        let key = regime("platformer", "41fdd8c5");
        let resolution = native_divergence("platformer.session.04", "resolved", Some(RESOLUTION), "2026-07-14T21:38:36Z");
        let by_regime = group_by_regime(vec![derived_from(&AntecedentIndex::new(), &key, &resolution)]);
        let evidence = by_regime.get(&key).unwrap();

        let foreign = Trajectory::new(
            TrajectoryId::new(),
            key.clone(),
            "seeded by a fixture",
            "prose this driver never wrote",
            evidence.rows.clone(),
            "2026-07-14T21:38:36Z".parse().unwrap(),
            vec!["fixture".to_string()],
        )
        .unwrap();
        let foreign_id = foreign.id;

        let PersistPlan::Fresh(minted) = plan_trajectory(std::slice::from_ref(&foreign), &key, evidence, None).unwrap() else {
            panic!("an untagged row is not this driver's to replace")
        };
        assert_ne!(minted.id, foreign_id, "a fresh row lands beside it, leaving the untagged row's bytes untouched");
    }

    #[test]
    fn two_same_instant_verdicts_render_identically_whichever_order_they_arrive_in() {
        // A records-source read orders by `at` ALONE, and a ledger `at`
        // is byte-stable rather than wall-clock, so two rows for one
        // regime routinely tie with no database tie-break. Arrival order
        // must reach neither the rendered text nor the ordered verdict
        // list the persisted identity folds.
        let key = regime("platformer", "41fdd8c5");
        let scenario = "platformer.session.04";
        let at = "2026-07-14T21:38:36Z";
        let a = native_divergence(scenario, "resolved", Some(RESOLUTION), at);
        let b = native_divergence(scenario, "still_divergent", Some(APP_FINDING), at);
        let empty = AntecedentIndex::new();

        let forward = group_by_regime(vec![derived_from(&empty, &key, &a), derived_from(&empty, &key, &b)]);
        let reversed = group_by_regime(vec![derived_from(&empty, &key, &b), derived_from(&empty, &key, &a)]);
        let (task_f, context_f) = forward.get(&key).unwrap().trajectory_text(&key);
        let (task_r, context_r) = reversed.get(&key).unwrap().trajectory_text(&key);
        assert_eq!((&task_f, &context_f), (&task_r, &context_r), "same-`at` arrival order must never reach the rendered text");
        assert_eq!(
            trajectory_content_digest(&key, &forward.get(&key).unwrap().rows, &task_f, &context_f),
            trajectory_content_digest(&key, &reversed.get(&key).unwrap().rows, &task_r, &context_r),
            "…nor the persisted identity, or a repeat ingest of an UNCHANGED record set writes another trajectory"
        );
    }

    #[test]
    fn equal_at_scenario_titles_resolve_by_schema_then_digest_never_by_read_order() {
        // A ledger `at` is byte-stable, so a canon parser change emits a
        // fresh record whose `at` is IDENTICAL to the stale one it
        // supersedes. `at` alone then leaves the slot to whichever row
        // the tier returned first — and a live database read does not
        // order equal-`at` rows at all.
        let at = "2026-07-14T19:27:35Z";
        let mut old_gen = scenario_record("platformer", "platformer.moving.01", "An older sentence", at);
        old_gen.0["schema"] = serde_json::json!(1);
        let mut new_gen = scenario_record("platformer", "platformer.moving.01", "A moving platform carries the standing player", at);
        new_gen.0["schema"] = serde_json::json!(2);

        let key = scenario_key("platformer.moving.01");
        let forward = ScenarioTitleIndex::from_records(&[old_gen.clone(), new_gen.clone()]);
        let reversed = ScenarioTitleIndex::from_records(&[new_gen, old_gen]);
        assert_eq!(
            forward.title_for(&key, Some("platformer")),
            Some("A moving platform carries the standing player"),
            "the newer FORMAT GENERATION wins an `at` tie — that is what `schema` is"
        );
        assert_eq!(forward.title_for(&key, Some("platformer")), reversed.title_for(&key, Some("platformer")));

        // Equal `at` AND equal `schema`: the content digest is the final
        // rung, and it too is the records' own data — never the order
        // they were handed over in.
        let a = scenario_record("platformer", "platformer.moving.01", "Sentence A", at);
        let b = scenario_record("platformer", "platformer.moving.01", "Sentence B", at);
        let ab = ScenarioTitleIndex::from_records(&[a.clone(), b.clone()]);
        let ba = ScenarioTitleIndex::from_records(&[b, a]);
        assert_eq!(ab.title_for(&key, Some("platformer")), ba.title_for(&key, Some("platformer")));
    }

    #[test]
    fn one_omitted_finding_is_counted_once_however_many_verdicts_reject_it() {
        // Every verdict in a regime absorbs the SAME indexed findings.
        // The kept lines dedupe against `antecedent_lines`; before this
        // fix the rejected ones did not, so the rendered marker read
        // "+3", then "+6", for three distinct omissions.
        let key = regime("platformer", "41fdd8c5");
        let scenario = "platformer.session.04";
        let mut events: Vec<ArtifactEvent> =
            (0..7).map(|n| native_divergence(scenario, "open", Some(&format!("SHIP-BLOCKER finding {n}")), &format!("2026-07-14T20:5{n}:00Z"))).collect();
        events.push(native_divergence(scenario, "resolved", Some(RESOLUTION), "2026-07-14T21:38:36Z"));
        events.push(native_divergence(scenario, "still_divergent", Some(APP_FINDING), "2026-07-14T21:40:00Z"));

        let index = index_antecedents(&events);
        let grouped = group_by_regime(vec![derived_from(&index, &key, &events[7]), derived_from(&index, &key, &events[8])]);
        let evidence = grouped.get(&key).unwrap();
        assert_eq!(evidence.antecedent_lines.len(), MAX_ANTECEDENT_LINES, "the same four survive both absorptions");
        assert_eq!(evidence.antecedents_omitted.len(), 3, "seven findings, four kept: three DISTINCT omissions, counted once each");

        let (_, context) = evidence.trajectory_text(&key);
        assert!(context.ends_with("\n  (+3 more omitted at the antecedent cap)"), "the marker counts findings, not rejections: {context}");
    }

    #[test]
    fn a_finding_from_another_project_never_attaches_to_this_projects_verdict() {
        // BLOCKER: a `Scenario`'s identity is `(project_id,
        // scenario_id)` and one scenario id genuinely lives in several
        // projects. Keyed on the bare id, project `world`'s open finding
        // attached itself to project `platformer`'s later resolution.
        let key = regime("platformer", "41fdd8c5");
        let scenario = "platformer.session.04";
        let mut theirs = native_divergence(scenario, "open", Some(APP_FINDING), "2026-07-14T20:50:34Z");
        theirs.detail["project_id"] = serde_json::json!("world");
        let mut ours = native_divergence(scenario, "resolved", Some(RESOLUTION), "2026-07-14T21:38:36Z");
        ours.detail["project_id"] = serde_json::json!("platformer");

        let index = index_antecedents(&[theirs.clone(), ours.clone()]);
        let grouped = group_by_regime(vec![derived_from(&index, &key, &ours)]);
        let (_, context) = grouped.get(&key).unwrap().trajectory_text(&key);
        assert_eq!(context, format!("resolved divergence: {RESOLUTION}"));
        assert!(!context.contains(APP_FINDING), "another project's finding must never be quoted as this one's antecedent: {context}");

        // The SAME identity drives the regime hash, so the two projects
        // cannot fold onto one trajectory either.
        assert_ne!(regime_hash(&event_identity(&theirs)), regime_hash(&event_identity(&ours)));

        // …and the join still works WITHIN one project, so this is a
        // correction, not a disabling.
        let mut same_project = theirs.clone();
        same_project.detail["project_id"] = serde_json::json!("platformer");
        let joined = index_antecedents(&[same_project, ours.clone()]);
        let grouped = group_by_regime(vec![derived_from(&joined, &key, &ours)]);
        let (_, context) = grouped.get(&key).unwrap().trajectory_text(&key);
        assert!(context.contains(APP_FINDING), "a same-project finding still attaches: {context}");
    }

    #[test]
    fn a_pathological_title_allocates_only_the_capped_prefix() {
        // The 160-char bound applied to the OUTPUT while the peak
        // allocation stayed proportional to the artifact-supplied INPUT.
        // `capacity` is the observable that separates the two, since
        // `String::truncate` never gives memory back.
        let fat = "x".repeat(400_000);
        let title = compact_scenario_title(&fat);
        assert_eq!(title.chars().count(), SCENARIO_TITLE_MAX_CHARS + 1, "the cap plus its one-char cut marker");
        assert!(title.ends_with('…'), "a cut is marked, never silent");
        assert!(
            title.capacity() <= SCENARIO_TITLE_MAX_CHARS * 4 + 8,
            "a joined title must never allocate the discarded tail of a record: {} bytes held for a {}-char title",
            title.capacity(),
            SCENARIO_TITLE_MAX_CHARS
        );
    }

    // ── s42 re-review: provenance-preserving convergence, a retriable
    //    rebuild order, and a typed `--run` admission ──

    /// A `Run` manifest of exactly the shape `canon dispatch begin`
    /// writes — serialized through the real `canon_model::records::Run`,
    /// so a test cannot accidentally assert against a JSON shape the
    /// record type does not produce.
    fn run_manifest(run_id: RunId, status: RunStatus) -> String {
        let at: DateTime<Utc> = "2026-07-14T21:38:36Z".parse().expect("a fixed RFC3339 instant");
        let actor = canon_model::envelope::Actor::new("canon".to_string(), canon_model::ids::RoleId::parse("dev").unwrap());
        let run = canon_model::records::Run::new(
            canon_model::envelope::Envelope::current(RecordKind::Run, at, actor),
            run_id,
            None,
            None,
            status,
            at,
            None,
        );
        serde_json::to_string_pretty(&run).expect("a Run is always serializable")
    }

    #[test]
    fn a_superseded_copy_carrying_its_own_verdict_is_kept_and_reported_never_deleted() {
        // BLOCKER regression (s42 re-review, `ReviewRuns` finding 1).
        // `verdict_record` is written back AFTER derivation, so two rows
        // sharing a derivation key can carry OPPOSITE outcomes. Deleting
        // the loser removed the contradiction `canon learn promote`
        // refuses on — `OccurrencePromotionGate` resets its success
        // streak on one `Failure` — turning a refusal into a promotion.
        let dir = tempfile::tempdir().unwrap();
        let trajectory_store = ParquetTrajectoryStore::open(dir.path().join("trajectories"));
        let strategy_store = ParquetStrategyStore::open(dir.path().join("strategies"));
        let key = regime("platformer", "41fdd8c5");
        let by_regime = resolved_divergence_evidence(&key);
        let evidence = by_regime.get(&key).expect("the group is keyed by the regime it was built with");

        let (canonical_id, dissenting_id, silent_id) = (seeded_id(1), seeded_id(2), seeded_id(3));
        trajectory_store
            .append(
                &pre_s38_trajectory(canonical_id, &key, evidence.rows.clone())
                    .with_verdict_record(TrajectoryVerdict::new(VerdictOutcome::Success, 0.9)),
            )
            .unwrap();
        trajectory_store
            .append(
                &pre_s38_trajectory(dissenting_id, &key, evidence.rows.clone())
                    .with_verdict_record(TrajectoryVerdict::new(VerdictOutcome::Failure, 0.3)),
            )
            .unwrap();
        // …and a copy claiming nothing at all: never marked, never
        // attributed. That one really IS only an older rendering, so it
        // is still deleted — the bar narrows the delete, it does not
        // disable it.
        trajectory_store.append(&pre_s38_trajectory(silent_id, &key, evidence.rows.clone())).unwrap();

        let converged = rebuild_namespace_converged(&trajectory_store, &strategy_store, &key).expect("the namespace converges");

        assert_eq!(converged.rows_deleted, 1, "only the copy whose provenance the canonical row subsumes may be deleted");
        let surviving: Vec<TrajectoryId> = rows_by_id(&trajectory_store, &key).into_iter().map(|row| row.id).collect();
        assert_eq!(surviving, vec![canonical_id, dissenting_id], "the dissenting row survives; the silent duplicate does not");

        // The observable a promotion gate reads: `query_by_regime_key`
        // still returns BOTH outcomes, so the contradiction is still
        // there to refuse on.
        let outcomes: Vec<VerdictOutcome> = rows_by_id(&trajectory_store, &key).iter().map(|row| row.verdict_record.outcome).collect();
        assert_eq!(
            outcomes,
            vec![VerdictOutcome::Success, VerdictOutcome::Failure],
            "deleting the losing Failure would turn this regime's refusal into a promotion"
        );

        // Kept is not served: the retained loser is still withheld from
        // the distiller, exactly as s41 left it.
        let canonical_row = rows_by_id(&trajectory_store, &key).into_iter().find(|row| row.id == canonical_id).expect("it survives");
        assert_eq!(
            converged.items,
            distill_namespace(&key, std::slice::from_ref(&canonical_row)),
            "a kept conflict is kept, not served — the distiller still sees the canonical row alone"
        );

        // And it is REPORTED, naming both rows and the disagreeing fact:
        // this driver refuses to pick, so an operator has to be able to.
        assert_eq!(converged.conflicts.len(), 1, "{:#?}", converged.conflicts);
        let conflict = &converged.conflicts[0];
        assert_eq!(conflict.retained_id, dissenting_id.to_string());
        assert_eq!(conflict.canonical_id, canonical_id.to_string());
        assert_eq!(conflict.regime_key, key.as_str());
        assert_eq!(conflict.claims.len(), 1, "one disagreeing field, one line: {:#?}", conflict.claims);
        assert!(
            conflict.claims[0].contains("failure") && conflict.claims[0].contains("success"),
            "the report must name BOTH verdicts, or the operator cannot tell which row to remove: {}",
            conflict.claims[0]
        );

        // A repeat pass re-reports the identical conflict over a
        // byte-identical rebuild and deletes nothing further: a kept row
        // is a STANDING condition, never a one-shot warning that is then
        // forgotten.
        let again = rebuild_namespace_converged(&trajectory_store, &strategy_store, &key).expect("the namespace converges");
        assert_eq!(again.rows_deleted, 0);
        assert_eq!(again.conflicts, converged.conflicts);
        assert_eq!(again.items, converged.items);
    }

    #[test]
    fn a_superseded_copy_carrying_its_own_run_attribution_is_kept_and_reported() {
        // The other independently-written field, same blocker.
        // `plan_trajectory` stamps `run_id` onto the row a `--run` pass
        // WRITES and onto no other, so a namespace really can hold an
        // attributed loser beside an unattributed canonical row —
        // and deleting it erases that run's only line in
        // `mart_flywheel_funnel`'s attribution stage.
        let dir = tempfile::tempdir().unwrap();
        let trajectory_store = ParquetTrajectoryStore::open(dir.path().join("trajectories"));
        let strategy_store = ParquetStrategyStore::open(dir.path().join("strategies"));
        let key = regime("platformer", "41fdd8c5");
        let by_regime = resolved_divergence_evidence(&key);
        let evidence = by_regime.get(&key).expect("the group is keyed by the regime it was built with");

        let (canonical_id, attributed_id) = (seeded_id(1), seeded_id(2));
        let run_id = RunId::new();
        trajectory_store.append(&pre_s38_trajectory(canonical_id, &key, evidence.rows.clone())).unwrap();
        trajectory_store
            .append(&pre_s38_trajectory(attributed_id, &key, evidence.rows.clone()).with_run_id(Some(run_id)))
            .unwrap();

        let converged = rebuild_namespace_converged(&trajectory_store, &strategy_store, &key).expect("the namespace converges");

        assert_eq!(converged.rows_deleted, 0, "the only superseded copy carries an attribution the canonical row does not");
        assert_eq!(
            rows_by_id(&trajectory_store, &key).into_iter().map(|row| row.run_id).collect::<Vec<_>>(),
            vec![None, Some(run_id)],
            "the attributed row survives verbatim"
        );
        assert_eq!(converged.conflicts.len(), 1, "{:#?}", converged.conflicts);
        assert!(
            converged.conflicts[0].claims[0].contains(&run_id.to_string()) && converged.conflicts[0].claims[0].contains("no run"),
            "the report names the run that would have been erased, and that the canonical row records none: {:#?}",
            converged.conflicts[0].claims
        );

        // An AGREEING attribution is not a conflict: subsumption, not
        // equality, is the bar, so the ordinary converged case still
        // deletes.
        let agreeing = seeded_id(4);
        trajectory_store.append(&pre_s38_trajectory(agreeing, &key, evidence.rows.clone())).unwrap();
        let converged = rebuild_namespace_converged(&trajectory_store, &strategy_store, &key).expect("the namespace converges");
        assert_eq!(converged.rows_deleted, 1, "an unattributed copy beside an unattributed canonical row claims nothing");
        assert_eq!(converged.conflicts.len(), 1, "…and the attributed one is still kept and still reported");
    }

    /// A `StrategyStore` whose `delete_for_regime_key` — the FIRST write
    /// [`rebuild_namespace_converged`] makes — fails on demand.
    /// Everything else delegates to a real `ParquetStrategyStore`, so
    /// clearing the fault and retrying exercises the genuine path rather
    /// than a second stub.
    struct FlakyStrategyStore {
        inner: ParquetStrategyStore,
        fail: std::cell::Cell<bool>,
    }

    impl StrategyStore for FlakyStrategyStore {
        fn append(&self, item: &StrategyItem) -> Result<(), LearnError> {
            self.inner.append(item)
        }

        fn query_by_regime_key(&self, regime_key: &RegimeKey) -> Result<Vec<StrategyItem>, LearnError> {
            self.inner.query_by_regime_key(regime_key)
        }

        fn delete_for_regime_key(&self, regime_key: &RegimeKey) -> Result<usize, LearnError> {
            if self.fail.get() {
                return Err(LearnError::Parquet("simulated strategy-tier outage".into()));
            }
            self.inner.delete_for_regime_key(regime_key)
        }

        fn find_by_id(&self, id: &canon_learn::StrategyId) -> Result<Option<StrategyItem>, LearnError> {
            self.inner.find_by_id(id)
        }

        fn mark_demoted(&self, id: &canon_learn::StrategyId, demotion: canon_learn::DemotionEvidence) -> Result<(), LearnError> {
            self.inner.mark_demoted(id, demotion)
        }
    }

    #[test]
    fn a_failed_strategy_rebuild_leaves_a_store_an_unchanged_retry_still_converges() {
        // BLOCKER regression (s42 re-review, `ReviewRuns` finding 2). The
        // raw duplicate is the ONLY condition that selects
        // `ConvergeDistilled`. s42 shipped the raw delete FIRST, so a
        // failed strategy rebuild returned with the trigger already gone:
        // an unchanged retry took `SkipDuplicate` and never repaired the
        // empty or half-rebuilt distilled tier — permanently, with
        // nothing in the store to say so.
        let dir = tempfile::tempdir().unwrap();
        let traj_root = dir.path().join("trajectories");
        let trajectory_store = ParquetTrajectoryStore::open(&traj_root);
        let strategy_store =
            FlakyStrategyStore { inner: ParquetStrategyStore::open(dir.path().join("strategies")), fail: std::cell::Cell::new(true) };
        let key = regime("platformer", "41fdd8c5");
        let by_regime = resolved_divergence_evidence(&key);
        let evidence = by_regime.get(&key).expect("the group is keyed by the regime it was built with");

        // The canonical row is already CURRENT and a stale sibling sits
        // beside it — the exact seeding whose only remaining work is
        // convergence, so `ConvergeDistilled` is what the plan must keep
        // selecting across the failure.
        let (canonical_id, losing_id) = (seeded_id(1), seeded_id(2));
        trajectory_store.append(&current_trajectory(canonical_id, &key, evidence)).unwrap();
        trajectory_store.append(&pre_s38_trajectory(losing_id, &key, evidence.rows.clone())).unwrap();
        let before = parquet_files(&traj_root);
        assert_eq!(before.len(), 2, "two copies of one verdict set is the premise");

        rebuild_namespace_converged(&trajectory_store, &strategy_store, &key).expect_err("the strategy tier is down");

        assert_eq!(
            parquet_files(&traj_root),
            before,
            "a failed rebuild must not have deleted a raw byte — the duplicate IS what makes the retry converge"
        );
        let existing = trajectory_store.query_by_regime_key(&key).unwrap();
        assert!(
            matches!(plan_trajectory(&existing, &key, evidence, None).unwrap(), PersistPlan::ConvergeDistilled),
            "an UNCHANGED retry must still select convergence, never skip as an already-converged duplicate"
        );

        // Fault cleared, same corpus, nothing else changed: the retry
        // rebuilds the distilled tier AND finishes the raw convergence.
        strategy_store.fail.set(false);
        let repaired = rebuild_namespace_converged(&trajectory_store, &strategy_store, &key).expect("the retry converges");
        assert_eq!(repaired.rows_deleted, 1, "the duplicate kept for exactly this reason is deleted once the rebuild committed");
        assert!(!repaired.items.is_empty(), "…and the distilled tier the whole convergence exists to repair is populated");
        assert_eq!(parquet_files(&traj_root).len(), 1, "the raw layer converges on the retry, not before it");
    }

    #[test]
    fn a_run_manifest_is_admitted_only_after_it_deserializes_as_that_exact_dispatched_run() {
        // SHOULD-FIX regression (s42 re-review, `ReviewAttribution`
        // finding 1). The bar was `std::fs::metadata`, which succeeds for
        // a DIRECTORY, for truncated or foreign JSON, and for a valid
        // `Run` filed under somebody else's id — and the unchecked id was
        // then stamped onto every trajectory the pass wrote. A false
        // attribution is worse than none.
        let dir = tempfile::tempdir().unwrap();
        let dispatch_dir = dir.path().join(crate::dispatch::DISPATCH_DIR);
        std::fs::create_dir_all(&dispatch_dir).unwrap();

        let as_dir = RunId::new();
        std::fs::create_dir(dispatch_dir.join(format!("{as_dir}.json"))).unwrap();
        let err = resolve_dispatched_run(dir.path(), Some(&as_dir)).expect_err("a directory is not a dispatch manifest");
        assert!(matches!(err, ArtifactIngestError::UnreadableRunManifest { .. }), "expected UnreadableRunManifest, got {err:?}");

        // The exact bytes the pre-hardening admission accepted verbatim.
        let malformed = RunId::new();
        std::fs::write(dispatch_dir.join(format!("{malformed}.json")), "{}").unwrap();
        let err = resolve_dispatched_run(dir.path(), Some(&malformed)).expect_err("an empty JSON object is not a Run");
        let ArtifactIngestError::UnreadableRunManifest { run_id, path, .. } = &err else {
            panic!("expected UnreadableRunManifest, got {err:?}")
        };
        assert_eq!(run_id, &malformed.to_string(), "the error names the run the operator typed");
        assert!(path.ends_with(&format!("{malformed}.json")), "and the exact path consulted: {path}");

        let filed_under = RunId::new();
        let impostor = RunId::new();
        std::fs::write(dispatch_dir.join(format!("{filed_under}.json")), run_manifest(impostor, RunStatus::Running)).unwrap();
        let err = resolve_dispatched_run(dir.path(), Some(&filed_under)).expect_err("a misfiled manifest must not be admitted");
        let ArtifactIngestError::MisfiledRunManifest { found, .. } = &err else { panic!("expected MisfiledRunManifest, got {err:?}") };
        assert_eq!(found, &impostor.to_string(), "the error names the run the manifest actually records, not the one asked for");

        let never_ran = RunId::new();
        std::fs::write(dispatch_dir.join(format!("{never_ran}.json")), run_manifest(never_ran, RunStatus::Pending)).unwrap();
        let err = resolve_dispatched_run(dir.path(), Some(&never_ran)).expect_err("a pending run never executed");
        assert!(matches!(err, ArtifactIngestError::RunNeverDispatched { .. }), "expected RunNeverDispatched, got {err:?}");

        // The two states that ARE admissible: the run this pass is
        // executing inside, and one a wrapper closed before its final
        // ingest. `dispatch end`'s stricter `(Running, ended_at: None)`
        // predicate is deliberately NOT this driver's bar — `end`
        // mutates the run, this only reads an attribution target.
        for status in [RunStatus::Running, RunStatus::Succeeded] {
            let admissible = RunId::new();
            std::fs::write(dispatch_dir.join(format!("{admissible}.json")), run_manifest(admissible, status)).unwrap();
            resolve_dispatched_run(dir.path(), Some(&admissible)).expect("a dispatched run is admissible");
        }
    }
}
