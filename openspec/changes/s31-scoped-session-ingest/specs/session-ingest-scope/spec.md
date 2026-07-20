## ADDED Requirements

### Requirement: File-granular watermark gate
A pass SHALL re-parse only files that are new or whose content digest
differs from the persisted cursor; byte-identical files SHALL be
skipped without being handed to parse, and the skip SHALL be surfaced
in the per-source summary.

#### Scenario: One growing transcript among thousands
- **GIVEN** a source whose cursor covers N files and exactly one file
  has grown since the last pass
- **WHEN** `canon ingest sessions` runs
- **THEN** exactly one file is parsed, N-1 are reported skipped, and
  the persisted records for the unchanged files are untouched

#### Scenario: Cursor advances only after a durable pass
- **WHEN** persistence degrades (needed rung unavailable)
- **THEN** no cursor advances, so every parsed file is re-parsed the
  next pass

### Requirement: Batched tier writes
`PgTier` SHALL persist ingest batches in chunked multi-row statements
whose per-row semantics are byte-identical to the single-row
append-only path (a byte-identical resubmission SHALL remain a no-op),
and tiers without an override SHALL fall back to the looped
single-write default.

#### Scenario: Re-ingest is still a no-op
- **GIVEN** a corpus already persisted
- **WHEN** the same corpus is force-re-ingested (`--full`)
- **THEN** the store's record count is unchanged

### Requirement: Project-scoped ingest by default
`canon ingest sessions` SHALL default to the current project's
sessions — the repo's main worktree root plus every linked
`git worktree` root — and SHALL prune cwd-partitioned scan roots
before reading; adapters whose transcripts carry no path partition
SHALL filter rows by workspace after parse. `--all-workspaces` SHALL
restore the machine-wide scan. Outside a git repo the project SHALL
fail-soft to the repo root alone.

#### Scenario: Worktree sessions count as the project
- **GIVEN** a session recorded in a linked worktree of this repo
- **WHEN** a default (project-scoped) pass runs
- **THEN** that session is ingested and its `Session` record carries
  the main worktree's `project_key`

#### Scenario: Foreign-project sessions are excluded
- **GIVEN** a session recorded in an unrelated directory
- **WHEN** a default pass runs
- **THEN** no records for it are written, and the summary names the
  active project scope

### Requirement: Session records carry workspace identity
Ingested `Session` records SHALL carry optional `workspace_key`,
`workspace_label`, and `project_key` fields resolved from the
transcript's own workspace context.

#### Scenario: Aggregation by project
- **WHEN** sessions from the main worktree and a linked worktree are
  ingested
- **THEN** both records share one `project_key`

### Requirement: User directives are captured as events
Adapters SHALL emit each USER-role message as a directive row, and
normalization SHALL persist it as an `Event` with
`label: "user_directive"` and the full text under `detail.text`,
with deterministic `seq` so re-parsing a grown transcript re-emits
byte-identical earlier events (digest-deduped). System, tool, and
assistant content SHALL NOT produce directive events.

#### Scenario: Directives queryable after ingest
- **GIVEN** a transcript containing two user messages
- **WHEN** the pass completes
- **THEN** `canon query --kind event` returns two `user_directive`
  events for that session, in transcript order, each carrying the
  verbatim message text
