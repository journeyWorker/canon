## Why

Strategic learning today is domain-scoped and single-repo: the donor harness's
`reasoning-bank.ts` proves the trace→verdict→distill→store→retrieve→apply
loop end-to-end for exactly three fixed namespaces (`dev|content|sim`) inside
one repo, backed by a LanceDB index no human ever reviews as text. The
operator's decision (design doc decision 6) is a role-namespaced strategy
memory — shared backbone, per-role evolution — with promoted strategies
landing in a git tier the team can see and PR. Nothing today generalizes the
namespace beyond the donor monorepo's three domains or gives the team a reviewable surface
for what the harness has learned.

## What Changes

- New `canon-learn` crate: ReasoningBank's trace→distill→store→retrieve loop
  generalized to an open `role` enum (`planning|design|dev|test|review|
  content|sim`, extensible via `canon.yaml`).
- New two-store split: raw `Trajectory` (cold tier, LanceDB) + distilled
  `StrategyItem` (title/description/content), non-destructive —
  `rebuild_strategies` deletes and re-derives only the distilled layer,
  never the raw trajectories.
- New canonical `regime_key` grammar `<role>/<repo>/<area>/<hash>`, a single
  function called identically at write time and read time (generalizes
  the donor tuning project's `simRetrievalKey` write/read-identity discipline).
- New git-tier strategy promotion: `canon learn promote <strategy_id>`
  writes a reviewable, PR'd file under `canon/strategies/<role>/` carrying
  `sourceTrajectoryIds` provenance — a capability the donor monorepo's implementation
  never had (the donor monorepo's distilled strategies are LanceDB-only).
- New migration-plan deliverable (§10 OQ3): a field-by-field mapping +
  cutover recommendation for the donor harness's existing dev/content/sim
  store — a PLAN document only; the actual cutover is a follow-up change.

## Capabilities

### New Capabilities
- `role-strategy-memory`: role-namespaced raw-trajectory + distilled-
  strategy stores, canonical regime-key grammar, non-destructive
  delete-rebuild distillation, similarity search scoped by role, git-tier
  strategy promotion with provenance.

### Modified Capabilities
(none — no existing `openspec/specs/` capabilities in this repo yet)

## Impact

- New `crates/canon-learn`, depending on `canon-model` (S1 — `Trajectory` /
  `StrategyItem` types, `regime_key` join-spine grammar) and `canon-store`
  (S2 — R2/LanceDB cold tier for raw trajectories, git tier for promoted
  strategies).
- Downstream consumers: S7 (reward wiring calls `mark_trajectory_verdict`
  on this store), S8 (retrieval reads this store's `StrategyItem` search
  surface).
- The donor harness's learning module is NOT
  touched by this change — this change's deliverable is a migration PLAN,
  not the cutover.
- New `canon/skills/` companion skill for `canon-learn` usage.
