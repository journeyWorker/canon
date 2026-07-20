## Why

S6/S7 produce promoted strategies, but nothing injects them into an actual
agent dispatch, and without verbatim manifest recording a run could never be
reproduced once live memory state changes. The donor tuning project's replay-by-manifest
pattern already proves the exact mechanism canon needs, generalized beyond
`sim` sweeps to every role's dispatch.

## What Changes

- New `canon retrieve --role <r> --regime <k>` command: top-k strategies +
  guardrails for a role/regime, advisory and fail-soft (never blocks a
  dispatch, never gates reproducibility).
- New verbatim manifest recording: injected guidance recorded byte-for-byte
  into the run manifest (S1's `run_id`-keyed record), mirroring
  the donor tuning project's `SweepManifest.injectedGuidance` /
  `manifestGuidanceForReplay` contract.
- New generic pre-dispatch hook script + the donor CLI's wiring for the donor monorepo, reusing S5's
  hook-seam wiring shape applied to dispatch instead of task-flip.
- New replay-determinism guarantee: replaying a manifest re-injects
  `injected_guidance` verbatim — never a fresh live-retrieval call — so a
  changed store never changes a replay's inputs.
- New demoted-strategy exclusion on the read side: retrieval excludes
  `status: demoted` strategies, restating S7's demotion contract as a hard
  requirement here.

## Capabilities

### New Capabilities
- `retrieve-before-task`: role/regime-scoped advisory retrieval, fail-soft
  retrieval contract, verbatim manifest guidance recording, replay
  determinism, demoted-strategy exclusion, pre-dispatch hook wiring.

### Modified Capabilities
(none — no existing `openspec/specs/` capabilities in this repo yet)

## Impact

- New CLI surface (`canon retrieve`) consuming S6's `StrategyItem` store
  (read-only) and S1's `run_id`/manifest join-spine record.
- Depends on S7's demotion contract (excludes `status: demoted` strategies
  from retrieval — a cross-change contract restated here on the read
  side).
- Downstream: the donor monorepo's dispatch-time wiring (a pre-dispatch hook alongside
  the existing `pre-edit-pattern-lookup.ts` hook) + a generic pre-dispatch
  script for other consumers.
- New `canon/skills/` companion skill for `canon retrieve` usage.
