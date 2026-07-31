# s41 — review-hardening

## Why

Three reviewer agents reviewed s37–s40 before merge. All three returned
`overall_correctness: incorrect`, with six blockers between them. Every
one was a real defect that the branch's own test suite, gate, drift
check, and dogfood had all passed over — because each guards a contract
the defect did not violate.

The two worst were things this session had already *observed* and
misread:

- **A stale trajectory could never converge.** `trajectory_content_digest`
  covered `regime_key` + verdict rows but not the rendered text, and the
  duplicate check ran before `trajectory_text` was computed. So every
  trajectory written before s38/s39 kept its digest, was skipped as a
  duplicate, and its ingest-driver prose was retained and re-distilled
  forever. During development this surfaced twice as "the new text isn't
  showing up", and both times it was worked around with
  `rm -rf .canon/learn`. That workaround was the bug report.
- **A dispatched run's plan-task binding accepted tasks that do not
  exist.** `--task` validation treated `WriteBackError::Unsupported` as
  proof of existence, but the superpowers write-back returns
  `Unsupported` for *every* id once it has located the change document.
  So `--task <real-change>#999` persisted exactly the dangling binding
  the validation was added to prevent.

The rest: the antecedent join keyed scenarios without `project_id`, so
one project's finding attached to another's verdict; the Claude adapter
stayed at parse version 1 although s37 changed its sidechain output, so
existing installs would never backfill execution lineage; `dispatch end`
could overwrite a closed run's provenance and accepted a terminal status
with no `ended_at`; and the funnel inner-joined `injected_guidance`
against current strategy ids, which `rebuild_namespace` regenerates —
so `retrieved` and `applied` silently decayed to zero after any rebuild,
while the panel prose asserted the opposite.

## What Changes

**Trajectory identity covers what it renders.** The digest folds an
identity version plus the rendered `task`/`context`; a separate
derivation key names the logical verdict set, and a stored row from this
driver carrying the same derivation key is superseded in place by
reusing its id. An upgrade converges without deleting anything.

**One event identity, carrying `project_id`.** Scenario events key as
`scenario:<scenario_id>@<project_id>`, with `@-` for an absent or
malformed project (unreachable as a real id: `ProjectId` cannot begin
with a hyphen). The same string feeds the antecedent lookup and
`regime_hash`, so the join and the regime can no longer disagree.

**Deterministic `StrategyId`.** Derived from the distilled row's own
content instead of minted, so `rebuild_namespace` is a fixpoint. This
closes a second, independent bug: `canon learn promote <ULID>` used to
target a value the next ingest silently replaced, orphaning the git-tier
file and leaving `demote` unable to resolve its subject.

**`--task` resolves membership from the parsed task set**, searching
every configured source before rejecting. **`dispatch end`** holds an
exclusive lock across read → validate → replace, permits only
`Running + ended_at: None` → terminal, and verifies the manifest's
`run_id` matches its filename. **`dispatch diff`** deserializes manifests
as typed `Run`s and reports scan failures instead of silently yielding
zero edges. **The Claude adapter is at parse version 2.**

Plus the ordering and counting fixes the reviews turned up: verdicts are
sorted before rendering, scenario titles resolve equal-`at` ties by
`(at, schema, digest)`, omitted antecedents are counted once, and a
`Deferred` reason is reachable from its serialized position.

## Migration — one-time, on a derived store

`Envelope.schema` is untouched; nothing about the *record* format
changes. What changes are two derived-store identities, so the
regenerable tier re-keys once:

- `.canon/learn/**` — delete it once after upgrading, then run
  `canon ingest artifacts`. Trajectories are re-derived from the ledger,
  so this is lossless. Skipping it is not harmful but leaves the
  pre-upgrade rows orphaned under their old regime keys, where they
  inflate `canon report`'s aggregates forever (observed here: verdicts
  counted 60 instead of 30).
- `.canon/strategies/<role>/<old-ULID>.md` — any promoted file must be
  deleted and re-promoted against a current id. This repo had none.

## What This Change Deliberately Does NOT Do

- **No backward-compat shim for either id.** canon has no external
  consumers; a clean cutover with a documented one-time migration beats
  a permanent translation layer.
- **`project_id` is not dropped from `regime_hash` to avoid the
  re-key.** Two projects sharing a scenario id must not fold into one
  regime — that is the same correctness bug as the antecedent bleed, and
  trading it for migration comfort would be the wrong way round.

## Impact

- `crates/canon-cli`: `artifact_ingest.rs`, `dispatch.rs`, `ingest.rs`.
- `crates/canon-ingest`: `artifact_adapter.rs`, `adapters/claude.rs`,
  `adapter.rs`.
- `crates/canon-learn`: `ids.rs`, `distill.rs`, `rebuild.rs`,
  `store/mod.rs`.
- `crates/canon-store`: `sql/views.sql`.
- `crates/canon-report`: `marts.rs`, `render.rs`.
- `scripts/check-release-workflow-safety.py`: asserts every
  `package.json` carries the Cargo workspace version — the drift it now
  catches (three published manifests stranded on `0.1.0` at tag
  `v0.2.1`) is what prompted the version alignment to `0.3.0`.
- Skills re-materialized into `.claude/`/`.codex/`, which had been left
  stale by earlier source edits.
