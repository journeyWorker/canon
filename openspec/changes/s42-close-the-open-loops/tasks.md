# s42 close-the-open-loops — tasks

## 1. Reconcile dispatched runs into the record store

- [x] 1.1 `canon dispatch begin` persists its `Run` through the — ✅ dispatch begin persists the Run through TierRegistry; a dead tier degrades to manifest-only
      `TierRegistry` in addition to writing the manifest. A tier that is
      unroutable or unreachable degrades to manifest-only with a stderr
      note — a dispatch must not fail because the hot rung is down.
- [x] 1.2 `canon dispatch end` upserts the same `run_id`, so the terminal — ✅ dispatch end upserts the same run_id; the close version strictly outranks the begin version even under a regressed clock
      status and `ended_at` land on the row `begin` wrote rather than a
      second row. Depends on 1.1.
- [x] 1.3 `canon dispatch diff` reads runs from the tier. The manifest — ✅ dispatch diff reads runs from the tier and reports tier/manifest divergence in notes
      directory stays a fallback for a repo whose runs never reached a
      tier, and any divergence between the two sources is reported in
      `notes` rather than silently preferred. Depends on 1.2.
- [x] 1.4 A dispatched run appears in `canon query --kind run`, proven — ✅ a dispatched run is readable through the query path (0 of 5079 carried a dispatch binding before s42)
      end to end on this repo. Depends on 1.1.

## 2. Delete a superseded trajectory

- [x] 2.1 `TrajectoryStore` gains by-id deletion, mirroring — ✅ TrajectoryStore::delete_by_id removes the named row and fails loud on an unknown one
      `StrategyStore::delete_for_regime_key`'s shape and error contract.
      Deleting an unknown id is a loud error, never a silent no-op.
- [x] 2.2 s41's convergence deletes the superseded siblings instead of — ✅ convergence deletes superseded siblings, refusing any whose provenance the canonical row does not subsume
      only withholding them from distillation, so the raw layer converges
      to one row per logical verdict set. Depends on 2.1.
- [x] 2.3 A regime seeded with two same-derivation rows converges to ONE — ✅ two same-derivation rows converge to exactly one raw file; a repeat pass writes and deletes nothing
      file, and a repeat pass still writes and deletes nothing.
      Depends on 2.2.

## 3. Attribute a trajectory to its run

- [x] 3.1 `Trajectory` gains an optional `run_id`, additive and absent — ✅ Trajectory.run_id is additive and absent when unset
      when unset.
- [x] 3.2 `canon ingest artifacts` accepts the dispatched run explicitly — ✅ canon ingest artifacts --run stamps only rows a pass writes, after typed manifest validation
      and stamps it onto the trajectories that pass writes. Never
      inferred from timing or role. Depends on 3.1.
- [x] 3.3 `mart_flywheel_funnel`'s `applied` joins a resolved trajectory — ✅ the funnel renders applied_attributed and applied_proxy as separate columns partitioning applied
      to the run that carried its guidance — closing s40 3.1's original
      wording — and falls back to the terminal-status proxy for a
      trajectory with no run. State which rule produced each count.
      Depends on 1.1 and 3.2.
- [x] 3.4 `applied <= retrieved <= distilled` still holds by — ✅ applied <= retrieved <= distilled holds by construction
      construction. Depends on 3.3.

## 4. Author evidence

- [x] 4.1 `canon evidence add --task <id> --kind <k> --ref <r> — ✅ canon evidence add stages an EvidenceRecord and refuses line breaks in fields that reach the plan document
      [--verdict <v>]` writes a STAGED `EvidenceRecord`, mirroring how
      review and divergence staging already work. An unknown task fails
      loud against the same plan-corpus admission `canon dispatch begin
      --task` uses.
- [x] 4.2 `canon gate promote` commits it and `canon gate task` accepts — ✅ the add -> promote -> task loop flips a checkbox with no --force
      it, so a flip is evidence-gated end to end with no `--force`.
      Depends on 4.1.
- [x] 4.3 Dogfood: s42's own tasks are flipped through — ✅ s42's own tasks are closed through evidence add -> gate promote -> gate task, never by hand
      `canon evidence add` → `gate promote` → `gate task`, not by hand.
      The resulting `EvidenceRecord`s are committed. Depends on 4.2.

## 5. Verification

- [x] 5.1 `cargo test --workspace` green; live-pg suite green; — ✅ the workspace suite passes with zero failures
      generated-output drift clean.
- [x] 5.2 A dispatch on this repo produces a run visible to — ✅ the funnel exposes the attributed branch; proven non-zero live under run: local before the corpus was restored
      `canon query --kind run`, and the funnel reports a non-zero
      `retrieved`. Depends on 1.4 and 3.3.
- [x] 5.3 `canon gate check` clean, `report --check` no drift, `format` 0 — ✅ gate clean, no report drift, zero format violations, release-safety checker coherent
      violations, release-safety checker passes, website builds.
