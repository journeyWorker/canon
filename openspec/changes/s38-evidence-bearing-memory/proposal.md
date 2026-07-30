# s38 — evidence-bearing-memory

## Why

s37 closed the reward flywheel's two severed links, so the loop now
turns end to end. Dogfooding the turning loop immediately exposed two
defects that were invisible while it was broken.

**1. The flywheel grinds metadata, not insight.** `distill_trajectory`
builds a strategy's entire text from `Trajectory.task` (→ title) and
`Trajectory.context` (→ content). But `canon ingest artifacts`
synthesizes those two fields from a description of its own plumbing:

```rust
let task = format!("{verdict_count} verdict(s) derived from canon-ingest artifact adapters for regime {regime_key}");
let context = format!("canon ingest artifacts: {verdict_count} VerdictRow(s) folded onto regime_key {regime_key} by the S14 artifact-ingest driver");
```

So `canon retrieve` returns, verbatim from this repo:

```
title:   "2 verdict(s) derived from canon-ingest artifact adapters for regime test/canon/platformer/41fdd8c50ee6"
content: "canon ingest artifacts: 2 VerdictRow(s) folded onto regime_key ... by the S14 artifact-ingest driver"
```

Injecting that into an agent's context teaches nothing. The real evidence
is sitting in `ArtifactEvent.detail` (reviewer pins, divergence prose,
task evidence notes) and `ArtifactEvent.join_key`, and the driver
discards it. The distiller is faithfully passing through garbage — the
defect is upstream of it.

This matters more than it looks. The whole argument for canon as an
anchor layer is that promoted strategy memory is grounded evidence rather
than a model agreeing with itself. Memory whose text is a description of
the ingest driver is not grounded in anything; it is the echo chamber
with extra steps.

**2. Same-`at` supersession picks arbitrarily.** A plan-derived record's
`Envelope.at` is `file_modified_at(<source doc>)` — byte-stable, never
wall-clock (s20 D7), which is correct: it is what makes re-importing an
unchanged source idempotent. But a canon CODE change does not advance the
source file's mtime, so the stale and fresh records for one `task_id`
carry an identical `at`, and `fold_latest_by_key` falls back to s21 D3's
lexicographic `digest` tie-break. The winner is therefore arbitrary
**per row**.

Observed right after s37 added `Task.depends_on`: `canon query --kind
task` surfaced the field on some ridge-v2 tasks and not others within the
same file, with zero import diagnostics. It looks exactly like a parser
bug and is not — three plausible structural hypotheses (distance from
heading, section shape, singular-vs-range refs) were chased and all
failed to correlate before the store fold was identified.

Two things are missing. A re-parse never even happens, because the
plan-import cursor is keyed only on dialect + root + per-file digests, so
a parser change looks like "unchanged". And even after a forced
re-parse, the fold has no ordering signal between the two generations.

## What Changes

**Evidence-bearing trajectories** (`canon-ingest` + `canon-cli`, no model
change):

- `ArtifactJoinKey::as_str`, `ArtifactEventKind::label` (a human phrase
  per variant, because these strings end up inside retrieved agent
  guidance), and `ArtifactEvent::evidence_line` — one compact line
  quoting the most salient prose the event actually carries, never the
  serialized `detail` blob, length-capped so one pathological record
  cannot bloat every retrieved strategy.
- `canon ingest artifacts` stops discarding the event. A trajectory's
  `task` names its concrete artifact and what happened to it; its
  `context` is the deduped evidence lines. Deterministic, so
  `trajectory_content_digest` idempotence is preserved.

**Deterministic supersession** (`canon-store` + `canon-model` +
`canon-ingest` + `canon-cli`):

- `fold_latest_by_key` becomes a total order over `(at, schema, digest)`.
  This STRENGTHENS s21 D3 rather than weakening it: `Envelope.schema` is
  the per-kind format generation and is data the item itself carries, so
  the fold stays a pure, machine-independent function of the input set,
  and `digest` remains the final tie-break.
- `Task.schema` → `2`. It gained `depends_on`, and `Task` is a
  byte-stable-`at` kind, so a field addition IS a generation change.
  Every other kind stays at `1`: `Run` and `Handoff` also gained fields
  in s37, but their `at` is derivation-time and cannot tie this way.
- `PlanAdapter::parse_version()` — required, not defaulted, so a new
  dialect decides deliberately — folded into the plan-import cursor id.
  Both shipped dialects go to `2`, because both gained dependency
  extraction and their output for an unchanged source genuinely changed.

## What This Change Deliberately Does NOT Do

- **No LLM distiller.** `canon-learn`'s distiller stays deterministic and
  non-LLM by design (S6 decision 6: an LLM-backed distiller is a future,
  separately-injected impl, never a dependency this crate takes on). This
  change fixes what the deterministic distiller is fed, which is the
  actual defect.
- **`fold.rs`'s digest tie-break is not weakened.** It is what guarantees
  s21 machine-independence. Adding a meaningful precedence above it is
  the fix; removing it would trade a correct invariant for a symptom.

## Impact

- `crates/canon-ingest`: `artifact_adapter.rs`, `plan_adapter.rs`,
  `plan_adapters/{openspec,superpowers}.rs`.
- `crates/canon-cli`: `artifact_ingest.rs`, `plans.rs`, plus the
  `fold_latest_by_key` call sites in `subject.rs`, `query.rs`,
  `inventory.rs`.
- `crates/canon-store`: `fold.rs` (+ `tests/pg_tier_live.rs`).
- `crates/canon-gate`: `ledger.rs`, `staleness.rs`.
- `crates/canon-report`: `divergence.rs`. `crates/canon-plugin`:
  `project.rs`.
- `crates/canon-model`: `records.rs` (`Task` schema), regenerated
  `schemas/task.schema.json`, `fixtures/well-formed/task.json`.
- `canon/skills-dev/state-model/SKILL.md`: the additive-field trap is now
  documented, and two stale counts (twelve kinds, eight join keys) are
  corrected to thirteen and nine.
- **Behavior change, intentional:** `canon ingest plans` re-parses once
  after this lands (the parse-version bump invalidates every plan
  cursor), and retrieved strategy text changes shape entirely.
