# canon-plan-import

> How to configure a canon.yaml `plans:` source (or `--dialect`/`--source`), run `canon ingest plans` to import a foreign planning dialect (openspec change dirs -- s17's reference dialect; superpowers `writing-plans`-shaped plan docs -- s30) into canon's Change/Task core records, and read them via `canon query --kind change`/`--kind task`. Explains the task_id join against S4's openspec-task verdict adapter (design R5: two readers, one join, shared key) that `mart_trust_matrix` already consumes, the plan_registry one-entry dialect-extension seam (D9/s30), and the still-deferred `donor-json` dialect. Use when adding a new plan dialect, debugging a `canon ingest plans` failure, or explaining why an imported row is indistinguishable from a hand-authored one.

# canon-plan-import

s17 (`s17-plan-import`) is the integration layer s15's own proposal named
as canon's one remaining pillar: "the integration layer (openspec /
superpowers / external-ledger IMPORT -- a secondary connector, like Jira
importing GitHub issues)". Before s17, `Change`/`Task` core records had
no bulk producer -- only `canon gate task` writing one hand-authored task
at a time. s17 normalizes a FOREIGN plan dialect's on-disk state into
ordinary `Change`/`Task` candidates and writes them through the SAME
validated tiered path every other ingest family uses. Imported rows carry
zero special marker beyond their fixed per-dialect actor
(`canon-plan-import-<dialect>`) -- anything that already reads
`Change`/`Task` (S9 marts, `canon query`) sees them exactly as it sees
native ones.

## The pipeline

```
canon.yaml `plans:` section (or --dialect/--source one-shot)
        │
        ▼
canon ingest plans [--repo <dir>] [--json]        # crates/canon-cli/src/plans.rs
        │ SourceCursor gate (content-digest, source-granular, skips an unchanged source wholesale)
        ▼
plan_registry::find(dialect) -> PlanAdapter        # crates/canon-ingest/src/plan_registry.rs
        │ .resolve_source(PlanSourceConfig) -> PlanSourceHandle
        │ .parse(handle) -> PlanParseOutcome { changes, tasks, unmapped, malformed }
        ▼
TierRegistry::persist (persist_idempotent, DuplicatePath-tolerant)
        │ unreachable tier -> `unwritten` seam (printed, non-fatal, cursor NOT advanced)
        ▼
canon query --kind change / --kind task           # crates/canon-cli/src/query.rs
        │ (imported rows byte-indistinguishable from hand-authored ones)
        ▼
mart_trust_matrix / a future consumer JOINs on task_id against S4's verdict events
```

## 1. Configure a `plans:` source

`canon.yaml`:

```yaml
plans:
  sources:
    - dialect: openspec
      root: .              # resolved relative to canon.yaml's own directory
```

- An ABSENT `plans:` section resolves to ZERO sources -- a clean,
  explicit no-op, never a hardcoded default root.
- A PRESENT section parses STRICTLY (`deny_unknown_fields`): a typo'd
  key, an unregistered `dialect`, or a nonexistent `root` all fail the
  command loud, naming the offender.
- `root` is whatever this dialect's adapter expects to scan -- for
  `openspec`, a repo root containing `openspec/changes/`, a changes dir
  passed directly, or a fixture tree holding just that substructure.

One-shot override (bypasses `canon.yaml` entirely for a single ad hoc
import; either flag alone fails loud):

```bash
canon ingest plans --dialect openspec --source ../other-repo
```

## 2. Run the import

```bash
canon ingest plans                 # canon.yaml-driven, every configured source
canon ingest plans --json          # machine-readable PlansOutcome
canon ingest plans --repo ../svc   # a specific repo root
```

Per source, the human/`--json` summary reports `changes_parsed`/
`tasks_parsed`, `changes_persisted`/`tasks_persisted`,
`changes_unwritten`/`tasks_unwritten` (an unreachable pg/r2 tier --
printed, never silently dropped), `duplicate_change_id` (two configured
sources producing the same `change_id` this pass -- the LATER one is
skipped and counted, never silently merged, design D8), the adapter's own
`unmapped` per-construct drop counts (design D3), and `malformed`
(structurally broken constructs). A source whose content digest is
byte-identical to its last successful cursor is `skipped_unchanged` --
re-importing an unchanged foreign plan writes ZERO new records.

## 3. Read the imported rows

```bash
canon query --kind change [--json]
canon query --kind task [--json]
```

No `--dialect`/`--plugin`-style filter exists or is needed -- an
imported `Change`/`Task` is an ordinary core record; `canon query`
cannot tell (and must not be able to tell) it apart from one
`canon gate task` wrote by hand. `canon-gate`/`canon-learn` carry ZERO
source reference to the plan family (design R1); `canon gate check`
verdicts are byte-identical with and without a prior `canon ingest
plans` run.

## The `openspec` dialect -- the worked example

`crates/canon-ingest/src/plan_adapters/openspec.rs` maps
`openspec/changes/**` onto core kinds:

| openspec construct | Core mapping |
| --- | --- |
| a change dir's basename | `Change.change_id` (`ChangeId::parse`, VERBATIM -- no slug massaging) |
| `proposal.md`'s `## Why` first paragraph | `Change.summary` (absent heading -> empty summary + a `proposal-missing-why` diagnostic) |
| archive location vs. checkbox tallies (design D6) | `Change.status` (`archived` wins unconditionally; else zero rows -> `proposed`, all done -> `completed`, mixed -> `in_progress`) |
| each `tasks.md` checkbox row | `Task` (`task_id = <change_id>#<n>`, status verbatim, evidence_note from the ` — ✅ ` suffix or a `**DEFERRED**`/`**DROPPED**` annotation) |
| `specs/**/spec.md` `#### Scenario:` blocks | dropped, counted under `spec-delta-scenario` -- NEVER a `Scenario` record |
| `design.md` | dropped, counted under `design-doc` |

A missing/unreadable `proposal.md`, or a basename failing
`ChangeId::parse`, skips the WHOLE dir (counted `malformed`), siblings
unaffected. A proposal-only dir (no `tasks.md` yet) imports as
`Change { status: proposed }` with zero tasks and zero diagnostics.

## The `superpowers` dialect -- the worked example

`crates/canon-ingest/src/plan_adapters/superpowers.rs` maps one plan
document (an `*.md` immediate child of the resolved plans root) onto
core kinds -- the grammar the superpowers `writing-plans` skill pins
as its format authority (s30 design D1: the exact deferral condition
s17 D9 left open -- "no format authority yet" -- now resolved by
citing that skill, not a speculative shape):

| `writing-plans` construct | Core mapping |
| --- | --- |
| the filename stem, slugified (lowercased, each `[^a-z0-9]+` run -> one `-`, edge `-` trimmed) | `Change.change_id` (`ChangeId::parse`; the date prefix is KEPT verbatim -- s30 D2, same "no date-prefix stripping" rule as the openspec dialect's D4) |
| the `**Goal:**` line's remainder, whitespace-normalized | `Change.summary` (absent Goal line -> empty summary + a named `goal-missing` diagnostic, never invented prose) |
| checkbox tallies across every `### Task N:` section (s30 D4 -- the openspec dialect's D6 tally rule minus its archive arm; superpowers has no archive convention) | `Change.status` (zero tasks -> `proposed`, all done -> `completed`, mixed -> `in_progress`) |
| each `### Task N: <name>` section | `Task` (`task_id = <change_id>#<N>` through the SAME shared `openspec_rows::task_id_for` the openspec dialect uses, `title` = the `<name>` text after the colon, status `done` iff the section has >=1 checkbox line and ALL are checked, else `open` -- a zero-checkbox section is open, never done) |
| steps, `**Architecture:**`/`**Tech Stack:**` header prose, Global Constraints, non-task headings | never read, never a diagnostic -- steps are EXPECTED to be unmapped (s30 D6) |

A worked example -- `2026-07-14-website-design.md`:

```markdown
# Website Implementation Plan

**Goal:** Build the project website.

### Task 1: Layout

- [x] **Step 1: scaffold the grid**
- [x] **Step 2: wire the nav**

### Task 2: Copy

- [x] **Step 1: draft the homepage**
- [ ] **Step 2: proofread**
```

imports as `Change { change_id: "2026-07-14-website-design", summary:
"Build the project website.", status: in_progress }` plus
`Task { task_id: "2026-07-14-website-design#1", title: "Layout",
status: done }` and `Task { task_id: "2026-07-14-website-design#2",
title: "Copy", status: open }`. The skill's `**Step N:**` bolding is
deliberately NOT load-bearing (D1) -- a plain `- [ ]`/`- [x]` row with
no bold prefix still counts.

An invalid task-number heading is skipped and named malformed; a
duplicate `Task N` heading keeps the first section and names the
later one malformed (s30 D3). A filename stem that fails to slug into
a valid `ChangeId` is malformed too. A markdown file under the plans
root with neither a `**Goal:**` line nor any `### Task N:` heading (a
stray `README.md`, say) is skipped with a named `not-a-plan-doc`
diagnostic rather than imported as garbage (s30 D5) -- the plans root
may be a repo root containing `docs/superpowers/plans/`, the plans
dir itself, or a flat fixture dir; only immediate-child `*.md` files
are read, byte-lexically sorted for deterministic enumeration; an
absent/unreadable root yields zero records, never an error.

## The task_id join against S4's verdict events (design R5)

`crates/canon-ingest/src/artifact_adapters/openspec_task.rs` (S4) reads
the exact SAME `openspec/changes/**` tree for a DIFFERENT job: it
normalizes each checkbox row into an `ArtifactEvent` keyed by
`ArtifactJoinKey::Task(task_id)`, which `crate::verdict::derive_verdict`
folds into a `{role, polarity, becomes}` verdict feeding canon's reward
flywheel (S6/S7). Both adapters derive `task_id` through the ONE shared
`crate::openspec_rows::task_id_for` function (design D5) -- so a plan
`Task.task_id` and a verdict event's `join_key` are BYTE-IDENTICAL for
the same row. This is the join, not a collision: the plan side emits a
`Task` candidate for EVERY row it reads (including untouched-open rows);
the verdict side only emits an event for a FLIPPED/annotated row (no
event can be derived from a single point-in-time snapshot of a plain
`- [ ]`) -- so the verdict adapter's emitted `task_id` set is always a
PROPER SUBSET of the plan adapter's.

canon's own S9 report already performs a `task_id`-keyed join across
core kinds: `mart_trust_matrix` (`crates/canon-store/sql/views.sql`)
LEFT JOINs `stg_records WHERE kind = 'task'` against
`int_task_evidence` (`EvidenceRecord` rows keyed by the SAME `task_id`)
for its `covered`/`green`/`who`/`evidence_count` columns. Before s17,
that mart's `task_id` side only ever saw hand-authored `canon gate task`
rows; `canon ingest plans` is now this mart's BULK producer for that
side, without changing one line of `mart_trust_matrix`'s own SQL --
proof that mapping onto core kinds is the point, not incidental.
s17 itself performs NO join, writes NO `EvidenceRecord`/verdict/
trajectory, and reads nothing outside the plan dialect's own source
tree -- joining plan state against evidence is entirely a downstream
reader's job (design R1, "connector never authority").

## The still-deferred dialect (design D9)

- **`donor-json` re-homing** (follow-up: `plan-dialect-donor-json`): the
  donor JSON canon already knows (ledger/divergence/handoff/task-state)
  is EVIDENCE, already ingested by S4 as verdicts -- s17 formally adopts
  those S4 adapters as this pillar's evidence-side members, zero new
  code. Deferred until a concrete donor PLAN corpus (not evidence)
  exists to target.

`superpowers` (s30) already proved the deferred-dialect claim in
practice: ONE new `plan_registry` entry plus one new
`crate::plan_adapters::superpowers` module, no change to `PlanAdapter`,
`PlanParseOutcome`, or the `canon ingest plans` driver. `donor-json`
lands the same way once its own deferral condition resolves. The
structural claim is also proven in-crate by a fixture SECOND dialect
adapter registered ONLY in
`crates/canon-ingest/tests/plan_fixture_dialect_seam.rs` (never in
production `plan_registry`): a trivial one-line-per-change text format
that resolves and parses through the exact same trait/outcome type
`openspec`/`superpowers` do.

## P1-P4 surface map

| Phase | What | Where |
| --- | --- | --- |
| P1 connector foundation | `PlanAdapter` trait, `PlanParseOutcome`, `plan_registry`; `openspec_rows.rs` shared checkbox grammar (extracted from S4's `openspec_task.rs`, zero behavior change) | `crates/canon-ingest/src/{plan_adapter,plan_registry,openspec_rows}.rs` |
| P2 openspec dialect | change-dir discovery, `Change`/`Task` mapping, D6 status derivation, D3 drop diagnostics | `crates/canon-ingest/src/plan_adapters/openspec.rs` |
| P3 CLI + persistence | `IngestCommand::Plans`, `canon.yaml` `plans:` parsing, per-source cursor gate, `unwritten` seam, cross-source collision (D8) | `crates/canon-cli/src/plans.rs` |
| P4 closure | this skill; the `plan-import` `canon selftest` suite; the fixture second-dialect seam proof | `canon/skills/canon-plan-import/SKILL.md`, `crates/canon-ingest/src/plan_selftest.rs`, `crates/canon-ingest/tests/plan_fixture_dialect_seam.rs` |
| s30 superpowers dialect | `writing-plans`-grammar discovery, `Change`/`Task` mapping (D2-D4), D6 unmapped diagnostics | `crates/canon-ingest/src/plan_adapters/superpowers.rs` |

## Selftest coverage (`canon selftest`, `plan-import` suite)

`crates/canon-ingest/src/plan_selftest.rs` registers the 11th `canon
selftest` suite: a SYNTHETIC openspec change tree (live/archive/
malformed/proposal-only dirs), built inside a `Drop`-cleaned scratch
directory at run time (never touches this repo's own
`openspec/changes/`), driven through the REAL `plan_registry`-resolved
`openspec` adapter -- the exact seam `canon ingest plans`'s driver uses.
A two-sided exact-set oracle (missing AND extra both fail) diffs the
emitted `Change`/`Task` ids + statuses, the named `unmapped` drop
counts, and the `malformed` scalar against a checked expectation.

Run it with `canon selftest` (all 11 suites) or `cargo test -p
canon-ingest plan_selftest`.

## What this skill does NOT cover

- **Authoring an openspec change dir itself** (`proposal.md`/
  `tasks.md`/`specs/**/spec.md` conventions) -- see the openspec CLI's
  own docs; this skill covers IMPORTING that shape into canon, not
  producing it.
- **`canon gate check`/`canon gate task`** (the hand-authored, one-task-
  at-a-time path `canon ingest plans` complements at bulk) -- see the
  `trust-spine-gate` skill.
- **S4's openspec-task verdict adapter's own evidence classification**
  (PR-merge/CI-fail mapping table) -- see
  `crates/canon-ingest/src/artifact_adapters/openspec_task.rs`'s own
  module doc; this skill only covers the shared `task_id` it derives,
  never re-derives its verdict logic.
- **`canon report`/`mart_trust_matrix`'s own SQL** -- see
  `crates/canon-store/sql/views.sql`; this skill only points at the
  `task_id`-keyed join it already performs, never re-implements it.
- **Wiring an imported `Task`/`Change` into a `canon gate` DECISION.**
  Explicit non-goal (design R1) -- needs its own reviewed change.
