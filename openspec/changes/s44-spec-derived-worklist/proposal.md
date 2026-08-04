# s44 — spec-derived-worklist

> Round 1 review: 14 findings, 8 blockers
> (`.canon/ledger/kind=finding/s44-spec-derived-worklist__0001__*`). This
> revision answers all of them; the original scope is split, and the
> report half moves to its own change (see "Split" below).

## Why

Canon is a spec-driven ledger. An unimplemented or mismatched spec IS the
work item. There is one class of spec canon can see neither way.

`GateContext::load` reads exactly one record kind —
`TierQuery::kind(RecordKind::EvidenceRecord)` (`context.rs:145`, the sole
tier read in `fn load`, `context.rs:141-153`). Coverage then groups THAT
set by join key (`coverage.rs:117-122`) and asks, per group, whether every
policy-required role is present. The cause is named in the module's own
words (`coverage.rs:47-49`):

> there is no external Task/Scenario registry in the frozen
> `GateContext` shape to discover an artifact with LITERALLY zero
> submitted evidence

The same doc names a mitigation, and it is real but partial
(`coverage.rs:49-53`): a task with no evidence is
`gated-task-completion`'s territory, caught as `unevidenced-flip` when
its checkbox is flipped. So the TASK half is covered.

The scenario half is covered too — for scenarios that belong to a
Subject. `mart_subjects` (`views.sql:1536-1593`) already performs exactly
this left join: it UNNESTs `Subject.scenario_ids` and LEFT JOINs the
latest verdict, yielding `scenario_count`/`covered_scenarios`, and it
renders a real row in canon's own report today
(`.canon/REPORT.md:128-134`). `subject.rs:425-447` enforces the same join
at the `verifying → shipped` transition.

**The residue is a `.feature` scenario that belongs to no Subject and has
no task.** No check reaches it. No mart counts it. It is specced, indexed
by `canon inventory sync`, and invisible to every surface canon has.

That residue is not a corner case in this repo — it is the whole corpus.
Measured against `.canon/ledger` at authoring time:

| Fact | Count |
| --- | --- |
| `Scenario` records indexed | 16 |
| …carrying a `subject_id` | **0** |
| `EvidenceRecord`s | 35 |
| …carrying a `scenario_id` | **0** (all are `task_id`-keyed) |
| `@subject:` tags in the `.feature` corpus | **0** |

So `mart_subjects` reports `scenario_count 0` for its one subject, and
all 16 scenarios sit in the residue. Canon's spec side and its evidence
side are joined in the schema and disjoint in practice.

`Scenario` is the right inventory to fix this from, and the model already
says so. It carries no coverage field, and the source states why
(`records.rs:311-313`):

> `covered`/`surface_ref` are deliberately NOT core fields (P1 shipped
> them, P3a removed them). Coverage stays `canon-gate`'s own
> `uncovered-cell` authority.

A shipped field was deliberately removed to keep coverage derived. The
design intended derivation; it simply anchored the derivation to the
evidence side, which by construction cannot enumerate what has no
evidence.

## What Changes

**The join is made real before anything reads it (phase 0).** The gate
work is inert until a scenario can be joined to evidence at all. Phase 0
tags the `.feature` corpus with `@subject:`, attributes scenario-keyed
evidence, and establishes the measurement that later phases accept
against. Without it there is no corpus on which the check's correct
behavior is demonstrable — only an all-red or wholly inert result.

**The gate learns to start from the spec corpus.** `GateContext` gains
`scenarios`, `divergences`, and `subjects`. A new `SpecCoverageCheck`
left-joins the Scenario corpus against them on the composite
`(project_id, scenario_id)` identity the model requires
(`records.rs:302-304`): a scenario with no evidence is unimplemented
work; a scenario whose folded divergence is `Open`, `StillDivergent`, or
`ResolvedInvalid`, or whose latest verdict is `Divergent`, is mismatched
work. Both surface as `uncovered-cell` with a distinguishing detail —
the precedent `subject.rs:437` already sets — so the closed eight-member
`FailureClass` set (`failure_class.rs:88-97`) is not extended.

**Malformed rows from the new kinds do NOT widen the existing violation
surface.** `LedgerCheck` maps every `ctx.violations` entry to
`MalformedEvidence` (`ledger.rs:133-138`) and is unconditionally in
`check_set` (`dispatch.rs:49-53`). Folding three new reads into that
field would turn a green repo red on upgrade with no policy change — the
exact opposite of this change's safety claim. The new kinds' read
violations therefore land in a separate field only `SpecCoverageCheck`
consults.

**The new check is silent unless a repo opts in.** `.canon/policy.yaml`
gains an optional `spec_coverage` section; absent resolves to `None` and
derives zero violations, mirroring the empty-`risk_routing` case pinned
at `coverage.rs:228-235`. Loudness for a malformed section lands at CHECK
time, not resolve time: `PolicyResolution::resolve` is documented frozen
infallible (`policy.rs:42-43`) and this change does not touch its
signature.

**Task is unchanged by this change.** The report-side demotion moves to
its own change (below), so `Task.scenario_refs`, `Task.depends_on`, the
marts, and the closed 14-kind set are all untouched here.

## Split

Round 1 established that the report half shares no source file with the
gate half — the gate touches `crates/canon-gate/src/*` and
`crates/canon-cli/src/context.rs`; the report touches
`crates/canon-store/sql/views.sql` and `crates/canon-report/src/*`. It
also carries its own blast radius the original scope never named: the
Parquet column contract asserted at `crates/canon-report/tests/snapshot.rs:22`,
the `order_by` pinned at `marts.rs:53`, the dashboard consumers, and
`manifest.rs:61-79`'s input inventory, whose accuracy has already been a
recorded finding twice.

It also rests on a diagnosis round 1 falsified. `mart_scope_status`
renders `_No rows._` NOT because a plan dialect owns it — canon.yaml
configures two dialects — but because `task: hot` (`canon.yaml:56`)
routes Task to a rung the report cannot read: "Postgres has ZERO SQL view
here" (`views.sql:494-496`). `.canon/ledger/kind=task` does not exist.
Task's inventory is structurally invisible whenever `hot` is
unconfigured, and `scenario: local` is a rung the report DOES read —
which is a better argument for the Scenario driver than the one
originally made, and belongs in the change that acts on it.

**`s45-report-scenario-driver` owns that work.** The two are independent;
either may ship first.

## Non-Goals

- **No anti-join in `canon query`.** The module doc states the boundary:
  "no cross-tier JOIN happens here or in the library it calls"
  (`query.rs:3-5`). The gate answers "what is left"; widening the query
  language is a larger change than the one it would serve.
- **No "do it entirely in canon-report" alternative.** A mart cannot
  block a commit, and `crates/canon-report/tests/gate_independence.rs:59-64`
  structurally forbids the gate from reading report output. The two
  surfaces are complementary, not substitutable.
- **No native Task authoring.** No `canon task new`; Task stays
  import-only.
- **No record-kind removal, no schema-version bump.**
- **No scheduler.** s37's D7 non-goal is preserved untouched.

## Risks

- **R1 — the join key is unpopulated, so the check has no demonstrable
  correct behavior.** 0/35 evidence records carry `scenario_id`; 0/16
  scenarios carry `subject_id`. Without phase 0 the check emits
  `uncovered-cell` for 16 of 16 scenarios (zero signal), and any
  configured `scope` disables it entirely, because a scenario with no
  `subject_id` is out of scope. This is the change's highest risk and
  phase 0 exists solely to retire it before phase 3 is accepted.
- **R2 — violation-surface widening.** Reading three more kinds runs
  `GitTier`'s layout validation over three more trees, so a pre-existing
  layout defect becomes a new violation. Mitigated by the separate
  violation field above; canon's own corpus reports zero violations for
  all three kinds today, but that is luck, not design, and the mitigation
  must not rest on it.
- **R3 — tier-routing assumption.** `GateContext::load` hardcodes
  `GitTier` (`context.rs:144`), safe for evidence only by convention,
  while `routing` is per-repo configurable (`canon.yaml:31-63`). A
  consumer routing `subject`/`scenario`/`divergence` off `local` would
  get a silently empty corpus and a check that fails OPEN. This is the
  same defect class that made Task invisible to the report. Phase 1
  refuses to enable `spec_coverage` when routing sends any of the three
  kinds off the rung the gate reads, rather than degrading silently.
- **R4 — divergence fold has an input the gate cannot derive.**
  `fold_to_current_state` (`fold.rs:82-86`) requires a `live_bindings`
  map whose only derivation is private to `canon-report`
  (`divergence.rs:65-89`), a crate canon-gate may not depend on
  (`gate_independence.rs:59-64`). Reimplementing it in canon-gate would
  be the second derivation this risk exists to forbid. Phase 3 promotes
  the derivation into `canon-model` so both callers share one.
- **R5 — three more full-tier scans land on interactive commands.**
  `GateContext::load` is called by `canon gate task` (`gate.rs:267`) and
  by `canon subject status` (`subject.rs:429`), not only by
  `canon gate check`. The reads are unconditional so that a policy change
  alone tightens the gate with zero corpus edits — the property
  `risk_routing` documents at `coverage.rs:29-35` — and that property is
  judged worth the scan cost. Measured, not assumed: phase 4 records the
  before/after wall time of `canon gate check` on canon's own corpus.
- **R6 — selftest fixture bijection.** `selftest.rs:425` asserts one
  fixture per `FAILURE_CLASSES` entry. Adding fixtures for the new check's
  cases while reusing `UncoveredCell` breaks that assertion. Phase 3
  places the new cases in the check's own unit tests rather than the
  selftest corpus, leaving the invariant intact.
