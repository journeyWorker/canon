## ADDED Requirements

### Requirement: Incremental ingest advances a per-source watermark
Each adapter SHALL persist a watermark cursor (`{source_id, last_seen_at,
last_seen_digest}`) through canon-store after a successful scan, and a
subsequent `canon ingest sessions` run SHALL only read source data newer than
the persisted watermark.

#### Scenario: Second run over an unchanged fixture set scans nothing new
- **WHEN** `canon ingest sessions` runs twice in succession over the same,
  unmodified fixture corpus
- **THEN** the second run's per-adapter "records scanned" count is zero, and
  its normalized output is byte-identical to the first run's

#### Scenario: Watermark advances past newly appended transcript lines
- **WHEN** a Codex `.jsonl` session file gains new lines after the first
  ingest run and `canon ingest sessions` runs again
- **THEN** only the newly appended lines are scanned, and the resulting
  normalized records are appended to the existing output without altering
  previously ingested records

### Requirement: Idempotent re-ingest via content digest
Every normalized record SHALL carry a stable content digest used as its
identity for canon-store's write path, so re-emitting an already-stored
record (watermark reset, restarted `--watch`, concurrent run) SHALL never
duplicate or double-count that record.

#### Scenario: Watermark reset does not duplicate stored records
- **WHEN** an adapter's watermark cursor is deleted or reset to zero and
  `canon ingest sessions` re-scans the full fixture corpus from the start
- **THEN** every record whose content digest already exists in the store is
  skipped as a duplicate, and the store's record count is unchanged from
  before the reset

### Requirement: Source-level record reconciliation before cost summing
The `claude` and `codex` adapters SHALL, before a record contributes to a
token/cost row, reconcile source-level duplicate/cumulative records:
`claude` SHALL dedup streamed re-writes of the same logical message by a
composite `messageId:requestId` key, merging duplicates by taking the
per-field max across them; `codex` SHALL treat `token_count` events as
cumulative session totals, diffing each new total against the previous
snapshot (never per-line summing), and SHALL detect a forked-child session
replaying its parent's history so the parent's tokens are not
double-counted under the child's own session id.

#### Scenario: Claude Code streamed duplicate messages merge by per-field max
- **WHEN** the `claude` adapter parses a session where the same logical
  message was re-written multiple times as a streaming response completed
  (same `messageId`/`requestId` pair, differing token counts per write)
- **THEN** the adapter emits exactly one normalized record for that message,
  with each token field equal to the maximum value seen across the
  duplicate writes — never a per-line sum of the duplicates

#### Scenario: Codex cumulative token totals are diffed, not summed
- **WHEN** the `codex` adapter parses a sequence of `token_count` events
  whose values are cumulative session totals
- **THEN** the adapter emits a token/cost row per event equal to the
  difference from the previous total (never the raw cumulative value
  summed across events), and a regressed/stale total (e.g. after a context
  compaction) does not produce a negative or inflated delta

#### Scenario: Codex forked-child session does not double-count parent tokens
- **WHEN** the `codex` adapter encounters a session that forked from a
  parent session and replays the parent's history under its own session id
- **THEN** the adapter attributes the replayed history's tokens to the
  fork-source identity (keyed on `session_forked_from_id` when present,
  falling back to a session-metadata id, never the adapter's own
  filename-derived surface id), so the parent's tokens are not counted a
  second time under the child's `session_id`

### Requirement: Cost parity with the vendored upstream launcher on fixture input
canon-ingest's normalized token/cost rows SHALL match the vendored upstream launcher's own computed
cost for the same fixture transcript input, within rounding tolerance.

#### Scenario: Fixture-corpus cost matches the launcher's recorded output
- **WHEN** `canon ingest sessions` runs over the S3 fixture corpus (sanitized
  real transcript samples from omp/pi, Claude Code, and Codex)
- **THEN** the sum of normalized cost rows per `session_id` equals the launcher's
  checked-in expected cost for that session, within the fixture's declared
  rounding tolerance
