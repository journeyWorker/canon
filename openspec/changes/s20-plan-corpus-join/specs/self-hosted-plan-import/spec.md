## ADDED Requirements

### Requirement: Canon's own root canon.yaml configures a plans source over its own openspec/changes tree
Canon's own repo-root `canon.yaml` SHALL carry a `plans:` section
(`sources: [{dialect: openspec, root: openspec/changes}]`) — the
DIRECT-changes-dir root shape `discover_change_dirs` already tolerates,
never the `root: openspec` near-miss shape that makes `discover_change_
dirs` treat `openspec/changes` itself as one malformed dir (tracked
separately). This SHALL be the first live, non-fixture, non-test
`plans:` configuration in canon's own repo.

#### Scenario: The configured root resolves to the real changes tree
- **WHEN** `canon ingest plans` reads canon's own root `canon.yaml`
- **THEN** the resolved source root is `<repo>/openspec/changes`, and
  `discover_change_dirs` enumerates every live change dir under it
  (non-zero, matching the on-disk directory count) — never zero changes
  discovered against a non-empty tree

### Requirement: canon ingest plans imports every one of canon's own live change dirs
Running `canon ingest plans` against canon's own repo SHALL import one
`Change` record per live (non-archive) `openspec/changes/<slug>/` dir
present on disk at run time, each with `change_id` equal to the dir
basename verbatim, and `status` derived per s17's existing D6 rules from
that dir's own `tasks.md` checkbox tallies.

#### Scenario: Every live change dir yields exactly one Change record
- **WHEN** `canon ingest plans` runs against canon's own repo, whose
  `openspec/changes/` holds N live change dirs and an empty `archive/`
- **THEN** the pass reports N `Change` records imported (zero
  `malformed`, zero `duplicate-change-id`), and `canon/ledger/
  kind=change/` on disk gains N corresponding files

#### Scenario: Task records persist to pg when reachable, degrade to unwritten otherwise
- **WHEN** the same pass runs with `task: pg` routed (canon's own
  `canon.yaml` `routing:`) and `CANON_PG_DSN` either set to a reachable
  database or unset
- **THEN** with a reachable DSN, every parsed `Task` persists into
  `canon_v1.task`; with no reachable DSN, every parsed `Task` candidate
  is reported through the documented `unwritten` seam (printed, non-
  fatal) while the git-routed `Change` records still persist normally,
  and the source's watermark cursor is NOT advanced in the unwritten
  case

### Requirement: The self-referential source root never triggers runaway re-scan churn
`canon ingest plans`'s per-source content-digest watermark cursor SHALL
exclude the importer's own git-ledger root (`tiers.git.root`,
`canon/ledger`) and cursor tree (`canon/ingest/cursors`) from the
digested file set for any configured source, including one whose root
is canon's own repo — so importing canon's own `openspec/changes` never
causes the importer's own writes to shift the next pass's digest before
it settles.

#### Scenario: A self-hosting pass followed immediately by another writes nothing new
- **WHEN** `canon ingest plans` runs twice in immediate succession
  against canon's own `openspec/changes` root, with no source-tree edits
  between runs
- **THEN** the second pass's cursor-digest check matches the first
  pass's recorded digest exactly (the first pass's own ledger/cursor
  writes are excluded from that digest) and skips the source wholesale
  — zero new records

### Requirement: Self-hosted plan import is idempotent and leaves gate authority untouched
Re-running `canon ingest plans` against canon's own repo with an unchanged `openspec/changes` tree SHALL write zero new records
(idempotence, per s17's existing determinism contract). `canon gate
check` verdicts SHALL be byte-identical before and after the
self-hosting import runs.

#### Scenario: An unchanged self-hosting re-run is a clean no-op
- **WHEN** `canon ingest plans` runs a second time against canon's own
  repo with no changes to any `openspec/changes/**` file since the
  first run
- **THEN** the pass reports zero new `Change`/`Task` records and the
  cursor is not advanced past its already-current watermark

#### Scenario: Gate authority is unaffected by the self-hosting import
- **WHEN** `canon gate check` runs against canon's own repo before the
  first `canon ingest plans` self-hosting pass and again after it
- **THEN** every gate verdict is byte-identical between the two runs
</content>
