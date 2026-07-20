---
name: canon-reward
description: How to read/use canon-learn's S7 reward layer — the per-role reward function registry, mark_trajectory_verdict's write-back contract, choosing a promotion gate mode (crn vs occurrence) via canon.yaml, and reading a demotion record after a contradicting trajectory arrives. Use when a role needs its trajectories scored, when deciding which PromotionGate a role should use, when a strategy's `status: demoted` front-matter needs decoding, or when wiring canon-learn into a real reward/promotion caller.
---

# canon-reward

`canon-learn`'s S7 reward layer (`crates/canon-learn/src/{reward,mark_verdict,promotion,verdict_outcome}.rs`)
turns S4's already-normalized verdict stream into a `[0, 1]` reward, a
covering trajectory verdict, and — eventually — a promotion or
demotion decision. It generalizes two things an internal monorepo
proves separately but never wires together: a `dev-reward-backfill.ts`
weighted-composite reward formula, and a `matts.ts` paired-CRN
statistical-corroboration gate.

Everything below is **library API**, not a `canon` CLI surface —
`canon-cli` wiring (a real webhook listener, `canon learn promote`,
`canon reward reconcile`) is deliberately out of `canon-learn`'s own
scope; see "What this skill does NOT cover".

## Reward function registry

`RewardRegistry` (`src/reward.rs`) holds ONE reward function per role
— never a single formula applied uniformly (design D1: a `content`/
`design`/`review` role's reward source is a review record, not git).

```rust
use canon_learn::RewardRegistry;

let registry = RewardRegistry::builtin(); // dev + 5 provisional roles
let (outcome, reward) = registry.compute_for_trajectory(&trajectory)?;
```

- `dev`'s formula (`compute_dev_reward`/`DevRewardSignals`) is the
  weighted-composite port of `computeDevReward`: `pr-merged 0.4 +
  ci-pass 0.3 + no-rollback 0.3`, a rollback or CI failure floors to
  `0.1`, a human-approval signal shortcuts straight to `1.0` (checked
  BEFORE the weighted sum, never averaged in).
- Every other built-in role (`content`/`design`/`review`/`planning`/
  `test`) currently shares the DEFAULT reward convention (`0.9`
  success / `0.3` failure / `0.5` pending, `VerdictOutcome::
  default_reward`) — PROVISIONAL, drafted from S4's review→verdict
  table, not fixed. A future change may replace any one of these with
  its own weighted composite once real cross-role data exists; nothing
  about `RewardRegistry`'s shape has to change when that happens.
- `dev_signals_from_verdicts` is the documented adapter closing the
  granularity gap between S4's collapsed `VerdictRow` and `dev`'s
  richer per-event signal set — read its doc comment before assuming a
  `VerdictRow` maps 1:1 onto a `DevRewardSignals` field.

## Writing a covering verdict: `mark_trajectory_verdict`

```rust
use canon_learn::{mark_trajectory_verdict, VerdictOutcome};

let verdict = mark_trajectory_verdict(&trajectory_store, &trajectory_id, VerdictOutcome::Success, reward)?;
```

- Rejects `VerdictOutcome::Pending` outright
  (`LearnError::CannotMarkVerdictPending`) — `Pending` is only the
  unset default every trajectory starts at (`TrajectoryVerdict::
  pending`), never a value a covering-verdict write may set. Calling
  this always MOVES a trajectory off `Pending`, never re-opens one.
- Rejects an unknown `trajectory_id` loudly
  (`LearnError::UnknownTrajectoryId`) — never a donor's own silent
  no-op.
- `reward` is re-clamped to `[0, 1]` here regardless of what the
  registry already clamped — this is the LAST gate before persistence.

## Choosing a promotion gate: `crn` vs `occurrence`

A role's `PromotionGate` is a `canon.yaml` `learn:`-section choice, not
a compile-time one — resolved via `LearnConfig`, NOT `policy.yaml`
(despite what the design doc's own decision-3 text says; see
`config.rs`'s "S7 task 3.2 reconciliation" module doc for why keeping
it in-crate avoided coupling `canon-learn` to `canon-policy`/
`canon-gate` for a setting only this crate reads):

```yaml
# canon.yaml
learn:
  promotion:
    dev:
      mode: occurrence
      n_min: 8          # optional — defaults to 5
      window_days: 14    # optional — defaults to 30
    sim:
      mode: crn
  demotion:
    hard_delete: false             # optional — soft-flag is the default
    strategies_root: canon/strategies  # optional
```

```rust
use canon_learn::{LearnConfig, PromotionMode, OccurrencePromotionGate, CrnPromotionGate, PromotionGate};
use canon_model::ids::RoleId;

let config = LearnConfig::from_manifest(&canon_yaml_text)?;
let role = RoleId::parse("dev")?;
let role_config = config.promotion_config_for(&role); // never errors — missing entry
                                                        // resolves to the conservative
                                                        // occurrence default below
let decision = match role_config.mode {
    PromotionMode::Occurrence => OccurrencePromotionGate::from_config(role_config).evaluate(&regime_key, &samples),
    PromotionMode::Crn => CrnPromotionGate.evaluate(&regime_key, &samples),
};
```

A role with NO explicit `promotion.<role>` entry at all is never a
missing-config error — `LearnConfig::promotion_config_for` resolves it
to `PromotionRoleConfig::default_occurrence()` (`mode: occurrence,
n_min: 5, window_days: 30`, the conservative defaults the design doc's
risk section calls for). `samples` in both branches is a caller's
already-resolved `TrajectoryStore::query_by_regime_key(regime_key)`
result — `PromotionGate::evaluate` itself does no I/O.

### `occurrence` (`OccurrencePromotionGate`, `src/promotion/occurrence.rs`)

For roles whose domain does NOT support deterministic CRN replay (most
of `dev`/`content`/`design`/`review` — "the permanent answer for
non-replayable domains, not a stopgap"). Promotes when, inside the
trailing `window`:

- at least `n_min` `Success`-verdict trajectories accumulate for the
  SAME `regime_key`, **and**
- zero `Failure`/`RolledBack`-verdict trajectories arrived for that
  regime.

A contradicting failure **resets** the corroboration count — it is
never averaged away. Samples strictly older than the trailing window
(measured from the evaluation instant) don't count either way; a
`Pending` sample (no covering verdict yet) is skipped, neither
corroborating nor contradicting. `RolledBack` resets the count exactly
like `Failure` — it's a STRONGER negative signal
(`VerdictOutcome::default_reward`: `0.1` vs `0.3`), so excluding it
would let a strategy promote despite a recorded rollback in its own
regime.

### `crn` (`CrnPromotionGate`, `src/promotion/crn.rs`)

For roles that CAN run a deterministic simulator (`sim`-shaped
domains): a clean-room port of `matts.ts`'s pure statistics core
(`decompose_band_variance`, `paired_contrast`, df-aware `F(1,df)`/
`t(df)` critical-value tables, the `MIN_DF_RESIDUAL`/
`MIN_PANELS_FOR_SIGNIFICANCE` floors that fixed MaTTS's own documented
F1/F2/F3 review issues). `CrnPromotionGate::evaluate` parses paired
common-random-number panel/config identity out of `Trajectory::tags`
(`crn:config=<label>` / `crn:panel=<index>` prefixes,
`CRN_CONFIG_TAG_PREFIX`/`CRN_PANEL_TAG_PREFIX`) — a CRN-capable role's
own trajectory-recording caller is responsible for stamping those tags
when it records each trajectory.

## Reading a demotion record

A previously-promoted strategy that later collects a contradicting
trajectory is demoted via `demote_strategy` — append-only: a NEW
record demotes, nothing is force-rewritten (§7).

```rust
use canon_learn::{demote_strategy, DemotionPolicy};

let record = demote_strategy(
    &strategy_store,           // &dyn StrategyStore
    strategy_id,
    contradicting_trajectory_id,
    &git_tier_root,             // e.g. <repo_root>/canon/strategies
    DemotionPolicy::default(),  // soft-flag; DemotionPolicy::HARD_DELETE to hard-delete
)?;
```

`demote_strategy` does two independent things:

1. **Durable evidence** — looks the strategy up
   (`StrategyStore::find_by_id`), and persists a `DemotionEvidence`
   (S1-envelope-shaped: `{schema, kind: EvidenceRecord, at, actor}` +
   `contradicting_trajectory_id` + `reason`) onto the row via
   `StrategyStore::mark_demoted`. Read it back off any reloaded
   `StrategyItem`:

   ```rust
   let item = strategy_store.find_by_id(&strategy_id)?.unwrap();
   match item.demotion {
       Some(evidence) => println!("demoted at {} — {}", evidence.demoted_at(), evidence.reason),
       None => println!("still active"),
   }
   ```

   `demotion: Option<DemotionEvidence>` is the WHOLE state — `None` is
   active, `Some(_)` is demoted, no separate status enum duplicating
   the same fact. `#[serde(default)]` means a pre-S7 `StrategyItem`
   row with no `demotion` key at all still deserializes as `None`.

2. **Git-tier file update** — soft-flags (default) or hard-deletes
   `<git_tier_root>/<role>/<strategy_id>.md`, **only if that file
   already exists**. Soft-flag merges `status: demoted` + `reason:
   <text>` into the file's EXISTING YAML front matter, leaving every
   other front-matter key and the whole body byte-unchanged:

   ```markdown
   ---
   title: batch the parquet writes
   description: avoids one fsync per row
   status: demoted
   reason: 'contradicting trajectory 01J... arrived for regime dev/app/auth-flow/deadbeef'
   ---
   buffer writes and flush once per namespace
   ```

   `canon learn promote` — the writer that would have CREATED this
   file in the first place — is not built yet (still `canon-cli`
   territory, out of `crates/canon-learn/**`'s own scope). A strategy
   demoted before ever being promoted to the git tier has nothing to
   soft-flag; that is NOT an error, `demote_strategy` still succeeds
   and durable evidence still lands on the `StrategyStore` row.

`demote_strategy` fails loud
(`LearnError::UnknownStrategyId`) on an unmatched `strategy_id` — never
a silent no-op, the same discipline `mark_trajectory_verdict` already
established for trajectories.

## What this skill does NOT cover

- **The webhook receiver's HTTP listener.** `webhook::
  handle_pull_request_merged`/`handle_workflow_run`/
  `evaluate_no_rollback_timer` are implemented and tested against
  synthetic payloads (`src/webhook.rs` module doc, Migration Step 1) —
  but no real HTTP server ships in `canon-learn`; wiring one is
  `canon-cli`/deployment territory.
- **`canon learn promote`** (the git-tier promotion WRITER — creates
  `canon/strategies/<role>/<id>.md` in the first place) and **`canon
  gate` / CEL policy authoring** — see the `canon-policy` skill, and
  its own explicit non-CEL boundary: "Reward functions (S7) stay
  versioned Rust. `canon-policy` has no reward-scoring entry point."
- **The `role`/`trajectory`/`strategy`/`store`/`distill`/`rebuild`/
  `retrieve` layers underneath this** (S6) — see `src/lib.rs`'s module
  doc for that inventory; this skill only covers the S7 reward layer
  built on top of it.
