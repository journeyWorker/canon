## ADDED Requirements

### Requirement: `canon report` renders a scope-status panel sourced from `mart_scope_status`
`canon report` SHALL render a `## Scope status` panel populated from `crates/canon-store/sql/views.sql`'s `mart_scope_status` view, showing one row per declared `(task_id, scenario_id)` pair with `task_status` (done — the plan's own checkbox), `evidence_covered`/`green` (verified — an `evidence_record` exists / its latest verdict is `faithful`), and `spec_covered` (scenario-authored — a `porting.coverage` overlay row exists and is `covered`). This SHALL be the same "DONE and VERIFIED in one answer" the join layer (s20) already computes in SQL, made reachable from the one blessed CLI surface a planner or developer already runs, on a git-only repo, with no r2 or learn tier configured.

#### Scenario: A git-only repo with a declared scenario ref renders its scope-status row
- **WHEN** `canon report` runs against a git-only repo whose ledger contains a `Task` declaring a `scenario_refs` entry, at least one `evidence_record` for that task, and a `porting.coverage` overlay record for the referenced scenario
- **THEN** the rendered report contains a `## Scope status` section, and its table includes a row for that `(task_id, scenario_id)` pair showing the task's `task_status`, whether evidence covers it (`evidence_covered`/`green`), and whether the scenario is spec-covered (`spec_covered`) — sourced from `mart_scope_status`, not re-derived or re-aggregated in Rust

#### Scenario: A task with no declared scenario refs contributes no scope-status row
- **WHEN** `canon report` runs against a ledger where every `Task` has an empty (or absent) `scenario_refs` — the pre-s20-authoring majority case
- **THEN** the `## Scope status` panel renders its documented empty-panel placeholder, exactly like any other panel with zero matching rows, never a missing section, an error, or a crash

#### Scenario: A repo with no `porting.coverage` overlay still renders the panel with an honest NULL, never a fabricated value
- **WHEN** a `Task` declares a `scenario_refs` entry but the repo has no `porting.coverage` overlay record for that scenario at all
- **THEN** the rendered row for that `(task_id, scenario_id)` pair shows `spec_covered` as an honest missing/NULL value (the same NULL-cell rendering every other panel already uses for a similarly absent value), never `false` and never a dropped row

### Requirement: `canon report --snapshot` exports `mart_scope_status`
`canon report --snapshot <dir>` SHALL export `mart_scope_status` to `<dir>/mart_scope_status.parquet` and list it in the written `manifest.json`'s `tables` array, using the same generic `COPY "<view>" TO '<path>' (FORMAT parquet)` mechanism every other exported mart already uses — no view-specific export code path.

#### Scenario: A snapshot directory contains the scope-status parquet file and its manifest entry
- **WHEN** `canon report --snapshot <dir>` runs against a repo with at least one declared `(task_id, scenario_id)` scope-status row
- **THEN** `<dir>/mart_scope_status.parquet` exists on disk, and `<dir>/manifest.json`'s `tables` array contains an entry with `table: "mart_scope_status"` and `file: "mart_scope_status.parquet"`

#### Scenario: The snapshot succeeds on a git-only repo with zero scope-status rows
- **WHEN** `canon report --snapshot <dir>` runs against a fresh, git-only repo with no `Task.scenario_refs` declared anywhere
- **THEN** the snapshot still succeeds, still writes `mart_scope_status.parquet` (a valid zero-row parquet file) and its manifest entry — never a failure merely because the view currently has no matching rows

### Requirement: The scope-status panel is deterministic and drift-checkable, matching every existing panel's discipline
The `## Scope status` panel's rendered content SHALL be timestamp-free (no wall-clock-derived value anywhere in its rendered rows) and SHALL render rows in a fixed, sorted order (`task_id`, then `scenario_id`) rather than any unordered iteration. Rendering `canon report` twice against an unchanged corpus SHALL produce byte-identical output for this panel, exactly as every other panel already guarantees — `canon report --check`'s existing byte-diff drift gate SHALL cover this panel with no separate mechanism.

#### Scenario: Two renders of an unchanged corpus produce a byte-identical scope-status panel
- **WHEN** `canon report` runs twice in succession against the identical, unchanged ledger corpus
- **THEN** the `## Scope status` section of both renders is byte-for-byte identical, and neither render's `## Scope status` section contains a `generated_at` or any other wall-clock-derived value

#### Scenario: `canon report --check` detects scope-status drift the same way it detects any other panel's drift
- **WHEN** a checked-in `canon/REPORT.md` was generated before a new `(task_id, scenario_id)` scope-status row was introduced into the ledger, and `canon report --check` is run afterward
- **THEN** the check reports drift (a non-clean outcome), exactly as it already would for a change to any of the existing five panels — no separate drift-detection code path exists for this panel

### Requirement: The scope-status panel is read-only reporting and never a `canon gate` input
Adding the `## Scope status` panel and its `SNAPSHOT_TABLES` entry SHALL NOT change `canon gate check`'s verdicts for any corpus, before or after this change lands. `canon-gate` SHALL continue to read nothing produced by `canon-report` — the scope-status panel is consumed only by a human or downstream dashboard reading `canon report`'s output, never by the gate authority, preserving connector-never-authority.

#### Scenario: Gate verdicts are unaffected by the new panel's existence
- **WHEN** `canon gate check` runs against a corpus both before and after `canon report`'s scope-status panel is added
- **THEN** `canon gate check`'s verdicts are byte-identical in both cases — no `canon-gate` source file reads `canon-report`'s output or `mart_scope_status` directly
