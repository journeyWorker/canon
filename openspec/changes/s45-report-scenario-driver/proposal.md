# s45 — report-scenario-driver

> The report half of `s44-spec-derived-worklist`, split out at s44's
> round 1 with a CORRECTED premise. s44's original diagnosis — that a
> plan dialect owns the empty panel — was falsified; the cause recorded
> below is the one this change acts on. The two changes share no source
> file and either may ship first.

## Why

Canon's own report renders the panel that answers "what is specced and
not done" as `_No rows._` (`.canon/REPORT.md:122-126`):

```
## Scope status

Task done × evidence-verified × spec-covered, per declared scenario ref (`mart_scope_status`).

_No rows._
```

The cause is NOT plan-dialect ownership. `canon.yaml` configures two
dialects (`canon.yaml:80` `- dialect: openspec`, `:86`
`- dialect: superpowers`) and this repo's `openspec/changes/` tree is
imported. The cause is a rung mismatch.

`mart_scope_status`'s row grain comes solely from
`int_task_scenario_refs` (`views.sql:933` `FROM int_task_scenario_refs r`
— the only non-`LEFT JOIN` relation in the view), which reads exactly
one kind (`views.sql:733`):

```sql
    WHERE kind = 'task'
```

and UNNESTs that task's `scenario_refs` (`views.sql:739`). But
`canon.yaml:56` routes `task: hot`, and `hot` is postgres
(`canon.yaml:28`), while the report layer is exhaustively git ∪ r2
(`views.sql:494-496`):

> This view is exhaustively `stg_git_records UNION ALL
> stg_r2_records`, no third source: Postgres has ZERO SQL view here

`.canon/ledger/kind=task` does not exist — the directory listing is
`change`, `divergence`, `evidence_record`, `finding`, `review`,
`scenario`, `subject`, and nothing else. The CLI says so itself:

```
$ ./target/release/canon query --kind task
canon query: hot tier (postgres) is not attached (no live DSN)
$ echo $?
1
```

and the generated report already lists `task` under **Kinds not read
directly** (`.canon/REPORT.md:22`). So the driver of the spec-coverage
panel is structurally invisible to the surface that renders it, for
every repo that has not stood up a postgres. That is canon's own
configuration, not a corner case.

**The same rung mismatch silently disarms the trust matrix.**
`mart_trust_matrix`'s subject inventory is a UNION
(`views.sql:838-842`):

```sql
subjects AS (
    SELECT task_id FROM tasks
    UNION
    SELECT task_id FROM evidence_counts
)
```

`tasks` reads `kind = 'task'` (`views.sql:819`); `evidence_counts`
aggregates `int_task_evidence`, which requires a non-NULL `task_id` on
an `evidence_record` (`views.sql:707`). Task is therefore the ONLY side
that can contribute a row with zero evidence. With no task rows the
inventory collapses to the evidence-keyed side, and every one of the 35
rows in canon's own trust matrix carries `evidence_count` 1
(`.canon/REPORT.md:31-65`, 35 rows, `awk`-counted: `35  1`). A matrix
whose purpose includes showing an uncovered cell cannot currently
produce one.

**`mart_subjects` is not the fix, and is closer than it looks.** It
ALREADY performs a Scenario left join — `subject_scenarios` UNNESTs
`Subject.scenario_ids` (`views.sql:1548-1553`) and LEFT JOINs
`scenario_latest_verdict` (`views.sql:1583`). But the join is
SUBJECT-driven: its `FROM` is `subjects` (`views.sql:1593`), so a
scenario linked to no Subject is invisible to it. That is the real,
narrow gap, and it is total in this corpus:

| Fact | Count |
| --- | --- |
| `Scenario` records (`canon query --kind scenario`) | 16 |
| …carrying a `subject_id` | **0** |
| distinct `project_id`s | 1 (`platformer`) |
| `mart_subjects` rows in `.canon/REPORT.md:134` | 1, `scenario_count 0` |

```
$ ./target/release/canon query --kind scenario --json \
    | jq '{count, with_subject_id: ([.records[] | select(.subject_id != null)] | length), projects: ([.records[].project_id] | unique)}'
{
  "count": 16,
  "with_subject_id": 0,
  "projects": [
    "platformer"
  ]
}
```

Sixteen scenarios are indexed, routed to a rung the report DOES read
(`canon.yaml:39` `scenario: local`; `canon.yaml:27`
`local: { backend: git, root: .canon/ledger }`), and appear in exactly
zero rows of exactly zero marts. **No SQL view reads
`kind = 'scenario'` today** — the complete set of kinds any view
selects is `divergence`, `event`, `evidence_record`, `finding`,
`handoff`, `porting.coverage`, `run`, `session`, `subject`, `task`.
`scenario` appears in `views.sql` only as a passed-through COLUMN
(`views.sql:557`, `:559`) and inside `WHERE kind = 'evidence_record'
AND (body ->> '$.scenario_id') IS NOT NULL` (`views.sql:1572`).

This change introduces the FIRST Scenario read in the mart layer.

## What Changes

**`mart_scope_status`'s row grain moves from Task to Scenario.** Its
driving relation becomes the `scenario` corpus at the composite
identity the model already declares (`records.rs:302-304`):

> `project_id` + `scenario_id` are the composite identity (design D6:
> `project_id` is REQUIRED, clean-cutover, no legacy `Option` branch)

Task is demoted from driver to a LEFT-JOINed annotation:
`task_id`/`task_status`/`evidence_covered`/`green` become nullable, and
a scenario that no plan declares — the residue s44's `## Why` names —
gets a row saying exactly that. On canon's own corpus the panel goes
from 0 rows to 16.

**A task-declared ref with no `Scenario` record still emits a row.**
The join is a FULL OUTER JOIN, not a scenario-only `FROM`. Today's row
set must not shrink: `crates/canon-report/tests/core_body_residual.rs:173-177`
asserts that a `kind=task` body the Rust reader REFUSES but DuckDB
globs still grows a `mart_scope_status` row —

> "`mart_scope_status` must grow the refused task's declared pair — the
> exception `manifest.rs` names beside its guarantee"

— and a scenario-only `FROM` would delete that pin's subject. A NULL
`project_id` on such a row reads as "no `Scenario` record backs this
declared ref", which is a corpus defect an operator should see, not one
the view should swallow.

**The overlay join tightens to its own declared key.** `cov` currently
joins on `scenario_id` ALONE (`views.sql:946`
`) cov ON cov.scenario_id = r.scenario_id`) even though the overlay
declares `join_key: [project_id, scenario_id]`
(`.canon/plugins/porting/plugin.yaml`) and the view's own header says
so (`views.sql:302-303`). The task side could not supply a project —
`Task.scenario_refs` is a bare list — so the view emitted one row per
covering project and a `spec_project_id` discriminator to say whose
answer each was (`views.sql:869-881`). A Scenario-driven grain supplies
the project, so the join becomes the full declared pair and the
ambiguity it worked around is gone. `spec_project_id` is removed: once
the join is the pair, it is either equal to `project_id` or NULL, and a
column that carries no information is worse than absent.

**`mart_trust_matrix` is NOT extended, and the reason is the primary
key.** It keys on `task_id` and derives `change_id` by splitting it
(`views.sql:845`):

```sql
    split_part(s.task_id, '#', 1)                  AS change_id,
```

A `Scenario` carries `project_id` and `scenario_id`
(`records.rs:320-343`) and no `task_id` and no `change_id`. UNIONing
scenarios into `subjects` would require either a synthetic `task_id`
(inventing a key the corpus does not hold, so `change_id` becomes a
`split_part` of a fabrication) or widening the mart's key into a tagged
union, which changes the meaning of every existing row and its pinned
column contract (`snapshot.rs:22`). Neither is worth doing to duplicate
a fact `mart_scope_status` will now state at the right grain. This is
recorded as a DECISION so it is not re-litigated: the trust matrix
stays the change/task view; the scenario inventory lives in
`mart_scope_status`.

**The column contract moves in lockstep at four sites.**
`mart_scope_status` gains `project_id` and loses `spec_project_id`.
Every site that pins the column list is enumerated in phase 2 and each
is test-pinned, so a site that lags is a test failure, never silent
drift.

**The panel prose stops being false.** `render.rs:416` currently reads
`"Task done × evidence-verified × spec-covered, per declared scenario ref (`mart_scope_status`).\n\n"`
— "per declared scenario ref" is exactly the grain this change
replaces. `packages/dashboard/test/panel-copy.test.ts:809-832` parses
that inline literal out of `pub fn render`'s body, and its own comment
names scope status as one of the three panels whose prose is an inline
literal "which is exactly where a paraphrase would otherwise be
unpinnable" (`panel-copy.test.ts:804-807`).

## Non-Goals

- **No new mart.** `mart_scope_status` already has the panel, the
  parquet, the manifest entry, and the report heading. A tenth table
  would move `SNAPSHOT_TABLES` (`snapshot.rs:40-50`), the "9 table(s)"
  assertion (`crates/canon-cli/tests/report.rs:100`), the nine-entry
  manifest assertion (`report.rs:123`), and the dashboard's own
  `TABLES` list (`build-fixture-snapshot.ts:32-42`) to buy a second
  view of one fact.
- **No dashboard panel component.** `packages/dashboard/src/panels/`
  holds seven files and none of them is scope status; the parquet is
  exported and no component renders it. Writing one is a design task
  with its own copy review, and this change does not depend on it.
- **No gate coupling.** `crates/canon-report/tests/gate_independence.rs:55-75`
  forbids any `canon-gate` source file from naming `mart_scope_status`.
  A mart cannot block a commit and this change does not pretend
  otherwise; the blocking half is s44's.
- **No record-kind change, no schema bump, no new join-spine key.**
  `Scenario` is read as it already exists.
- **No `@subject:` tagging of the `.feature` corpus.** That is s44
  phase 0. This change deliberately works on the corpus AS IT IS —
  0 of 16 scenarios carry a `subject_id` — because a panel that only
  works after a corpus edit is a panel that does not work.
- **No native Task authoring and no change to Task routing.**
  `task: hot` stays. This change makes the report correct WITHOUT a
  postgres, rather than requiring one.

## Risks

- **R1 — the Parquet column contract is asserted in four places that
  must move together.** `crates/canon-report/tests/snapshot.rs:39-42`
  pins `("mart_scope_status", &["task_id", "scenario_id",
  "task_status", "evidence_covered", "green", "spec_project_id",
  "spec_covered"])` and its module doc calls a writer/reader drift a
  failure that "must fail HERE, not silently diverge"
  (`snapshot.rs:11-12`). `crates/canon-report/src/marts.rs:161-162`
  declares the same list as `SCOPE_STATUS_COLUMNS`.
  `packages/dashboard/test/fixture-schema.ts:105-118` declares it again
  with types. `packages/dashboard/scripts/build-fixture-snapshot.sql:88-93`
  materializes the fixture with a positional `AS t(...)` alias. Four
  declarations of one list is the risk; phase 2 changes all four in one
  commit and phase 5 proves it by running the pins.
- **R2 — `order_by` is a total order and removing a column removes a
  term.** `marts.rs:165` pins
  `order_by: "task_id, scenario_id, spec_project_id"`, and
  `marts.rs:182-186` states why `spec_project_id` is in it: "with more
  than one row per pair, `(task_id, scenario_id)` alone is no longer a
  total order and the rendered table's row order would not be stable."
  Under the new grain `task_id` is nullable and can repeat, and
  `spec_project_id` is gone. The replacement order must be total or the
  markdown table's row order is nondeterministic and
  `canon report --check` becomes a coin flip. `int_task_scenario_refs`
  applies no `DISTINCT` (`views.sql:736-740`), so a plan listing one
  ref twice already produces two indistinguishable rows — a latent
  defect the new order surfaces, fixed in phase 1.
- **R3 — `manifest.rs`'s input inventory goes stale, and its accuracy
  has already been a recorded finding twice.** `manifest.rs:62-66`
  asserts:

  > A snapshot whose `mart_scope_status` rows moved cannot, over
  > VALIDATED core records and overlay inputs: that mart's two inputs
  > are `Task.scenario_refs` (a covered core kind) and
  > `porting.coverage` (a namespaced overlay), and both are digested.

  After this change the inputs are THREE, the added one being
  `scenario` — which `manifest.rs:50` already lists on the digested
  corpus side, so the GUARANTEE survives and only its enumeration goes
  wrong. That is precisely the failure mode already recorded twice in
  this repo's own ledger:
  `.canon/ledger/kind=finding/s43-findings-are-records__0004__0004__52355cc12e84.json`
  ("manifest.rs:61-64 says mart_scope_status cannot move under an
  unchanged source_digest, and lines 83-90 of the same file document
  the counterexample") and
  `.canon/ledger/kind=finding/s43-findings-are-records__0003__0001__7f0aa5e56cee.json`
  ("a corpus differing only in a porting.coverage row changes
  mart_scope_status.parquet under an unchanged source_digest"). A third
  round of the same finding is the outcome this risk exists to prevent,
  so phase 3 pins the enumeration with a test rather than a comment.
- **R4 — the trust-matrix key question must be DECIDED, not deferred.**
  `mart_trust_matrix` keys on `task_id` and derives `change_id` by
  `split_part(s.task_id, '#', 1)` (`views.sql:845`); `Scenario` carries
  `project_id`, not `change_id` (`records.rs:320-343`). Left
  unanswered, a later change adds scenarios to the UNION with a
  synthetic key and every `change_id` in the panel becomes a
  `split_part` of an invention. The decision above (not extended, with
  the reason) is the mitigation, and phase 3 records it where a future
  reader of the view will hit it.
- **R5 — the row set is not a pure superset, and the deltas must be
  enumerated rather than discovered.** Three of them: (a) 16 scenarios
  with no declaring task GAIN rows; (b) a scenario id covered by two
  spec roots LOSES its second row, because the overlay join is now the
  full declared pair instead of `scenario_id` alone (`views.sql:946`)
  — the ambiguity `views.sql:869-881` says the view "cannot pick one"
  about is resolved by the Scenario record's own project, not picked
  arbitrarily; (c) a task-declared ref with no `Scenario` record KEEPS
  its row with a NULL `project_id`, which is what protects
  `core_body_residual.rs:173-177`. Each is a phase-5 assertion.
- **R6 — `.canon/REPORT.md` gains rows, so `canon report --check` fails
  against the committed copy until it is regenerated.** The same
  intentional behavior change s43 carried. Noted because the report is
  generated-never-edited and drift-gated: the regeneration is part of
  this change, not a follow-up.
- **R7 — a NULL-heavy table is easy to misread.** With 0 of 16
  scenarios carrying a `subject_id` and 0 evidence records carrying a
  `scenario_id`, canon's own panel will render 16 rows whose task-side
  columns are all NULL. Read as "everything is broken" that is noise;
  read as "16 specced scenarios, none joined to a plan or an
  attestation" it is the change's entire point. The panel prose must
  say which, in the report itself, not only here.
