## ADDED Requirements

### Requirement: Every malformed plan-import construct is named by path and reason
A `PlanAdapter`'s `parse` SHALL report each malformed construct (an
unreadable directory, a basename failing the identity grammar, a directory
with no readable required file) as a NAMED entry carrying at minimum the
construct's relative path and a specific reason drawn from a fixed,
per-adapter reason vocabulary — never an anonymous increment to a bare
count with no way to identify WHICH construct failed or WHY. This applies
uniformly across every registered dialect via the shared `PlanParseOutcome`
shape (s17's `plan-import-connector` capability), not as an
openspec-adapter-only enrichment.

#### Scenario: A missing proposal.md is named by path and reason
- **WHEN** the openspec dialect adapter parses a change directory with a
  valid `ChangeId`-grammar basename but no readable `proposal.md`
- **THEN** the malformed entry names that directory's relative path and
  the reason `missing-proposal-md` — never a bare count with no path

#### Scenario: An invalid ChangeId basename is named by path and reason
- **WHEN** the openspec dialect adapter encounters a directory whose
  basename fails `ChangeId`'s kebab-slug grammar
- **THEN** the malformed entry names that directory's relative path and
  the reason `invalid-change-id-grammar`

#### Scenario: A malformed change dir still does not sink sibling imports
- **WHEN** one change dir under a source is malformed while its siblings
  are well-formed (s17's existing "one malformed change dir does not sink
  the pass" behavior)
- **THEN** every sibling still imports normally, and the malformed dir's
  named entry appears in the summary alongside the successfully imported
  siblings — the naming enrichment changes only what a malformed entry
  reports, never the skip-and-count control flow

### Requirement: A malformed changes-directory near-miss carries an actionable root hint
The importer SHALL attach an additional actionable hint to a malformed
`missing-proposal-md` entry whose directory basename is exactly `changes`:
the configured `root:` may be pointing at the changes directory's PARENT
rather than at (or above) `openspec/changes` itself — the concrete
misconfiguration shape where `root: <dir>` should have been
`root: <dir>/changes`.

#### Scenario: root: pointed one level above openspec/changes surfaces the hint
- **WHEN** a source's configured `root:` resolves to a directory `X` such
  that `X/openspec/changes` does not exist but `X` itself directly
  contains a subdirectory literally named `changes`, and that `changes`
  entry has no `proposal.md` of its own
- **THEN** the malformed entry for `changes` carries the `missing-proposal
  -md` reason plus the root-near-miss hint, naming the exact directory
  path

#### Scenario: A changes-named directory that legitimately IS a change is never hinted
- **WHEN** a directory named `changes` sits among a source's real change
  dirs but DOES carry a readable `proposal.md`
- **THEN** it imports as an ordinary `Change` record — no malformed entry,
  no hint, no special-casing of the basename when the directory is
  otherwise well-formed

### Requirement: A malformed-nonzero, zero-persisted source makes canon ingest plans non-clean at the process level
`canon ingest plans` SHALL treat a configured source that produced
`malformed > 0` AND persisted zero records
(`changes_persisted == 0 && tasks_persisted == 0` for that source) as
NON-CLEAN at the process level: the command SHALL print an unconditional
stderr WARN (regardless of `--json`) naming the source's dialect, root,
and malformed count, and SHALL exit with a non-zero `ExitCode` — never the
unconditional `ExitCode::SUCCESS` this condition currently produces. This
is distinct from (and does not replace) the existing loud, fail-BEFORE-scan
behavior for a malformed CONFIGURATION (unparseable `plans:` section,
unregistered dialect, nonexistent root), which keeps failing loud with its
own exit code before any parse is attempted.

#### Scenario: The root-one-level-too-high near-miss now exits non-zero
- **WHEN** `canon ingest plans` runs against a `plans:` source whose
  `root:` points one level above `openspec/changes` (the reproduced
  near-miss: zero changes parsed, one malformed `changes`-named entry,
  zero persisted records)
- **THEN** the command prints an unconditional stderr WARN naming the
  source's dialect, root, and malformed count (including the root-hint
  from the sibling requirement), and the process exits with a non-zero
  code — a `canon ingest plans && next-step.sh` CI chain SHALL now fail
  instead of shipping green while importing nothing

#### Scenario: A legitimately empty source stays a clean silent no-op
- **WHEN** `canon ingest plans` runs against a `plans:` source whose root
  exists, is well-formed, and genuinely contains zero change directories
  yet (`malformed == 0`, `changes_parsed == 0`)
- **THEN** the command exits `0` with no WARN printed for that source —
  the non-clean condition requires `malformed > 0`, never `persisted == 0`
  alone, so a fresh/empty plan tree is unaffected

#### Scenario: A source with some malformed dirs but at least one persisted record stays clean
- **WHEN** a source's pass has `malformed > 0` but ALSO
  `changes_persisted > 0` (a partially malformed tree with at least one
  well-formed change dir that imported successfully)
- **THEN** the command exits `0` — a partial success is not the targeted
  near-miss (skip-and-count of individually malformed constructs, s17's
  own established behavior, is unaffected)

#### Scenario: One flagged source among several makes the whole pass non-zero
- **WHEN** `canon ingest plans` runs against two configured sources, one
  well-formed (nonzero persisted records, zero malformed) and one hitting
  the malformed-nonzero/zero-persisted near-miss
- **THEN** the well-formed source's records persist normally and are
  visible in the summary, the flagged source's WARN is printed naming it
  specifically, and the process as a whole exits non-zero — the flag is
  per-source, the exit code is pass-wide, and neither source's outcome is
  silently absorbed by the other's

#### Scenario: A malformed configuration keeps failing loud before any scan, unchanged
- **WHEN** `canon.yaml`'s `plans:` section is itself malformed (a
  non-list `sources:`, an unregistered dialect id, or a nonexistent
  configured root — s17's existing `PlansError` paths)
- **THEN** the command fails exactly as it does today, before any source
  is scanned, with its own existing exit code — this requirement's
  non-clean condition is a SEPARATE, additional signal for a
  configuration that parses but yields nothing useful, never a
  replacement for the existing pre-scan configuration checks
