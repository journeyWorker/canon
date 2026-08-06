# s48 — the-whole-surface-specced

> s47 gave canon a spec corpus of its own and ended with a stated limit:
> one area out of twenty-odd, and the rest "visibly absent rather than
> silently green". This removes the limit.

## Why

s47's closing paragraph was honest and it was also a place to stop:

> Eight scenarios in one area is a beginning, not coverage of canon.
> Most of canon's fourteen record kinds and twenty-odd commands have no
> `.feature` yet, and `scope` is what keeps that honest — an area with
> no Subject at `building`/`verifying` blocks nothing, and is visibly
> absent rather than silently green.

`scope` genuinely does keep that honest. But "honest about what is
missing" is a property of the reporting, not of the repo. canon ships a
check whose whole argument is that a specified-but-unattested behavior
should be visible, and canon was running it against one twenty-first of
its own surface. The gate was clean because almost nothing was in scope.

The surface, counted: 25 commands, 14 record kinds, 1673 tests across
ten crates. One area specced.

## What changes

**A Subject per area, all at `verifying`, so all of it blocks.** Twenty
areas join `finding`, each with its own Subject and its own
`.feature` file(s) under `specs/features/kind=feature/area=<area>/`:

`context` `format` `authoring` `inventory` `gate` `evidence` `finding`
`review` `divergence` `subject` `ingest` `query` `report` `retrieve`
`learn` `dispatch` `tier` `plugin` `policy` `vocab` `skills`

`spec_coverage`'s `scope: [building, verifying]` is unchanged, and its
meaning is unchanged. What changes is that nearly everything canon does
is now inside it.

**Every scenario is attested by a test that was RUN first.** Same rule
as s47, applied at scale: a scenario cites the exact `cargo test`
invocation that exercises it, and that invocation was executed and
observed passing before its `EvidenceRecord` was written. The corpus is
BOUNDED by what is verifiable — a behavior with no test does not get a
scenario, it gets recorded as a gap.

**Scenarios describe behavior, not implementation.** A refusal, a state
transition, a derived number, an idempotence property, a boundary. Never
a function name. The corpus has to be readable by someone who has not
seen the code, or it is a second copy of the test names with worse
tooling.

## What does NOT change

- `examples/platformer` stays out of scope, and stays a demo. Its
  Subject is at `proposed` and ten of its scenarios have no Subject at
  all. A demo is not one of canon's product units, and pinning it to a
  `verifying` Subject to make a number go up is the move this whole
  gate exists to refuse.
- `scope` is not widened, removed, or special-cased. The gate turns red
  the same way it did before; there is simply far more inside it.
- No test is written to make a scenario attestable. The corpus follows
  the tests, never the reverse — writing a test to satisfy a spec you
  just wrote, in the same pass, attests nothing.

## What this does NOT establish

A green `spec_coverage` means every in-scope scenario has a non-divergent
evidence record. It does not mean the cited test exercises the scenario
it is attached to. canon does not read the test, does not run it, and
cannot tell a precise citation from a plausible one — the citation is a
claim by an actor, exactly like every other attestation in this ledger,
and the author of a claim and its beneficiary remain the same party.

What the corpus buys is that the claim is WRITTEN DOWN, per scenario,
against a named test, in a file a reviewer can read against the code. A
wrong citation is now a reviewable line. Before it was an absence, and
absences do not get reviewed.

Coverage of the SURFACE is not coverage of the BEHAVIOR. Twenty-one
areas with a handful of scenarios each describes what each command
does; it does not exhaust what each command does. The honest claim is
that canon's own gate now measures canon across its whole command
surface, not that canon is fully specified.
