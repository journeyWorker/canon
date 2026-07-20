## Why

S6 ships a strategy store, but without reward wiring nothing ever gets a
verdict, and without statistical promotion every trajectory could blindly
become a "strategy" on a sample of one — a failure mode the donor monorepo's own MaTTS
review already caught and fixed (F1: "a single sweep NEVER auto-promotes a
strategy"). The donor monorepo ships the reward half (`dev-reward-backfill.ts`) and the
promotion half (`matts.ts`) as two separately-proven, never-cross-role,
never-fully-wired pieces: the reward substrate's own doc says its webhook
receiver "lands later," and MaTTS's corroboration gate only ever ran for the
`sim` namespace, whose deterministic simulator makes CRN replay possible.
This change completes both halves, generalized across roles.

## What Changes

- New role-specific reward functions over S4's verdict stream (PR
  merge/revert, CI, review verdicts, gate results, test ledger),
  generalizing `computeDevReward`'s weighted-composite formula beyond the
  `dev` role.
- New `mark_trajectory_verdict` completing S6's store write-back surface —
  the canon-side equivalent of the donor monorepo's `markTrajectoryVerdict`.
- New statistical promotion gate (MaTTS generalized): paired
  common-random-number (CRN) panels for roles whose domain supports
  deterministic replay (sim/tuning-like), otherwise an n-occurrence
  threshold + zero-contradiction window.
- New webhook receiver completing the donor monorepo's deferred PR/CI event ingestion —
  built on S4's already-normalized verdict-event shape, not a bespoke
  ingester.
- New demotion path: a promoted strategy that later collects a
  contradicting trajectory is demoted, not merely skipped at the next
  promotion pass.

## Capabilities

### New Capabilities
- `reward-statistical-promotion`: role-specific reward function registry,
  trajectory verdict write-back, paired-CRN statistical promotion gate,
  n-occurrence + zero-contradiction promotion gate, strategy demotion on
  contradiction, webhook receiver for PR/CI ingestion, golden fixture
  verdict streams.

### Modified Capabilities
(none — no existing `openspec/specs/` capabilities in this repo yet)

## Impact

- New logic inside/alongside `crates/canon-learn` consuming S4's
  verdict-event stream (review/handoff/artifact ingest), S6's `Trajectory`/
  `StrategyItem` stores (`mark_trajectory_verdict`, promotion writes), and
  S1's `sha`/`pr` join-spine keys ("reward signals ↔ trajectory").
- No S8 dependency — S8 consumes this change's promoted output, never the
  reverse.
- Downstream: S8's `canon retrieve` only ever surfaces strategies this
  gate promoted, and MUST exclude strategies this gate demoted.
- New `canon/skills/` companion skill for reward/promotion usage.
