## Why

Task/evidence completion across the team's tooling is **self-reported** today
(design doc §1: openspec `tasks.md` checkboxes ⊥ the donor CLI's handoff queue ⊥
`.superpowers/sdd/progress.md`, zero evidence backing any flip). The donor
parity harness proves the fix — a two-layer static/dynamic
trust spine with a human flag ratchet — but it is trapped in a repo-local
110KB Python script. The donor monorepo's donor CLI ships an overlapping but partial version
(evidence-gated task flip + fabrication-marker scanning, no coverage gate, no
trust ladder, no staleness, no staging→promote). S5 generalizes the donor parity harness's
machinery into `canon-gate` and makes canon — not the donor CLI, not the donor parity harness — the
format authority for the openspec task/checkbox grammar itself.

## What Changes

- New `canon-gate` crate implementing the two-layer trust spine: static
  coverage (does required evidence exist per policy) + dynamic verdict ledger
  (did it pass, by whom, how stale) — generalized from the donor parity harness's D3.
- New trust ladder: `draft → reviewed → ratified`, with `flagged` as a
  human-only, sticky, one-way overlay — generalized from D21. Bare
  `reviewed` promotion with no review-record is a gate failure
  (`unreviewed-promotion`).
- New policy-derived requirement routing (`policy.yaml`, D7 pattern): facts
  live on artifacts as tags, routing lives centrally; tightening coverage is
  a policy diff, never a corpus retag.
- New staleness policy: surface-scoped git-diff when a surface ref is
  declared on the evidence record, else a `max_commits_behind` ceiling.
- New staging→promote workflow: reviewers write unordered `_staging/`
  records; `canon gate promote` (the O13 serialized integrator step) assigns
  a monotonic per-(role, surface) `run_seq`, re-validates each record with
  the SAME check the gate applies before it lands, and refuses (never
  commits, never consumes a run_seq) a malformed staging record.
- New canon-owned openspec task/checkbox grammar: canon parses and writes
  `- [ ] ` / `- [x] ` rows (including `**DEFERRED to §<to>**` /
  `**DROPPED**` annotations and the ` — ✅ <evidence>` suffix) directly —
  canon is the format authority, not a caller of the donor CLI's parser.
- New `canon gate task <task_id>` CLI command: requires a matching
  `EvidenceRecord` before flipping a gated task; fabrication-marker scanning
  over structured evidence fields only (never free prose).
- New hook-seam wiring: `.claude/settings.json` / `.codex/hooks.json` entries
  invoking `canon gate task` in the same shape the donor CLI's `hook run <kind>`
  entries already use, plus a generic pre-commit script for non-donor-CLI repos.
- New fixture-corpus selftest (`canon gate selftest`) with EXPECTED-violation
  files, one per stable failure-class string, GateCtx-style rebindable roots.
- **Migration-target boundary (not executed in this change):** the donor CLI's
  task-flip logic (`flipTaskDone` +
  `scanFakeMarkers`) becomes a documented migration target for a follow-up,
  donor-CLI-side change that swaps its callers to shell out to
  `canon gate task` — the same treatment the donor parity harness gets at S11. This change
  ships the capability and the boundary, not the donor-CLI-side cutover.

## Capabilities

### New Capabilities
- `trust-spine-gate`: two-layer covered-vs-green evaluation, D21 trust
  ladder + human-only flag ratchet, D7 policy-derived requirement routing,
  surface-scoped/ceiling staleness, staging→promote monotonic `run_seq`,
  stable failure-class strings, fixture-corpus selftest.
- `gated-task-completion`: canon-owned openspec checkbox grammar, evidence-
  gated task flip, fabrication-marker scanning, `canon gate task <task_id>`
  command, hook-seam wiring (Claude/Codex settings + generic pre-commit
  script), the donor-CLI migration-target boundary.

### Modified Capabilities
(none — no existing `openspec/specs/` capabilities in this repo yet)

## Impact

- New `crates/canon-gate` (Rust), consuming `canon-model`'s (S1) envelope
  types (`EvidenceRecord`, `Task`, `Change`, join-spine `task_id`/`change_id`)
  and `canon-store`'s (S2) git-tier adapter for `_staging/` and promoted
  ledger writes.
- Downstream: the donor CLI's task-flip and marker-scan logic
  become migration targets (not
  edited by this change); the donor monorepo's `.claude/settings.json` /
  `.codex/hooks.json` gain new hook-seam entries alongside the existing
  `hook run <kind>` entries (additive); the donor parity harness
  gets its own delegation boundary in S11 (not this change).
- New `canon/skills/` companion skill for `canon gate` usage.
