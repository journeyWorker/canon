## ADDED Requirements

### Requirement: A wholly-unproductive malformed pass never advances its source's watermark cursor
`canon ingest plans` SHALL NOT write a fresh watermark cursor for a
configured source whose pass yields `malformed > 0` (at least one malformed
construct) AND persists zero records (`changes_persisted == 0 &&
tasks_persisted == 0` for that source) — regardless of whether every
routable candidate that WAS attempted landed durably
(`changes_unwritten == 0 && tasks_unwritten == 0`). This condition SHALL be
the identical `malformed`/`changes_persisted`/`tasks_persisted` fact the
`loud-plan-import-diagnostics` capability's non-clean-source WARN already
computes for that source, never a separately-derived expression that could
diverge from it.

#### Scenario: A malformed-zero-persisted source withholds its cursor
- **WHEN** a configured source's pass finds at least one malformed construct
  and persists zero `Change`/`Task` records
- **THEN** `canon ingest plans` does not write a watermark cursor for that
  source — the source's on-disk cursor (if any existed from a prior pass)
  is left untouched, and no fresh cursor keyed off this pass's file set is
  created

#### Scenario: The withheld-cursor condition reuses the non-clean-source WARN's own fact
- **WHEN** a source's pass satisfies `loud-plan-import-diagnostics`'s
  non-clean condition (`malformed > 0 && changes_persisted == 0 &&
  tasks_persisted == 0`, driving that capability's unconditional stderr WARN
  and non-zero exit)
- **THEN** that SAME pass also withholds its cursor — a source is never
  flagged non-clean by the WARN while simultaneously having its cursor
  advanced, and never has its cursor withheld while being reported clean

### Requirement: An unchanged, wholly-malformed source re-attempts its full scan and re-emits every loud diagnostic on EVERY subsequent run
`canon ingest plans` SHALL re-run the full scan/parse/persist pass, on
every subsequent invocation, for a configured source that hit the
malformed-zero-persisted condition (and therefore has no advanced
watermark cursor), never silently short-circuiting via
`skipped_unchanged` — and SHALL re-emit the identical named malformed
entries, actionable hints, unconditional stderr WARN, and non-zero exit code
each time, for as many consecutive runs as the underlying misconfiguration
remains unfixed.

#### Scenario: A root-one-level-too-high near-miss stays loud across three consecutive runs
- **WHEN** `canon ingest plans` runs three times in succession against an
  unchanged `plans:` source whose `root:` points one level above
  `openspec/changes` (the s18-reproduced near-miss: zero changes parsed,
  one malformed `changes`-named entry, zero persisted records)
- **THEN** all three runs print the named malformed entry, the
  root-near-miss hint, the unconditional stderr WARN naming the source's
  dialect/root/malformed count, and exit non-zero — run #2 and run #3
  reproduce run #1's diagnostic byte-for-byte, never `skipped unchanged
  (watermark)` and never `exit 0`

#### Scenario: Fixing the source's config quiets it starting the very next run
- **WHEN** an operator corrects the misconfiguration (adds a real
  `proposal.md`-bearing change dir, or fixes `root:`) after one or more
  loud, non-zero-exit runs, and then runs `canon ingest plans` again
- **THEN** the corrected run parses and persists real records, is no longer
  malformed-zero-persisted, and (being otherwise fully durable) writes a
  fresh cursor — no manual cursor-reset step is required — and the run
  AFTER that, against the now-unchanged fixed source, exits `0` with
  `skipped unchanged (watermark)`, silent, exactly like any other
  legitimately clean source

### Requirement: A legitimately clean or partially-successful pass keeps its current watermark-advance behavior, unaffected
This capability's cursor-withholding condition SHALL apply exclusively to
the malformed-nonzero-zero-persisted case. Every other outcome — a
genuinely empty/fresh source (`malformed` empty, zero changes parsed), a
fully clean source (`malformed` empty, records persisted), and a
partially-malformed source that persisted at least one record
(`malformed > 0` but `changes_persisted > 0` or `tasks_persisted > 0`) —
SHALL advance its watermark cursor on a fully-durable pass exactly as it did
before this capability existed, and a subsequent unchanged re-run of any of
these SHALL still short-circuit via `skipped_unchanged` at `exit 0`, with no
WARN.

#### Scenario: A legitimately empty source stays a clean, cursor-advancing, silent no-op
- **WHEN** `canon ingest plans` runs twice against an unchanged source whose
  root exists, is well-formed, and genuinely contains zero change
  directories (`malformed` empty, `changes_parsed == 0`)
- **THEN** run #1 exits `0`, advances the cursor, and prints no WARN; run #2
  reports `skipped unchanged (watermark)` and exits `0` — unaffected by
  this capability's cursor-withholding condition

#### Scenario: A partially-malformed source with at least one persisted record stays a clean, cursor-advancing pass on repeat runs
- **WHEN** `canon ingest plans` runs twice, unchanged, against a source
  whose pass has `malformed > 0` but also `changes_persisted > 0` (some
  change dirs are malformed, at least one sibling imported successfully —
  s18's own "partial success is not the targeted near-miss" case)
- **THEN** run #1 persists the well-formed records, is flagged non-clean by
  `loud-plan-import-diagnostics`'s existing WARN (unchanged from s18), AND
  advances its cursor; run #2 reports `skipped unchanged (watermark)` and
  exits `0`, with no WARN — a partially-successful source goes quiet on its
  very next unchanged run, exactly as it did before this capability existed

#### Scenario: An unwritten-blocked pass keeps its existing (unrelated) cursor-withholding reason, unaffected by this capability's condition
- **WHEN** a source's pass has zero malformed constructs but at least one
  `Change`/`Task` candidate that could not be durably persisted (the
  existing `unwritten` seam — an unreachable routed tier)
- **THEN** the cursor is withheld for the existing `changes_unwritten > 0 ||
  tasks_unwritten > 0` reason, exactly as before this capability existed —
  this capability's malformed-nonzero-zero-persisted condition never
  applies (malformed is zero) and is not the reason cited for that
  withholding
