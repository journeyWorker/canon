# s39 — joined-evidence-grounding

## Why

s38 stopped `canon ingest artifacts` from describing its own plumbing and
made a trajectory quote the prose its own `ArtifactEvent` carries. Then a
question exposed that "its own event" is the wrong boundary, and that a
claim shipped in s38 — including into the website — is misleading.

**The claimed ceiling is not a ceiling.** s38 recorded that a
review-derived strategy can only ever quote its pin sha because
`records::Review` carries no prose field. The first half is true;
`Review` is exactly `{envelope, project_id, scenario_id, reviewer, pin,
provenance_ref}`. The conclusion is wrong, because canon's join spine
exists precisely so a record does not have to carry every fact about
itself. Two joins are available and unused:

- `Scenario` carries `title` — a real sentence
  (`platformer.moving.01` → "A moving platform carries the standing
  player"). A review attests `(project_id, scenario_id)`, so its
  trajectory can name what was attested instead of only its sha.
- `Review.provenance_ref.original_spec_ref` points at the spec file the
  attestation came from.

**And the corpus's best evidence is discarded.** The richest prose canon
holds on this repo is the eight `open` divergence findings — concrete,
file-and-line ship-blockers:

```
SHIP-BLOCKER examples/platformer/src/App.tsx:45-46 calls sim.setPaused() inside a React updater
SHIP-BLOCKER examples/platformer/src/engine/simulation.ts:151 clears jumpBuffered after ...
```

None of it reaches strategy memory. `derive_native_divergence_verdict`
maps `Open | Deferred => None`, which is CORRECT — an open finding is not
an outcome yet — so those events produce no verdict and
`artifact_ingest`'s accumulation loop skips them. What survives is the
resolution narrative alone: "Fixed and independently re-verified+attested
at 505a668e".

So canon distills *that it got fixed* and throws away *what was broken*.
The finding plus its resolution is the single most valuable strategy shape
available — a concrete pitfall with a demonstrated fix — and both halves
are already parsed, in the same batch, in `all_events`, one loop before
they are dropped.

## What Changes

Two independent read-side joins in `canon ingest artifacts`. No new record
kind, no schema bump, no new tier read for the first one, and no LLM.

**Antecedent findings.** When a verdict-bearing event becomes a
trajectory, the non-verdict events sharing its `join_key` that precede it
in time contribute their prose as antecedent evidence. A `resolved`
divergence therefore distills with the `open` findings it closed. Ordered,
deduped, and capped, so trajectory content stays deterministic and
`trajectory_content_digest` idempotence holds.

**Scenario titles.** The driver reads the `Scenario` ledger index once and
indexes `(project_id, scenario_id) -> title`. Any trajectory whose
`join_key` is a scenario names it in human terms. This is the read that
turns `platformer.moving.01: review attestation` into a line naming what
the attestation actually covers.

## What This Change Deliberately Does NOT Do

- **Does not make `Open` verdict-bearing.** An open finding is still not
  an outcome, and `derive_native_divergence_verdict` is untouched. Open
  findings contribute *evidence text* to a trajectory that some other
  event's verdict created; they never mint a verdict, a reward, or a
  trajectory of their own.
- **Does not weaken the divergence fold.** The findings were never folded
  away — `fold_handoff_records` is handoff-only. The gap was the verdict
  filter, not supersession.
- **No LLM distiller.** Still S6 decision 6: the deterministic distiller
  stays; this changes what it is fed.

## Impact

- `crates/canon-cli`: `artifact_ingest.rs`.
- `packages/website`: the `strategy-memory` pages assert the pin-sha
  ceiling as a model-level limit. That claim ships to readers and is
  wrong — it must be corrected in both languages.
- **Behavior change, intentional:** trajectory `task`/`context` text
  changes shape again, so `.canon/learn` re-derives on the next ingest.
