## Context

The donor harness's `reasoning-bank.ts` already proves the full
trace→verdict→distill→store→retrieve→apply loop for THREE fixed domain
namespaces (`dev|content|sim`, `PatternNamespace`) inside one repo, with
`PatternTrajectory` (raw) and `StrategyMemoryItem` (distilled,
`title`/`description`/`content`, non-destructive via
`deleteStrategiesForNamespace` + `rebuildStrategies`) modeled as a shared
store partitioned by a string column. The donor monorepo's live store used a shared
LanceDB index for those rows; `canon-learn` keeps the proven two-store shape
but generalizes the raw tier to a parquet-first, operator-local cold store.
The donor tuning project's `sweep-trajectory.ts` independently proves the
canonical-regime-key discipline for the `sim` namespace specifically:
`simRetrievalKey(regime)` is the single function called at write time
(`buildSweepTrajectory`) and read time (`buildPreSweepQuery`), so a sweep's
own distilled strategy is guaranteed to be its own top retrieval hit. Neither
piece generalizes past the donor monorepo's three domains, past one repo, or gives the
team a human-reviewable form of what got learned — the donor monorepo's promoted
strategies live ONLY in LanceDB. `canon-learn` generalizes the namespace to
an open `role` enum, the regime key to the join-spine grammar, and adds the
git-tier promotion path the donor monorepo never built.

## Goals / Non-Goals

**Goals:**
- Generalize `PatternNamespace` (`dev|content|sim`) to an open `role` enum
  (`planning|design|dev|test|review|content|sim` shipped built-in,
  extensible per consumer repo) any repo can populate.
- Ship the same two-store shape the donor monorepo proves out: raw `Trajectory` cold-tier
  + distilled `StrategyItem` git/warm-tier, non-destructive
  (`rebuild_strategies` deletes only the distilled layer, never raw).
- Generalize `simRetrievalKey`'s write/read-identical canonical
  serialization into the join spine's `regime_key` grammar
  `<role>/<repo>/<area>/<hash>`, usable by any role, not just `sim`.
- Land PROMOTED strategies as git-tier files under `canon/strategies/` —
  reviewable, PR'd, provenance-tagged with `sourceTrajectoryIds` — a
  net-new capability, not a port.
- Produce a migration/cutover PLAN for the donor harness's existing store
  (§10 OQ3) — plan only, not the cutover itself.

**Non-Goals:**
- Executing the cutover of the donor harness's dev/content/sim namespaces
  onto `canon-learn` — a follow-up change; this change's task only
  produces the plan document, no donor-repo edits.
- Reward computation / statistical promotion gating — S7.
- Retrieval dispatch hook / advisory injection — S8.
- Building a parallel LanceDB/vector cold store alongside the parquet-first
  choice below "just in case" — the LanceDB-vs-parquet question (§10 OQ2) is
  resolved as one parquet-first decision for this change, not hedged.

## Decisions

1. **Role enum is open, not closed.** `Role` is a `canon-model` string
   newtype validated against a REGISTERED set (`planning|design|dev|test|
   review|content|sim` shipped built-in), extendable per consumer repo via
   `canon.yaml` — never a closed Rust `enum` requiring a canon release to
   add a role. *Alternative rejected:* a closed, exhaustively-matched
   `enum Role` — would force every new specialist role to wait on a canon
   crate release; the donor monorepo's own dev-* handler roster already grows faster
   than a core crate should re-release for.

2. **Canonical regime key reuses the donor tuning project's write/read-identity
   discipline exactly.** `regime_key(role, repo, area, hash) -> String` is
   the SINGLE function both the trajectory-store write path and every read
   path (S7's promotion queries, S8's retrieval queries) call — no second
   implementation. `role` leads the tuple (the primary retrieval axis — a
   `dev` trajectory must never surface as a "similar regime" hit for a
   `content` role even under identical repo+area+hash), mirroring
   `simRetrievalKey`'s `char`-leads rationale exactly. *Alternative
   rejected:* hashing the whole tuple into one opaque key — loses
   partial-match "same role, different area" queries and makes the S1
   join-spine table's grammar column undocumentable.

3. **Two-store split with non-destructive delete-rebuild.** `Trajectory`
   (raw, immutable, cold tier) and `StrategyItem` (distilled,
   `sourceTrajectoryIds` provenance) are separate tables; `rebuild_
   strategies(role)` deletes ONLY the strategy rows for a role and
   re-derives them from retained raw trajectories — mirrors
   `deleteStrategiesForNamespace` + `rebuildStrategies` exactly, never
   touching raw. *Alternative rejected:* mutating strategy rows in place —
   loses the audit trail of "what the distiller believed at time T", which
   S9's flywheel-health funnel needs.

4. **Promoted strategies land in the git tier as reviewable files.**
   `canon learn promote <strategy_id>` writes
   `canon/strategies/<role>/<strategy_id>.md` (front-matter
   title/description + body content) with a provenance block
   (`sourceTrajectoryIds`, S7's promotion evidence) into the consumer
   repo's git tree — a human-reviewable PR diff. This is the "team sees
   and PRs the harness's learned behavior" decision (design doc decision
   6; §4 architecture: "promote to git tier (versioned, PR-reviewed)").
   *Alternative rejected:* keeping promoted strategies LanceDB/Postgres-
   only with a dashboard view — the design doc's storage-tier diagram
   already places "promoted strategies" in the git tier, and S9's
   dashboard explicitly MIRRORS canon-report, never IS the source of
   truth.

5. **Parquet-first for the raw `Trajectory` cold tier (resolves §10 OQ2
   for this change).** Store immutable raw trajectories as Arrow/Parquet
   files in an operator-local cold tier, Hive-nested by the canonical
   `regime_key` tuple (`role`/`repo`/`area`/`hash`) and hidden behind a
   `TrajectoryStore` trait so write/read callers do not know the storage
   adapter. `canon-store`'s R2 parquet tier (S2) remains an archival copy
   and DuckDB-queryable reporting tier, not a competing raw store.
   *Alternative rejected (recorded per §10 OQ2):* LanceDB/vector-ANN as the
   primary raw index — the donor monorepo used LanceDB as donor-provenance, but this
   change defers ANN to an additive future swap behind the same
   `TrajectoryStore` trait because the native-dependency cost and the donor monorepo's
   zero-production-callers precedent compound into avoidable rollout risk.

6. **Distillation is fail-soft and decoupled from the primary write.**
   `store_trajectory` never waits on distillation — distillation is a
   separate, best-effort step, mirroring `reasoning-bank.ts`'s explicit
   split between `storeTrajectory` and `distillTrajectory` as independent
   facades (proven by the donor monorepo's own tests: "a throwing distiller never wrote
   through" / "the sweep's own trajectory is still stored — distillation
   runs strictly AFTER the outcome is recorded"). *Alternative rejected:*
   synchronous store+distill in one transaction — a distiller fault would
   then block trajectory persistence, contradicting §7's "malformed
   evidence is no evidence, never crash" principle generalized to "a
   broken distiller must never break recording".

## Risks / Trade-offs

- [Risk] An open role registry lets a typo'd role (`"dEv"` vs `"dev"`)
  silently fork a regime namespace → [Mitigation] the role registry is
  validated at `canon context` (S12) / `canon gate check` time;
  `canon-learn` rejects an unregistered role at write time — fail loud,
  not fail soft (a write-time schema check, distinct from retrieval's
  advisory fail-soft contract in S8).
- [Risk] Cross-repo strategy git-tier files could leak repo-specific
  paths/secrets into a promoted strategy's `content` field →
  [Mitigation] S7's statistical-promotion gate is the primary enforcement
  point; this change's `canon learn promote` adds a content-length +
  literal-path-pattern lint as defense-in-depth (documented as advisory —
  full secret-scanning is out of scope).
- [Risk] LanceDB-from-Rust would add a native dependency (not pure-Rust;
  ships prebuilt libs) to the prebuilt-binary launcher (S0)
  → [Mitigation] retired by the parquet-first pivot: LanceDB is no longer a
  shipped dependency for this change. The `TrajectoryStore` trait seam is
  still preserved so a future LanceDB/vector-ANN adapter can land as an
  additive swap, never a call-site rewrite.
- [Risk] The donor cutover-plan task could be misread as this change also
  executing the cutover → [Mitigation] `tasks.md` scopes that task
  explicitly to producing a plan document; proposal.md's Non-Goals state
  the same directly.
- [Risk] The donor LanceDB pattern store Decision 5 originally cited has
  ZERO production callers: `harness-svc` doesn't even import
  `PatternStoreTag`, and two donor harness components both independently hard-wire
  `makeInMemoryPatternStore` instead. Every reasoning-bank write across
  the donor monorepo's whole harness, all three namespaces, is process-local and lost on
  every restart — this is not a partial gap, it is the single most
  consequential finding for S6's scoping. → Mitigation: this change's
  cutover plan (Migration Plan, §10 OQ3) MUST include wiring `canon-learn`'s
  `TrajectoryStore` into a real default entrypoint from day one — the
  deferred-language pattern the donor monorepo used twice ("caller can override with a
  LanceDB-backed Layer once that lands") is explicitly the failure mode to
  avoid, not a template to repeat. This evidence's own recommendation was
  TAKEN at implementation time: together with LanceDB's native-dependency
  cost (risk retired above), the zero-callers precedent resolved §10 OQ2
  toward parquet-first operator-local storage, with LanceDB/vector-ANN
  deferred behind the trait seam as an additive future adapter.

## Migration Plan

- Step 1: `canon-learn` ships standalone; zero donor-monorepo changes required for
  it to function.
- Step 2: this change's cutover-plan task documents the field-by-field
  mapping (`PatternNamespace` `dev|content|sim` → `role`; `PatternTrajectory`
  → `Trajectory`; `StrategyMemoryItem` → `StrategyItem`) and two options
  for the donor monorepo: (a) point `PatternStoreTag`'s live implementation at
  `canon-learn`'s store via a thin adapter, keeping `reasoning-bank.ts`'s
  existing call sites unchanged, or (b) a deeper cutover replacing
  `reasoning-bank.ts` outright. The plan recommends (a) as lower-risk
  given `reasoning-bank.ts`'s existing test suite.
- Rollback: not applicable at this change's scope — nothing in the donor repo is
  touched.

## Open Questions

- §10 OQ2 (LanceDB vs parquet-only): resolved above as parquet-first for
  the raw `Trajectory` cold tier; LanceDB/vector-ANN is the additive future
  swap behind the `TrajectoryStore` trait, not the shipped primary index.
- §10 OQ3 (the donor harness cutover plan): this change's task produces
  the plan; which change OWNS executing the cutover (canon-side or
  donor-side, and its timing) is not decided here — flagged for operator
  sequencing after S6 lands.
