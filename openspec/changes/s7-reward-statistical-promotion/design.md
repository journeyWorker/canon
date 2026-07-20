## Context

The donor monorepo proves two halves of this independently and never wires them together
across roles. (a) `dev-reward-backfill.ts`: a complete, weighted composite
reward function (`pr-merged 0.4 + ci-pass 0.3 + no-rollback 0.3`, rollback/
CI-fail floors to `0.1`, human-approval shortcuts to `1.0`) plus a
`DevRewardSignalStore` port and a `backfillDevReward` facade that calls
`markTrajectoryVerdict` — but its own doc states "the webhook receiver
wiring... lands later," and the actual `apps/harness-svc/server/routes/
github.ts` receiver was never shipped. (b) `matts.ts`: a complete,
rigorously reviewed (F1/F2/F3 fixes documented inline) statistical-
corroboration gate — paired CRN panels, ANOVA-style variance decomposition,
df-aware significance tables — scoped ONLY to the `sim` namespace's tuning
sweeps, because CRN replay requires a deterministic simulator most roles
don't have. `canon-learn`'s reward/promotion layer generalizes both and
resolves the domain split explicitly: paired-CRN where replay is possible,
n-occurrence + zero-contradiction elsewhere.

## Goals / Non-Goals

**Goals:**
- Generalize `computeDevReward`'s weighted-composite shape to a per-role
  reward function registry, reading S4's verdict stream (the design doc's
  review→verdict table: code-review finding→dev failure, review-record→
  success, PR merge→success, PR revert/CI fail→failure, etc).
- Ship `mark_trajectory_verdict`, completing S6's store write-back — this
  change is the FIRST real caller of that surface besides tests.
- Generalize MaTTS's `corroboratedEffect`/`pairedContrast`/
  `decomposeBandVariance` machinery as the promotion gate for any role
  whose domain supports deterministic CRN replay; for roles that don't
  (most of `dev`/`content`/`design`/`review`), fall back to an
  n-occurrence threshold + zero-contradiction window.
- Ship the webhook receiver the donor monorepo deferred, generalized: ingests PR/CI
  events (building on S4's adapters), publishes into the reward-signal
  store, and the backfill driver marks verdicts.
- Ship the demotion path: a PROMOTED strategy that later collects a
  contradicting trajectory is demoted — removed from or flagged in the git
  tier, not silently skipped at the next promotion pass.

**Non-Goals:**
- Building a new simulator for non-sim roles just to make paired-CRN
  available everywhere — the n-occurrence fallback is the permanent answer
  for non-replayable domains, not a stopgap.
- Retrieval / dispatch-hook injection of promoted strategies — S8.
- The review→verdict ADAPTER logic (turning a code-review finding into a
  `failure` event) — that is S4's ingest surface; this change only
  consumes S4's already-normalized verdict stream.
- Changing the donor monorepo's `dev-reward-backfill.ts` weights or behavior — canon
  generalizes the PATTERN via a clean-room Rust reimplementation of the
  same formula shape; the donor monorepo's TS implementation is not forked or wrapped.

## Decisions

1. **Reward functions are per-role, registered, not one universal
   formula.** A `RewardFn: Role -> VerdictEvent -> f64` registry holds one
   entry per role, mirroring `computeDevReward`'s dev-specific weights
   (0.4/0.3/0.3) as the `dev` role's default entry; other roles get their
   own weight sets keyed off S4's review→verdict table (e.g. `review`'s
   reward source is a `clear-record after @flagged` corrective success;
   `design`'s is a design-review promotion). *Alternative rejected:* one
   global reward formula parameterized by event type only — PR-merge/
   CI-pass/no-rollback is meaningless for a `content` or `design` role
   whose verdict source is a review record, not git.

2. **`mark_trajectory_verdict` mirrors the donor monorepo's verdict-write contract
   exactly.** A `pending|success|failure|rolled-back` verdict enum +
   `reward: f64` clamped to `[0, 1]`, with the same default convention
   `reasoning-bank.ts` documents (0.9 success / 0.3 failure / 0.5
   pending) as canon's DEFAULT — a role's registered reward function may
   override the exact scalar but must stay within `[0, 1]` and must not
   leave a trajectory at `pending` once a covering verdict event arrives.
   The no-rollback WINDOW stays config (mirrors `dev-reward-backfill.ts`'s
   "the timer is a webhook-receiver concern" — the constant set only
   enforces the math). *Alternative rejected:* a free-form reward scale
   per role — S9's dashboard funnel (verdicts→distilled→retrieved→
   applied) needs one comparable scale across roles.

3. **Statistical promotion: paired-CRN where replayable, n-occurrence +
   zero-contradiction otherwise.** A `PromotionGate` trait has two
   implementations. `CrnPromotionGate` ports `matts.ts`'s pure statistics
   core to Rust as deterministic functions of `f64` sample arrays —
   `seed_panels`, `decompose_band_variance` (df-aware F(1,df) table +
   `MIN_DF_RESIDUAL` floor), `paired_contrast` (df-aware two-sided t(df)
   table + `MIN_PANELS_FOR_SIGNIFICANCE` floor), `corroborated_effect` —
   so a promotion decision never trusts a too-small sample.
   `OccurrencePromotionGate` (new — no direct donor) requires
   `n_min` corroborating `success`-verdict trajectories for the SAME
   `regime_key` AND zero `failure`-verdict trajectories for that
   `regime_key` inside a configurable observation window; a single
   contradicting failure resets the counter rather than being averaged
   away. A role declares which gate applies via `canon.yaml`
   (`promotion.<role>.mode: crn | occurrence`). *Alternative rejected:*
   always using n-occurrence (simplest, universal) — throws away the CRN
   gate's stronger guarantee for domains that CAN support it, where
   "lucky seed" false positives are a documented real failure (F1) MaTTS
   review already caught.

4. **Demotion is a first-class outcome, not a no-op.**
   `demote_strategy(strategy_id, contradicting_trajectory_id)` writes a
   demotion evidence record (S1 envelope) and, for git-tier-promoted
   strategies, either deletes the `canon/strategies/<role>/<id>.md` file
   or annotates it `status: demoted` (front-matter) depending on
   `canon.yaml` policy — default is soft-flag, matching §7's append-only
   "corrections are new records" principle applied to the git tier (a NEW
   commit demotes; nothing is force-rewritten). *Alternative rejected:*
   silent removal from retrieval scoring only, leaving the file untouched
   — violates "the team sees and PRs the harness's learned behavior"
   (design decision 6): a demotion the team can't see in git history isn't
   reviewable.

5. **Webhook receiver built on S4, not a bespoke ingester.** The PR/CI
   webhook receiver is a thin adapter that normalizes GitHub webhook
   payloads (`pull_request.merged`, `workflow_run.conclusion`) into S4's
   already-defined verdict-event shape, then calls this change's reward
   functions + `mark_trajectory_verdict` — it does not duplicate S4's
   ingest adapter registry. *Alternative rejected:* a standalone webhook
   service independent of S4's normalization — would recreate the "two
   stores, no documented join key" failure mode (design doc §1) one layer
   down, as two verdict paths instead of one.

## Risks / Trade-offs

- [Risk] `n_min`/observation-window defaults for `OccurrencePromotionGate`
  are picked without real cross-role data (no donor for this gate —
  it is new) → [Mitigation] ship conservative defaults (`n_min: 5`,
  window: 30 days, documented as provisional in `policy.yaml`'s schema
  doc) and expose them as policy fields tunable per role without a code
  change, per D7's "tightening is a policy diff" discipline.
- [Risk] The CRN promotion gate inherits MaTTS's own documented fragility
  (F1/F2/F3 fixes were needed post-review) → [Mitigation] port the FIXED
  versions verbatim (df-aware tables, frozen-field drift guard,
  `MIN_DF_RESIDUAL`/`MIN_PANELS_FOR_SIGNIFICANCE` floors) plus golden
  fixture tests reproducing the exact scenario each fix addressed — a
  2-config k=2 batch with per-panel diffs `[0.1, 0.3]` must NOT read
  `configEffectReal: true`, the counter-example `matts.ts` itself
  documents.
- [Risk] The webhook receiver becomes an availability dependency for
  reward accuracy — a missed delivery silently loses a verdict →
  [Mitigation] documented as a known gap; a reconciliation sweep
  (`canon reward reconcile --since <ref>`) re-derives missed verdicts
  from S4's already-ingested PR/CI state rather than relying on webhook
  delivery alone.
- [Risk] Demotion's default soft-flag means a demoted strategy's file
  still exists and could be accidentally retrieved if a caller ignores
  `status` → [Mitigation] S8's retrieval query MUST exclude
  `status: demoted` — restated explicitly in S8's own spec as a
  cross-change contract, not assumed silently.
- [Risk/Scope] The reward path the donor monorepo proves is half-wired at THREE
  independently-deferred seams, not one — this change's webhook-completion
  work must close all three, not just ship the receiver: (a) no
  SHA→trajectoryId join exists anywhere (the webhook translator's own code
  borrows a commit SHA as a trajectory-id slot, which never actually
  matches); (b) no `no-rollback` timer exists in production anywhere in
  the donor monorepo's codebase — `dev-reward-backfill.ts`'s own doc defers it as "the
  webhook-receiver wiring... lands later" and nothing ever built it; (c)
  `PatternStoreTag` resolves to `makeInMemoryPatternStore` in EVERY
  production Layer (S6's own donor gap, §10 OQ3) — a verdict this change
  writes via `mark_trajectory_verdict` is worthless if S6's store isn't
  durable when S7 lands. → Mitigation: task group 5 (webhook receiver)
  MUST cover (a) and (b) explicitly, not just the receiver→reward→
  `mark_trajectory_verdict` happy path (see tasks.md 5.4/5.5, added for
  this reason); (c) is S6's precondition, restated here as a scope
  dependency this change cannot silently assume away.

## Migration Plan

- Step 1: reward functions + `mark_trajectory_verdict` ship against S6's
  store with a synthetic/fixture verdict stream (no live webhook needed
  for initial correctness).
- Step 2: the webhook receiver ships behind a per-repo opt-in
  (`canon.yaml` `webhook.enabled`), so a consumer repo without a public
  endpoint (local-only mode per §9) is unaffected.
- Rollback: disabling the webhook receiver reverts to fixture/manual
  verdict marking only; no promoted strategy is retroactively
  un-promoted — promotion records are append-only evidence, per §7.

## Open Questions

- Exact per-role default reward weight sets beyond `dev` (ported from
  `computeDevReward`) are provisional — `content`/`design`/`review`/
  `planning`/`test` weight sets are drafted at implementation time from
  S4's verdict table, not fixed in this design.
- Whether `canon reward reconcile` (the webhook-miss mitigation) ships in
  THIS change or is deferred — flagged as a risk mitigation, not
  committed as a task, since it depends on S4's ingest surface being
  ready in time (a wave dependency, not an S6 dependency).
