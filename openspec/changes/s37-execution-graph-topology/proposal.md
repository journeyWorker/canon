# s37 — execution-graph-topology

## Why

Two independent findings, one root cause.

**1. The reward flywheel has two severed links.** S6/S7 built the whole
trajectory → verdict → strategy → retrieval loop and left two joints
unwired, each in `canon-cli`:

- `canon ingest artifacts` constructs every `Trajectory` via
  `Trajectory::new`, which by design starts at
  `VerdictOutcome::Pending`, and never calls `mark_trajectory_verdict`.
  S7's promotion gates read `verdict_record.outcome`, **not** the raw
  `verdicts` list — so every artifact-derived trajectory was invisible
  to them, permanently. `RewardRegistry::compute_for_trajectory` exists
  and its own doc names itself "the shape `mark_trajectory_verdict`'s
  caller typically wants"; nothing called it.
- `canon learn promote` never calls `PromotionGate::evaluate`. S6's own
  design says "[Mitigation] S7's statistical-promotion gate is the
  primary enforcement point; this change's `canon learn promote` adds a
  content-length + literal-path-pattern lint as defense-in-depth
  (documented as advisory)" — but S6 shipped before those gates existed
  and nothing came back. `canon.yaml`'s `learn.promotion.<role>` block
  is parsed AND validated (it rejects `n_min: 0` specifically because
  that "would defeat the n-occurrence gate entirely") and then read by
  no write path at all.

The visible symptom: `canon report`'s **Role memory** and **Flywheel
funnel** panels render `_No rows._` — not for want of data, but because
the pipeline that produces those rows is cut in two places.

**2. canon cannot represent an execution graph, and actively destroys
the one it already receives.** canon is a substrate for agents that
increasingly run as graphs of delegating subagents, yet:

- `Task` has no `depends_on` — the declared plan-time dependency
  structure that `tasks.md` already expresses in prose is discarded at
  import.
- `Run` has no `parent_run_id`. A multi-agent call tree cannot be
  reconstructed from any typed field.
- `Handoff` carries `parent_handoff_id`, `chain_id`, and `seq` — a
  directed edge with **no endpoints**. `claimed_by` names a claimant and
  `Envelope.actor` names the record's producer; neither is a
  source/target role pair.
- Worst: `canon-ingest`'s Claude adapter collapses every sidechain into
  its parent session and *discards subagent identity outright*, with a
  module doc admitting it ("excludes subagent display-name resolution
  because `UnifiedRow` has no agent field"). The omp/pi adapter ignores
  `parentId`. canon reads transcripts from four agent CLIs — it already
  holds the only ground-truth execution graph anyone has — and flattens
  it at the door.

## What Changes

- **Flywheel repair (`canon-cli` only, no model change):**
  - `canon ingest artifacts` resolves each freshly persisted
    trajectory's covering verdict via
    `RewardRegistry::compute_for_trajectory` and writes it back with
    `mark_trajectory_verdict`, before `rebuild_namespace`. A role whose
    reward function legitimately resolves to `Pending` (the `dev`
    triad below a full `1.0`, awaiting a no-rollback signal) is counted
    and left pending, never forced.
  - `canon learn promote` evaluates the per-role
    `PromotionMode` gate from `canon.yaml` over the strategy's own
    regime trajectories, and **fails closed** with no `--force`. The
    gates stay pure; `canon-cli` resolves the samples, exactly as
    `canon_learn::webhook`'s doc assigns the mirror task.
- **Additive lineage fields** (every one `Option<T>`/`Vec<T>` with
  `#[serde(default)]`, so no `Envelope.schema` bump):
  - `Run.parent_run_id: Option<RunId>` — the run that dispatched this
    run. `None` = root.
  - `Task.depends_on: Vec<TaskId>` — DECLARED plan-time dependency.
    canon never schedules or executes from it.
  - `Handoff.from_role` / `Handoff.to_role: Option<RoleId>` — the edge
    gains endpoints.
  - `UnifiedRow`/`DirectiveRow` gain `agent_id` / `parent_agent_id`.
- **Stop discarding subagent identity.** Claude sidechain rows keep
  their parent-session grouping (other code depends on it) but now also
  carry their own `agent_id` and their parent's. `normalize` emits one
  root `Run` per session plus one child `Run` per distinct agent, linked
  by `parent_run_id`. A single-agent session must normalize to exactly
  one `Run` with `parent_run_id: None` — byte-identical to today.

## What This Change Deliberately Does NOT Do

canon does not become an orchestrator. It gains no scheduler, no agent
launcher, no DAG execution engine, and `canon dispatch begin` stays a
manifest-write seam. The orchestrator owns topology; canon's
differentiated position is the grounded-evidence layer beneath whatever
orchestrator runs — and that position is worth strictly more once canon
can record what the orchestrator actually did.

## Impact

- `crates/canon-cli`: `artifact_ingest.rs`, `learn.rs`,
  `tests/learn_promote.rs`.
- `crates/canon-model`: `records.rs` (`Run`, `Task`), `handoff.rs`;
  regenerated `schemas/*.schema.json`.
- `crates/canon-ingest`: `adapter.rs`, `normalize.rs`,
  `adapters/{claude,omp,codex,hermes}.rs`, `plan_adapters/*`,
  `artifact_adapters/handoff.rs`.
- **Behavior change, intentional:** `canon learn promote` now refuses an
  uncorroborated strategy. The prior integration fixture asserted
  ungated success; it seeds corroborating trajectories now, and the
  blocked cases are covered as first-class contracts.
