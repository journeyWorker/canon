## ADDED Requirements

### Requirement: A superpowers plan document imports as one Change with its Goal as summary
The `superpowers` plan dialect SHALL map one plan document (an `*.md`
immediate child of the resolved plans root) to one `Change` whose
`change_id` is the slugified filename stem validated through
`ChangeId::parse`, whose `summary` is the `**Goal:**` line's
whitespace-normalized text, and whose `at` is the file's mtime. An
absent Goal line SHALL yield an empty summary plus a named
`goal-missing` diagnostic, never invented prose.

#### Scenario: A writing-plans-shaped doc becomes a Change keyed by its filename stem
- **WHEN** the adapter parses `2026-07-14-website-design.md`
  containing a `# Website Implementation Plan` H1 and
  `**Goal:** Build the project website.`
- **THEN** the outcome carries one `Change` with `change_id`
  `2026-07-14-website-design`, summary
  `Build the project website.`, and the actor
  `canon-plan-import-superpowers`

#### Scenario: A Goal-less plan imports with an empty summary and a named diagnostic
- **WHEN** the adapter parses a plan doc with task headings but no
  `**Goal:**` line
- **THEN** the `Change` imports with an empty summary
- **AND** `unmapped` carries one `goal-missing` count

### Requirement: Task sections import through the shared task_id derivation with checkbox-derived status
Each `### Task N: <name>` section SHALL map to one `Task` whose
`task_id` derives through the SAME shared
`openspec_rows::task_id_for` used by the openspec dialect and the S4
verdict adapter, whose title is the heading's `<name>` text, and
whose status is `Done` iff the section contains at least one checkbox
line and all its checkbox lines are checked. An invalid task number
SHALL be skipped and named malformed; a duplicate task number SHALL
keep the first section and name the later one malformed.

#### Scenario: Checked steps complete a task, unchecked steps keep it open
- **WHEN** a plan's `### Task 1: Adapter` section carries only
  `- [x]` checkbox lines and its `### Task 2: Docs` section carries a
  mix of `- [x]` and `- [ ]` lines
- **THEN** `2026-…#1` imports as `Done` and `2026-…#2` as open
- **AND** a task section with zero checkbox lines imports as open,
  never done

#### Scenario: The task join key is byte-identical to the S4 verdict layer's
- **WHEN** a superpowers plan `Task` and an S4 openspec-task verdict
  both derive a key for change `x` task `3`
- **THEN** both produce exactly `x#3` through
  `openspec_rows::task_id_for` — one derivation, no second grammar

### Requirement: Non-plan markdown degrades named, never imports garbage
A markdown file under the plans root SHALL be skipped, with a named
diagnostic, when it carries neither a `**Goal:**` line nor any
`### Task N:` heading — the skip uses the
`not-a-plan-doc` unmapped diagnostic; an absent or unreadable root
SHALL yield zero records without error; step lines and plan-header
prose SHALL never map onto records.

#### Scenario: A stray README in the plans dir is named, not imported
- **WHEN** the adapter parses a root containing one valid plan doc
  and one `README.md` with neither Goal nor task headings
- **THEN** exactly one `Change` imports
- **AND** `unmapped` carries one `not-a-plan-doc` count

### Requirement: The dialect registers through the one-entry seam and the CLI resolves it
`superpowers` SHALL be resolvable as `plans.sources[].dialect:
superpowers` in `canon.yaml` and as `canon ingest plans --dialect
superpowers --source <root>`, through the existing
`plan_registry::find` lookup — no driver, cursor, or persistence
changes.

#### Scenario: End-to-end CLI import of a fixture plan corpus
- **WHEN** `canon ingest plans --dialect superpowers --source
  <fixture-root>` runs against a `local`-routed `canon.yaml` and a
  fixture plan doc with one done and one open task
- **THEN** the command exits 0 and `canon query --kind change` /
  `--kind task` return the imported records with the derived
  statuses
