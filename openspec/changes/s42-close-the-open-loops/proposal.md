# s42 — close-the-open-loops

## Why

v0.3.1 shipped with four gaps recorded honestly in s40's and s41's
"deliberately does NOT do" sections, plus one found while flipping this
repo's own task boxes. They share a shape: canon builds a mechanism, then
stops one step short of the seam that would make it usable on itself.

**1. A dispatched run is invisible to canon.** `canon dispatch begin`
writes `<repo>/.canon/dispatch/<run_id>.json` and nothing ingests it, so
no tier-backed read ever sees it. Two visible consequences: `canon
dispatch diff` reads the manifest directory as a SECOND source purely to
work around this, and `mart_flywheel_funnel` reports `retrieved 0` on
this repo immediately after a dispatch that recorded guidance into
`injected_guidance`. The funnel's corrected SQL is proven on a synthetic
corpus and starved on a real one.

**2. Superseded trajectories keep their bytes.** s41's convergence rule
withholds a superseded row from distillation, but `TrajectoryStore` has
no deletion primitive at all — `append`/`query_by_regime_key`/
`find_by_id`/`mark_verdict` — so N ids is N files forever. Every tier a
reader consumes converges; the raw layer accumulates.

**3. `applied` is a proxy, not attribution.** s40 task 3.1 is still open
because the trajectory feeding the funnel is canon-learn's parquet row,
which carries no run id, so "a resolved trajectory joined to its own run"
was unimplementable. What shipped counts an injected strategy whose
recipient run reached a terminal status — weaker, and documented as such.

**4. canon does not use its own evidence gate.** This repo has **675
checked task boxes and exactly one `EvidenceRecord`**. Not negligence:
there is no way to record one. `canon gate task` refuses an
`unevidenced-flip`, `canon gate promote` promotes staged evidence to the
committed ledger, and the only production writer of an `EvidenceRecord`
is `demo.rs`. The gate demands evidence that no command can author, so
every real flip — including the 59 this session just made — is a hand
flip. The site's hero reads "Coding agents can say they're done. Canon
makes them prove it."

## What Changes

**Reconcile dispatched runs.** `canon dispatch begin`/`end` persist the
`Run` through the same `TierRegistry` every other record goes through,
in addition to the manifest. `Run`'s natural key is its `run_id`, and
runs route to `hot` (sqlite by default), so `end` upserts the row `begin`
wrote. The manifest stays — it is the live, human-readable artifact and
the replay input — but it stops being the only place the run exists.
`canon dispatch diff` then reads runs from the tier like everything else.

**Delete a superseded trajectory.** `TrajectoryStore` gains a by-id
deletion mirroring `StrategyStore::delete_for_regime_key`, and s41's
convergence uses it, so the raw layer converges to one row per logical
verdict set instead of merely hiding the losers.

**Attribute a trajectory to its run.** `Trajectory` gains an optional
`run_id`, set when `canon ingest artifacts` runs inside a dispatched run
— passed explicitly, never inferred from timing. With runs in the tier
(change 1) the funnel can then join a resolved trajectory to the run that
carried its guidance, which is what s40 3.1 actually asked for.

**Author evidence.** A new `canon evidence add` writes a staged
`EvidenceRecord` for a task — the missing seam. `canon gate promote` then
commits it and `canon gate task` can finally flip on real evidence. This
repo dogfoods it for s42's own tasks.

## What This Change Deliberately Does NOT Do

- **No retroactive evidence for the 675 existing flips.** Fabricating
  evidence records for work whose proof was never captured would be the
  precise failure the gate exists to prevent. New flips get evidence;
  history stays honestly unevidenced.
- **`Trajectory.run_id` is never inferred.** A trajectory derived from
  artifacts outside a dispatched run has no run, and says so with `None`
  rather than guessing from timestamps or roles.
- **canon still does not route or schedule.** Recording a run and
  diffing plan against actual remains reporting.

## Impact

- `crates/canon-cli`: `dispatch.rs`, `main.rs`, `artifact_ingest.rs`, a
  new evidence module, `gate.rs`.
- `crates/canon-learn`: `store/mod.rs`, `store/parquet.rs`,
  `trajectory.rs`, `distill.rs`.
- `crates/canon-store`: `sql/views.sql`.
- `crates/canon-report`: funnel column docs.
- **Behavior change, intentional:** a dispatched run now appears in
  `canon query --kind run`. The funnel's `retrieved`/`applied` become
  non-zero once a dispatch actually happens.
