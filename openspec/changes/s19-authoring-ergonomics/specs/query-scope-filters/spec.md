## ADDED Requirements

### Requirement: --change-id and --status are gated to --kind change and --kind task
`canon query --change-id <id>` and `canon query --status <s>` SHALL
require `--kind change` or `--kind task`. Supplying either flag with any
other `--kind` value SHALL fail loud (exit `2`, naming the two supported
kinds) rather than silently returning the unfiltered result set for the
queried kind.

#### Scenario: --change-id with an unsupported kind fails loud
- **WHEN** `canon query --kind scenario --change-id wall.render` runs
- **THEN** the command fails with an error stating `--change-id` applies
  only to `--kind change`/`--kind task`, exits `2`, and prints no record
  data

#### Scenario: --status with an unsupported kind fails loud
- **WHEN** `canon query --kind session --status done` runs
- **THEN** the command fails with an error stating `--status` applies
  only to `--kind change`/`--kind task`, exits `2`, and prints no record
  data

### Requirement: --change-id filters Change records by identity and Task records by their owning change
`canon query --kind change --change-id <id>` SHALL return only the
`Change` record(s) whose `change_id` equals `<id>`. `canon query --kind
task --change-id <id>` SHALL return only `Task` record(s) whose
`task_id`'s owning change (`TaskId::change_id()`) equals `<id>` — the
identical `<change_id>#<n>` derivation `canon-ingest`'s plan/verdict
adapters already use, never a second parsing of the task id string.

#### Scenario: --change-id scopes a task query to one change's rows
- **WHEN** a ledger carries `Task` records for both
  `add-audio-reactive#1.1..2.2` and `add-widget#1.1`, and `canon query
  --kind task --change-id add-audio-reactive --json` runs
- **THEN** the returned records are exactly the `add-audio-reactive#*`
  tasks — `add-widget#1.1` is excluded

#### Scenario: --change-id scopes a change query to at most one record
- **WHEN** `canon query --kind change --change-id add-widget --json` runs
  against a ledger carrying multiple `Change` records
- **THEN** the returned records are exactly those (fold-latest history
  included) whose `change_id` equals `add-widget`

### Requirement: --status validates against the queried kind's own status domain
`canon query --status <s>` SHALL validate `<s>` against the STATUS DOMAIN
of the kind named by `--kind`: `open`/`done` for `--kind task`;
`proposed`/`in_progress`/`completed`/`archived` for `--kind change`. A
value outside the queried kind's domain (e.g. `--kind task --status
archived`) SHALL fail loud, exit `2`, naming the valid values for that
specific kind — never silently matched against the wrong kind's domain or
silently accepted as an always-empty filter.

#### Scenario: --status filters task records by open/done
- **WHEN** `canon query --kind task --status open --json` runs against a
  ledger with a mix of open and done tasks
- **THEN** the returned records are exactly those with `status: "open"`

#### Scenario: --status filters change records by their four-value domain
- **WHEN** `canon query --kind change --status in_progress --json` runs
- **THEN** the returned records are exactly the `Change` records whose
  `status` equals `"in_progress"`

#### Scenario: A status value outside the queried kind's domain fails loud
- **WHEN** `canon query --kind task --status archived` runs (`archived`
  is a `ChangeStatus` value, not a `TaskStatus` value)
- **THEN** the command fails with an error naming `task`'s valid status
  values (`open`, `done`), exits `2`, and prints no record data

### Requirement: --kind task carries a done/total rollup over the (possibly filtered) result set
A `canon query --kind task` invocation SHALL compute a `done`/`total`
rollup over its own result set (after any `--change-id`/`--status`
filtering has applied) — human output SHALL print a `<done>/<total> done`
summary line; `--json` output SHALL carry a `"rollup": {"done": <n>,
"total": <m>}` object. `--kind change` (and every other `--kind`) SHALL
NOT carry a rollup — `Change`'s own `status` field is the query's answer
for that kind.

#### Scenario: The rollup reflects the filtered result set, not the whole ledger
- **WHEN** `canon query --kind task --change-id add-audio-reactive --json`
  runs against a change with 6 tasks, 2 done, alongside other changes'
  tasks elsewhere in the ledger
- **THEN** the JSON `rollup` reads `{"done": 2, "total": 6}` — computed
  only over `add-audio-reactive`'s own tasks, not the whole ledger

#### Scenario: An unfiltered task query rolls up the whole result set
- **WHEN** `canon query --kind task --json` runs with no `--change-id`/
  `--status` filter, against a ledger carrying 10 tasks total, 4 done
- **THEN** the JSON `rollup` reads `{"done": 4, "total": 10}`

### Requirement: --kind change and --kind task output sorts deterministically by (change_id, task_id)
`canon query --kind change`/`--kind task` output SHALL be sorted
ascending by `change_id` (and, for `task`, secondarily by the task
number segments of `task_id`) before printing/emitting — reusing the
same natural-key resolution `format_human`'s existing per-row
`resolve_partition` call already derives, never a second parser. Every
OTHER `--kind`'s existing `at`-merge order (`TierRegistry::query`'s
native merge) SHALL remain byte-identical to its pre-change behavior.

#### Scenario: Task output sorts by change_id then task number, not merge order
- **WHEN** a ledger's raw merge order for `--kind task` interleaves rows
  from two changes (e.g. `add-widget#2.1`, `add-audio-reactive#1.2`,
  `add-widget#1.1`, `add-audio-reactive#1.1`), and `canon query --kind
  task --json` runs
- **THEN** the returned `records` array is ordered
  `add-audio-reactive#1.1`, `add-audio-reactive#1.2`, `add-widget#1.1`,
  `add-widget#2.1` — grouped by `change_id`, then by task number within
  each change

#### Scenario: A non-change/task kind's ordering is unchanged
- **WHEN** `canon query --kind trajectory --json` runs against a ledger
  whose records split across two tiers with differing `at` values (the
  exact fixture `tests/query.rs::merges_records_split_across_the_routed_
  tier_and_its_aging_destination` already exercises)
- **THEN** the returned order is byte-identical to this change's
  pre-existing `at`-merge behavior — the new sort applies to `change`/
  `task` only
