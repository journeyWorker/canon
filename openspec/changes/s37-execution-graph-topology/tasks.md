# s37 execution-graph-topology — tasks

## 1. canon-cli — flywheel repair (D1/D2/D3/D4)

- [x] 1.1 `canon ingest artifacts` resolves each freshly persisted
      trajectory's covering verdict: `RewardRegistry::builtin()` +
      `compute_for_trajectory` + `mark_trajectory_verdict`, called
      BEFORE `rebuild_namespace` (D4). A `VerdictOutcome::Pending`
      result is counted and left pending, never forced (D3).
      `ArtifactIngestOutcome` gains `trajectories_marked` +
      `trajectories_left_pending`, surfaced in both `format_human` and
      `--json`.
- [x] 1.2 `canon learn promote` evaluates the per-role promotion gate
      before any write: resolve the item's `regime_key`/`role`, read that
      regime's trajectories, pick `PromotionMode` from `canon.yaml`, call
      `evaluate_now`. Fails closed, no `--force` (D2). A blocked
      `--dry-run` still renders its preview, then reports the refusal.
- [x] 1.3 `tests/learn_promote.rs` reflects the gate: promoting fixtures
      seed corroborating trajectories; new cases cover an unproven
      strategy blocked, `Pending` samples never corroborating (the exact
      pre-repair repo state), a later contradiction resetting the streak,
      a blocked dry-run, and `canon.yaml`'s `learn.promotion.<role>`
      config driving the gate (previously a dead config path).

## 2. canon-model — additive lineage fields (D5)

- [x] 2.1 `Run.parent_run_id: Option<RunId>` + a `with_parent_run_id`
      builder matching the existing `with_injected_guidance` style.
- [x] 2.2 `Task.depends_on: Vec<TaskId>` — declared plan-time
      dependency, mirroring `scenario_refs`' shape and doc style (D7).
- [x] 2.3 `Handoff.from_role` / `to_role: Option<RoleId>`; the existing
      serialized-column-set and column/type/nullable assertions updated
      to match.
- [x] 2.4 `cargo xtask write` regenerates `schemas/*.schema.json` +
      `JOIN_SPINE.md`; `cargo xtask check-generated` clean. No
      `Envelope.schema` bump on any kind (D5).

## 3. canon-ingest — stop discarding execution lineage (D6)

- [x] 3.1 `UnifiedRow` + `DirectiveRow` gain `agent_id` /
      `parent_agent_id`; every construction site across all four
      adapters updated.
- [x] 3.2 Claude adapter: sidechain rows keep parent-session grouping
      but now carry their own `agent_id` and their parent's. The stale
      module doc claiming subagent identity is unresolvable is corrected.
      Depends on 3.1.
- [x] 3.3 omp/pi adapter: model `parentId` and thread it into the lineage
      fields IF it genuinely marks agent delegation; if it is only
      intra-session message threading, leave both fields `None` and say
      so in a code comment rather than faking an edge. Depends on 3.1.
- [x] 3.4 `normalize` emits one root `Run` per session plus one child
      `Run` per distinct `agent_id`, linked by `parent_run_id`.
      Deterministic ids preserved — ingest stays idempotent under the
      watermark cursor. Depends on 2.1 and 3.1.
- [x] 3.5 Regression guard: a plain single-agent session normalizes to
      exactly ONE `Run` with `parent_run_id: None`, byte-identical to
      pre-change behavior. Depends on 3.4.

## 4. canon-ingest — declared dependency import (D7)

- [x] 4.1 Plan dialects populate `Task.depends_on` from whatever the
      corpus genuinely expresses. Conservative + fail-soft: an
      unresolvable reference is dropped and counted as an import
      diagnostic, never an import failure. A dialect with no dependency
      expression populates nothing. Depends on 2.2.
- [x] 4.2 `Handoff` artifact adapter folds `from_role`/`to_role` into its
      emitted event detail when present, same conditional-insert style as
      `parent_handoff_id`. Its `NonVerdict` behavior is untouched —
      handoff is management plumbing and must never green or fail work.
      Depends on 2.3.

## 5. Verification

- [x] 5.1 `cargo test --workspace` green, including
      `canon_model::gen::tests::committed_generated_output_matches_current_source`
      (generated-output drift fails the suite directly).
- [x] 5.2 Dogfood the repaired flywheel against this repo: `canon ingest
      artifacts` reports a nonzero `trajectories_marked`, and `canon
      report`'s Role memory / Flywheel funnel panels stop rendering
      `_No rows._`. Depends on 1.1.
- [x] 5.3 `canon learn promote` on a real distilled strategy from this
      repo either promotes with named corroboration or refuses with a
      named reason — never silently succeeds on unresolved evidence.
      Depends on 1.2 and 5.2.

## Deliberately out of scope

The plan-vs-actual graph diff (D8) is the next increment. It needs both
`depends_on` and `parent_run_id` populated against real corpora before
its semantics can be specified from evidence rather than guessed. canon
also gains no scheduler, agent launcher, or DAG execution engine in this
change; `canon dispatch begin` stays a manifest-write seam.
