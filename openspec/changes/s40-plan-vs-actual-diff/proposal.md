# s40 — plan-vs-actual-diff

## Why

s37 added `Run.parent_run_id` and `Task.depends_on` and claimed, in its
own PR, that a plan-vs-actual graph diff was "now possible for the first
time". Auditing that claim against real data shows it is false, and that
two neighbouring surfaces are wrong in the same way — each states a
relationship its data does not actually carry.

**1. Nothing populates `Run.task_id`.** Across this repo's 5079 real
runs: `task_id` on **0**, `parent_run_id` on **6**, `injected_guidance`
on **0**. The two production `Run` writers are session ingest
(`normalize.rs`, which cannot know a task) and `canon dispatch begin`
(`dispatch::run_begin`, as of `14a03681`, which passed `None, None` to
`Run::new` for `session_id, task_id`). So the execution tree
exists and the plan DAG exists, and there is no edge between them. The
diff is not implementable as shipped; the claim was wrong.

**2. s37's own figure is inverted.** The PR says "5073 of this repo's
5079 real runs are children". It is the reverse: 5073 roots, 6 children.
A single-agent session yields exactly one run, which is the overwhelming
case, and that is the honest and expected shape.

**3. The flywheel funnel is not a funnel.** `mart_flywheel_funnel`
renders `Verdicts → distilled → retrieved → applied` and reports
`retrieved 0 / applied 16` for `test` on this repo. `retrieved` counts
strategies appearing in some `Run.injected_guidance`; `applied` counts
trajectories with a resolved outcome, computed with no reference to
retrieval at all. So `applied` can, and here does, exceed `retrieved` —
an impossible reading for a funnel. This is the same defect class as
s39's burn-down panel: a column name asserting a relationship the SQL
never computes.

**4. Session ingest has the plan cursor's bug.** s38 fixed
`canon ingest plans` skipping unchanged sources after a parser change by
folding `PlanAdapter::parse_version()` into the cursor id, and left the
session-side question open (s38 task 5.3). Answer: `SourceCursor` is
keyed on `client_id` plus per-file content digests only. A session
adapter's normalization can change and every unchanged transcript is
still reported `skipped unchanged (watermark)` — identical exposure,
unfixed.

## What Changes

**Bind runs to plan tasks.** `canon dispatch begin` gains `--task
<task_id>` and `--parent-run <run_id>`, populating the two fields that
were always `None`. `--task` is validated against the plan corpus, so a
typo fails loud instead of persisting a dangling binding. With both, one
dispatched run records which plan task it serves and which run dispatched
it — the missing edge.

**`canon dispatch diff`.** Compares the DECLARED plan DAG
(`Task.depends_on`) against the OBSERVED execution graph (`Run.task_id` +
`Run.parent_run_id`), reporting satisfied edges, declared-but-never-run
dependencies, and undeclared edges that executed anyway. Read-only; it
reports, it never gates.

**Make the funnel a funnel.** All three trailing stages count
STRATEGIES over one relation, so the chain narrows by construction:
`retrieved` is the distinct strategies injected into a run's context AND
still distilled today, and `applied` is that same set restricted to the
ones whose recipient run reached a terminal `Run.status`.

The "still distilled today" half is not a hedge. The view inner-joins
each recorded `StrategyRef` against the current strategy items, so a
strategy re-derived from CHANGED evidence takes a new content-derived id
and its earlier injection stops counting. s41 made that id a pure
function of the distilled content, so an unchanged strategy survives
every rebuild — but a genuinely changed one is a different strategy, and
the funnel says so rather than crediting the new text with the old
text's retrievals.

This is a PROXY, and the reason it is a proxy matters. The obvious
definition — a resolved trajectory joined to its own run — is not
available: the trajectory feeding this mart is canon-learn's parquet
row, which carries no `run_id`, and the `Trajectory` record kind that
does carry one has no production writer. So "applied" means guidance was
in context AND the run finished, not that the outcome was attributed to
the guidance. Attribution needs a trajectory that knows its run, and
that record does not exist yet.

**Version the session cursor.** A session adapter declares a parse
version folded into its cursor identity, exactly as s38 did for plan
dialects, so changing normalization re-reads instead of skipping.

**Fix the two shipped claims.** The inverted 5073 figure and the
"diff is now possible" claim are corrected wherever they were published.

## What This Change Deliberately Does NOT Do

- **`canon dispatch diff` never gates.** It is a read-only reporting
  surface and always exits `0` on a successful read, like `canon
  divergence status`. canon records topology; it still does not schedule
  or execute from it, and an undeclared edge is information, not a
  violation.
- **`--task`/`--parent-run` stay optional.** A dispatch with no plan task
  is still valid — the single-agent, no-plan case is the common one and
  must not require ceremony.
- **Does not reconcile dispatch manifests into the record store.**
  `canon dispatch begin` writes `<repo>/.canon/dispatch/<run_id>.json`
  and nothing ingests it, so a dispatched run is invisible to every
  tier-backed read. This was found while verifying this change, and it
  has two visible consequences. `canon dispatch diff` reads the manifest
  directory directly as a second source — without that it would report
  zero observed edges no matter how many runs were dispatched. And
  `mart_flywheel_funnel` reads `stg_records`, so it still reports
  `retrieved 0` on this repo even immediately after a dispatch that
  recorded a strategy into `injected_guidance`. The funnel's corrected
  SQL is proven on a synthetic corpus; its input on a real repo stays
  empty until a reconciliation step exists. Adding one is a design
  decision about where a live run lives and how it merges with the
  post-hoc run session ingest derives — including whether that is a
  git-tier duplicate-path write, which is a hard error today — and that
  belongs in its own change, not bolted onto this one.

## Impact

- `crates/canon-cli`: `main.rs`, `dispatch.rs`, `ingest.rs`.
- `crates/canon-ingest`: the session adapter trait and its four adapters.
- `crates/canon-store`: `cursor.rs`, `sql/views.sql`.
- `crates/canon-report`: funnel column docs.
- PR #1's body; `packages/website` only if a published page repeats
  either wrong claim.
- **No re-read on landing.** Version `1` IS the unsuffixed cursor id, so
  every cursor already on disk stays valid: it was produced by the
  normalization that is now version 1, and giving it a new id would
  assert a difference that does not exist. The suffix appears from `2`,
  the first version whose output can differ from what a stored cursor
  was computed against. Installing the mechanism therefore costs nobody
  a full transcript re-read; only a genuine normalization change does.
- **Behavior change, intentional:** `applied` in the flywheel funnel
  drops to `0` on this repo. That is the honest number — no
  `canon dispatch begin` has ever run here, so no `Run` carries
  `injected_guidance` and the retrieval loop has never closed.
