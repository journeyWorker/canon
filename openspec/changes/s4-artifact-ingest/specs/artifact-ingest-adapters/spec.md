## ADDED Requirements

### Requirement: Four artifact adapters normalize to the S1 join spine
`canon-ingest` SHALL ship four adapters — a `canon.yaml`-configured-root
ledger adapter (`kind=run|review|clear|drill`; the donor consumer repo is the reference
source, never a hardcoded path), a configured-root divergence JSONL adapter
(manifest/review/remediation events), a handoff adapter (canon's own
Postgres-tier `Handoff` table, read via `canon-store`'s `Tier::read` —
never a live donor event-store / hosted Postgres connection), and an openspec change/task-state
adapter — each normalizing its source into canon-model `Event` records
keyed by the appropriate S1 join-spine identifier (`scenario_id`,
`handoff_id`, `change_id`/`task_id`).

#### Scenario: Ledger review record normalizes with its scenario_id
- **WHEN** the ledger adapter reads a `spec/ledger/kind=review/area=<area>/
  <scenario_id>.json` record
- **THEN** the normalized `Event` carries `scenario_id` equal to the source
  record's `scenario_id` field and `kind` equal to `review`

#### Scenario: Ledger run/drill records are read from their flat layout
- **WHEN** the ledger adapter reads a `spec/ledger/kind=run/<file>.json`
  record (flat, no `area=` partition level)
- **THEN** the adapter normalizes it without requiring an `area=` segment,
  matching the ledger's own layout distinction between
  `review/design-review/code-review/clear` (Hive `area=`-partitioned) and
  `run/drill` (flat)

#### Scenario: Divergence manifest and review lines are distinguished by type
- **WHEN** the divergence adapter reads a `.jsonl` file containing one
  `"type":"manifest"` line followed by one or more `"type":"review"` lines
- **THEN** the manifest line normalizes to a round-bookkeeping `Event` and
  each review line normalizes to a separate `Event` carrying its own
  `scenario_id` and `status`

#### Scenario: Handoff state transition normalizes with its handoff_id
- **WHEN** the handoff adapter observes a row in canon's own `handoffs`
  table (S1-wire-compatible with the prior event store's schema, read via `canon-store`'s
  Postgres tier) whose `state` is `done` and whose `id` is a known handoff
  identifier
- **THEN** the normalized `Event` carries `handoff_id` equal to that `id`
  verbatim (no re-derived identity) and, when present, the row's
  `openspecChangeSlug` as the event's `change_id`

#### Scenario: openspec task flip normalizes with its task_id
- **WHEN** the openspec adapter reads a `tasks.md` row that flipped from
  `- [ ]` to `- [x] <id> … — ✅ <evidence>`
- **THEN** the normalized `Event` carries `task_id` in the `<change_id>#<n>`
  grammar (S1) and the evidence string verbatim

### Requirement: Idempotent re-ingest across all four adapters
Re-running the artifact-ingest adapters over an unchanged source SHALL
produce no duplicate `Event` or verdict records, using the same
content-digest identity mechanism S3's session adapters use.

#### Scenario: Re-ingesting an unchanged ledger corpus adds nothing
- **WHEN** the ledger adapter ingests the same, unmodified
  `spec/ledger/**` corpus twice in succession
- **THEN** the second run's stored event count is unchanged from the first
