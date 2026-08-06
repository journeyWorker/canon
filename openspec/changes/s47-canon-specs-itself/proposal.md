# s47 — canon-specs-itself

> s44 shipped `spec_coverage`, the check that starts from the Scenario
> corpus and reports a spec nobody attested to. canon never turned it on
> for itself. Asked why, the answer turned out not to be "canon does not
> follow its specs" — it was that canon had no specs.

## Why

`canon.yaml`'s spec configuration, before this change, was one root:

```yaml
specs:
  roots:
    - id: platformer
      root: examples/platformer/specs
```

That is the demo — `examples/platformer` is a browser game: `index.html`,
PixiJS bundles, `red-panda.png`, `pinecone.png`. Every one of the sixteen
`Scenario` records in canon's own ledger is a `platformer.*` describing
it:

```
platformer.hud.01     Pause freezes the simulation and resume continues it
platformer.enemy.01   Losing every heart shows game over and returns to menu
platformer.levels.01  Completing a level unlocks the next in the menu
```

Zero `.feature` files describe canon. So enabling `spec_coverage` would
not have measured canon; it would have gated canon's releases on a demo
game, and going green would have meant authoring twelve evidence records
claiming canon's test suite verifies a platformer's physics. Four such
records already exist, authored as s44 phase-0 fixtures:

```json
{ "scenario_id": "platformer.movement.01",
  "evidence": { "kind": "test-run",
                "ref": "cargo test -p canon-gate spec_coverage" },
  "verdict": "faithful" }
```

`platformer.movement.01` is "Left world edge clamps the player". That
record says a canon unit test verified it. As a demonstration that the
check fires it was honest and it was documented as one; as coverage it is
the fabricated-evidence shape this whole project exists to refuse.

## What changes

**canon gets a spec corpus of its own.** A second `specs.roots[]` entry,
id `canon`, root `specs/`. It starts with one area — `finding`, whose
behavior s46 just built and whose tests are named and passing — and
grows one area at a time. Eight scenarios, each attested by an
`EvidenceRecord` whose `ref` is the exact `cargo test` invocation that
exercises it, every one of them run and observed passing before the
record was written.

**`spec_coverage` is enabled, scoped by subject status.** `.canon/
policy.yaml` gains `require_evidence: true` with
`scope: [building, verifying]`. This is `scope` used exactly as
documented — "narrow blocking to work a subject says is underway" — not
a project filter wearing a disguise:

- canon's eight scenarios carry `@subject:finding-lifecycle`, a Subject
  at `verifying`. Underway, so blocking.
- Six platformer scenarios carry `@subject:platformer-traversal`, at
  `proposed`. Nobody is building it, so not blocking. If someone moves
  that subject to `building`, its scenarios start blocking — which is
  the mechanism working, not a leak.
- Ten platformer scenarios carry no subject at all. A demo is not a
  product unit, so it has no Subject, so a configured `scope` places it
  outside.

**`canon feature new` / `canon scenario new` gain `--project <id>`.**
Both refused outright on a multi-root config, with no override. Adding
the second root would have broken them; the flag selects a configured
root by the id that becomes the record's `project_id`.

## Three defects this work surfaced

**`canon format spec` was checking a directory that does not exist.**
`canon format` takes the corpus root as a POSITIONAL argument, and this
repo has no `spec/` — only `specs/` and `examples/platformer/specs`. So
every release verification that ran `canon format spec` and reported
`0 file(s) checked, 0 violation(s)` was reporting on nothing. The count
was there the whole time and nobody read it. Recorded as a finding; the
fix is that the verification invocations name real roots, and the
zero-file case is worth a louder answer than silent success.

**The four platformer evidence records remain, and stay OPEN.** The
ledger is append-only and canon has no delete, correctly. What s47
changes is that canon's coverage no longer rests on them — but the
records still say what they said, and canon has no verb that retracts
an attestation. `canon finding close` moves a finding's disposition;
nothing moves an `EvidenceRecord`'s. So this finding is recorded and
left `open`, which is the true answer: a known defect with no mechanism
to resolve it yet. Closing it would be the fabrication this change is
about.

**`canon query --kind scenario` disagreed with the gate about the
corpus.** `canon inventory sync` re-materializes a `Scenario` per scan,
so a `.feature` edit plus a re-sync leaves both generations committed.
`spec_coverage` has folded them since s44 — it must, or it judges one
scenario twice and may judge the stale copy — but `canon query` did not,
and read this repo's 24 scenarios as 38. A reader deciding what to
attest and the gate deciding what is unattested were seeing different
corpora. `Scenario` joins `Subject` and `Finding` in `canon query`'s
re-written-kind fold. `Review` deliberately does not: its natural key
carries the pinned sha, so two attestations at two commits are two keys,
not two versions.

## What this does NOT establish

Eight scenarios in one area is a beginning, not coverage of canon. Most
of canon's fourteen record kinds and twenty-odd commands have no
`.feature` yet, and `scope` is what keeps that honest — an area with no
Subject at `building`/`verifying` blocks nothing, and is visibly absent
rather than silently green.

A passing `spec_coverage` means every in-scope scenario has an evidence
record whose verdict is not divergent. It does not mean the cited test
exercises the scenario it is attached to. canon does not read the test,
does not run it, and cannot tell a precise citation from a plausible
one. The author of an attestation and its beneficiary are the same
party — the gap `canon evidence add` states, unchanged here. What the
corpus buys is that the claim is now WRITTEN DOWN, per scenario, and a
wrong one is a reviewable line rather than an absence.
