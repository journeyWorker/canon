## ADDED Requirements

### Requirement: Canon-owned openspec checkbox grammar
The system SHALL own the `openspec/changes/<slug>/tasks.md` task/checkbox
grammar as its own parser and writer — `- [ ] ` / `- [x] ` rows, the
`**DEFERRED to §<to>**` / `**DROPPED**` annotation forms, and the
` — ✅ <evidence>` suffix — without depending on or delegating to any other
tool's parser.

#### Scenario: Round-tripping every recognized row shape
- **WHEN** canon parses a `tasks.md` file containing an open checkbox, a
  flipped checkbox with an evidence suffix, a `DEFERRED` row, and a
  `DROPPED` row
- **THEN** canon reconstructs each row byte-identically when re-serialized
  without modification

### Requirement: Evidence-gated task flip
The system SHALL flip a task row from `- [ ] ` to `- [x] ` only when a
matching `EvidenceRecord` exists for that `task_id` and passes the
fabrication-marker scan. A missing or malformed evidence record SHALL
fail-closed: the row stays unflipped and the command exits non-zero.

#### Scenario: Flip succeeds with clean evidence
- **WHEN** `canon gate task <task_id>` runs and a passing, non-fabricated
  `EvidenceRecord` exists for `<task_id>`
- **THEN** the task row flips to `- [x] ` with an appended evidence note,
  and the command exits zero

#### Scenario: Flip is blocked with no evidence record
- **WHEN** `canon gate task <task_id>` runs and no `EvidenceRecord` exists
  for `<task_id>`
- **THEN** the task row is not modified and the command exits non-zero with
  an `unevidenced-flip` violation

#### Scenario: Flip is blocked on malformed evidence
- **WHEN** an `EvidenceRecord` exists for `<task_id>` but fails schema
  validation
- **THEN** the flip is refused — malformed evidence is treated as no
  evidence, never as a passing evidence record

### Requirement: Fabrication-marker scanning
The system SHALL scan only structured evidence fields (never free
conversational prose) for fabrication markers before permitting a task
flip, and SHALL treat a bare `verified` token with no attached command
result as a fabrication marker.

#### Scenario: A fabricated evidence field blocks the flip
- **WHEN** a structured evidence field's text contains a fabrication marker
  (e.g. "would pass", "TBD", "n/a")
- **THEN** `canon gate task` reports a `fabricated-evidence` violation and
  the task row is not flipped

#### Scenario: A bare verified token with no command result blocks the flip
- **WHEN** a structured evidence field's text is `verified` with no
  attached captured command result
- **THEN** the fabrication scan flags it and the flip is refused

#### Scenario: Free prose containing a blocklist word is not scanned
- **WHEN** an agent's free-form reply text contains a fabrication-marker
  substring, but the structured evidence fields passed to the scanner
  contain none
- **THEN** the scan passes cleanly — free prose is never handed to the
  fabrication scanner

### Requirement: canon gate task CLI command
The system SHALL provide `canon gate task <task_id>` as the single entry
point for evidence-gated task flips, resolving `<task_id>` through the S1
join spine (`<change_id>#<n>`).

#### Scenario: Unknown task_id is reported, not silently ignored
- **WHEN** `canon gate task <task_id>` is invoked with a `<task_id>` that
  matches no row in the resolved change's `tasks.md`
- **THEN** the command exits non-zero and reports that the task was not
  found

### Requirement: Hook-seam wiring generation
The system SHALL generate hook-seam entries invoking `canon gate task` in
the consumer repo's `.claude/settings.json` and `.codex/hooks.json` using
the same `{matcher, hooks: [{type: "command", command, timeout}]}` shape
existing hook entries use, and SHALL ship a generic pre-commit script for
repos without that hook system. Generation SHALL be idempotent.

#### Scenario: Installing hooks is idempotent
- **WHEN** `canon gate install-hooks` runs twice in a row against the same
  repo with no manual edits in between
- **THEN** the second run reports no diff and writes nothing

#### Scenario: A non-donor-CLI repo gets a generic pre-commit script
- **WHEN** `canon gate install-hooks` runs in a repo with no
  `.claude/settings.json` hook entries for `canon gate`
- **THEN** a generic pre-commit script invoking `canon gate` is emitted

### Requirement: donor-CLI migration-target boundary
The system SHALL document, without executing, the migration path for
the donor CLI's evidence-gated flip (`flipTaskDone` + `scanFakeMarkers`) to delegate
to `canon gate task`. This change SHALL NOT modify any file in the donor CLI's
own package tree.

#### Scenario: The donor CLI's existing behavior is unchanged by this change
- **WHEN** `canon gate task` ships and is installed alongside the donor CLI's
  existing `hook run <kind>` entries
- **THEN** the donor CLI's `flipTaskDone`/`scanFakeMarkers` continue operating
  exactly as before, unmodified, until a separate donor-CLI-side change adopts
  the migration
