## ADDED Requirements

### Requirement: `canon report` renders a config-derived boundary note for `pg`-routed record kinds
`canon report` SHALL compute, from `canon.yaml`'s STATIC `TierPolicy.routing` table alone (NEVER a live `pg` connection or row count), the set of `RecordKind`s routed to the `pg` tier. When that set is non-empty, `canon report` SHALL render a `## Tiers not reflected` section in the generated report, placed after `## Inputs (digest)` and before `## Trust matrix`, naming every such kind and pointing at `canon query --kind <kind>` as the read path for that data. When the set is empty (no `canon.yaml`, or a `canon.yaml` whose `routing` never sends any kind to `pg`), `canon report` SHALL render NO such section at all.

#### Scenario: A multi-tier repo renders the boundary note naming the pg-routed kinds
- **WHEN** `canon report` runs against a repo whose `canon.yaml` routes at least one `RecordKind` (e.g. `task`) to `pg`
- **THEN** the rendered report contains a `## Tiers not reflected` section naming `task` (and every other `pg`-routed kind), and pointing at `canon query --kind <kind>` as the way to read that data
- **AND** the section does NOT name any kind routed to `git` or `r2`

#### Scenario: A git-only repo renders no boundary section
- **WHEN** `canon report` runs against a repo with no `canon.yaml` present, or a `canon.yaml` whose `routing` table sends every kind to `git`/`r2` only
- **THEN** the rendered report contains no `## Tiers not reflected` section at all — not an empty section, not a placeholder, nothing

#### Scenario: A malformed canon.yaml degrades to the same no-note behavior as no canon.yaml
- **WHEN** `canon report` runs against a repo whose `canon.yaml` exists but fails to parse as a valid `TierPolicy` (e.g. an unknown `routing` key)
- **THEN** `canon report` still succeeds (does not fail or panic on this account) and renders no `## Tiers not reflected` section, exactly as it would for a repo with no `canon.yaml` at all

### Requirement: The boundary note is deterministic and drift-checkable, matching every existing panel's discipline
The `## Tiers not reflected` section's content SHALL be timestamp-free and SHALL list kinds in a fixed, sorted order (ascending by the kind's wire string) rather than any unordered iteration. Rendering `canon report` twice against an unchanged `canon.yaml` SHALL produce a byte-identical `## Tiers not reflected` section (or its byte-identical absence), regardless of whether a live `pg` connection is reachable at either run — `canon report --check`'s existing byte-diff drift gate SHALL cover this section with no separate mechanism.

#### Scenario: Two renders of an unchanged multi-tier config produce a byte-identical boundary section
- **WHEN** `canon report` runs twice in succession against a repo whose `canon.yaml` is unchanged between runs and routes at least one kind to `pg`
- **THEN** the `## Tiers not reflected` section of both renders is byte-for-byte identical, and neither render's section contains a wall-clock-derived value or a live `pg` row count
- **AND** this holds identically whether or not a live `CANON_PG_DSN` is set for either run

#### Scenario: `canon report --check` reports no drift solely from the boundary note's presence
- **GIVEN** a committed `canon/REPORT.md` was generated against the current, unchanged `canon.yaml`
- **WHEN** `canon report --check` is run afterward with no change to `canon.yaml`, the corpus, or the ledger
- **THEN** the check reports no drift — the boundary note's presence or absence never causes spurious drift on an otherwise-unchanged input

### Requirement: `canon report` emits a stderr warning naming the same pg-routed kinds as the rendered note
`canon report` (the CLI) SHALL print one stderr line, prefixed `canon report: WARN `, naming the identical (config-derived, sorted) set of `pg`-routed `RecordKind`s the rendered `## Tiers not reflected` section names, computed via the same derivation so the two can never disagree. This SHALL happen for every `canon report` invocation mode (the flagless write, `--check`, and `--snapshot <dir>`). A repo with no kinds routed to `pg` SHALL produce no such stderr line.

#### Scenario: A multi-tier repo's `canon report` run emits a stderr WARN matching the report's own note
- **WHEN** `canon report --repo <dir>` runs against a repo whose `canon.yaml` routes `task`, `session`, and `event` to `pg`
- **THEN** the command's stderr contains a line starting `canon report: WARN` that names `task`, `session`, and `event`
- **AND** the written `canon/REPORT.md`'s `## Tiers not reflected` section names the exact same three kinds

#### Scenario: A git-only repo's `canon report` run emits no WARN line
- **WHEN** `canon report --repo <dir>` runs against a repo with no `canon.yaml`, or one whose `routing` never touches `pg`
- **THEN** the command's stderr contains no `WARN` line naming any record kind

### Requirement: The boundary note and warning add no live `pg` dependency and change no gate/query behavior
Adding the `## Tiers not reflected` section and its stderr warning SHALL NOT introduce a live `pg` connection, a new `stg_pg_records` view, or any other live-`pg` read path into `canon-report` or `canon-store`'s DuckDB view layer. `canon query --kind <kind>` for a `pg`-routed kind SHALL be unaffected by this change. `canon gate check` SHALL continue to read nothing produced by `canon-report`, and its verdicts SHALL be byte-identical for any corpus, before and after this change.

#### Scenario: `canon query` for a pg-routed kind is unaffected
- **WHEN** `canon query --kind task` runs against a repo whose `canon.yaml` routes `task` to `pg`, both before and after this change lands
- **THEN** the command's behavior (rows returned, exit code) is identical in both cases — this change touches no `canon query` code path

#### Scenario: Gate verdicts are unaffected by the new boundary note's existence
- **WHEN** `canon gate check` runs against a corpus both before and after `canon report`'s `## Tiers not reflected` section is added
- **THEN** `canon gate check`'s verdicts are byte-identical in both cases — no `canon-gate` source file reads `canon-report`'s output or the boundary note
