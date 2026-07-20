## ADDED Requirements

### Requirement: specs.roots[] config: stable-literal id, default, fail-loud
`canon.yaml` SHALL declare spec roots via a `specs.roots[]` list of
`{id, root}` entries; `id` SHALL be a stable, author-declared literal —
NEVER derived from the checkout directory name (that would split one
project's identity across clones) — and `root` SHALL default to `specs`
when omitted on an entry. When the `specs:` key is absent entirely,
canon SHALL resolve a single default root `{id: root, root: specs}`.
When a `specs:` key is PRESENT but its shape does not parse (malformed
`roots` entries, wrong types), config resolution SHALL fail loud rather
than silently falling back to the default.

#### Scenario: Missing specs: key resolves the default single root
- **WHEN** `canon.yaml` has no `specs:` key
- **THEN** spec-root resolution yields exactly one root
  `{id: "root", root: "specs"}`

#### Scenario: A malformed specs.roots entry fails loud, never defaults
- **WHEN** `canon.yaml` declares a `specs:` section whose `roots`
  entries are malformed (e.g. an entry missing its `id` field, or
  `roots` is not a list)
- **THEN** config resolution returns a load error naming the malformed
  section, and does NOT fall back to the default root

#### Scenario: A monorepo declares multiple named roots
- **WHEN** `canon.yaml` declares
  `specs.roots: [{id: app-a, root: apps/a/specs}, {id: app-b, root: apps/b/specs}]`
- **THEN** config resolution yields exactly those two roots, each
  keeping its own literal `id`

#### Scenario: id is never derived from the checkout directory
- **WHEN** the SAME `canon.yaml` `specs.roots[]` entry is resolved from
  two different checkout paths (e.g. two clones of the same repo at
  different absolute paths)
- **THEN** the resolved root's `id` is identical in both cases — the
  checkout directory name is never substituted for a missing or
  differing `id`

### Requirement: canon inventory sync validates before materializing, whole-root abort
`canon inventory sync` SHALL run S11 validation (`canon-fmt::check`)
against each configured root before materializing any `Scenario` record
for that root; ANY violation reported for a root SHALL abort
materialization for the WHOLE root, writing zero `Scenario` records for
it — never a partial sync that lands some records and skips others.

#### Scenario: A validation violation blocks the whole root's sync
- **WHEN** `canon inventory sync` runs against a root whose corpus
  contains at least one S11 validation violation
- **THEN** sync writes zero `Scenario` records for that root and
  reports the violation(s); no partial set of records is materialized

#### Scenario: A clean root proceeds to materialization
- **WHEN** `canon inventory sync` runs against a root whose corpus
  passes S11 validation with zero violations
- **THEN** sync proceeds to scan features and materialize `Scenario`
  records for that root

### Requirement: Gherkin line-scan retention and source_digest
`canon inventory sync` SHALL scan each root's `.feature` files using
canon-fmt's EXISTING line-scan (`gherkin::scan`) — never a new Gherkin
parser — surfacing each `@<area>.<surface>.<nn>` tag attached to its
following header, with the header label exposed as the scenario's
`title`; sync SHALL compute each `.feature` file's `source_digest` as a
full sha256 hash over the file's raw bytes.

#### Scenario: Tag-to-header linkage and title surface via the existing scan
- **WHEN** sync scans a `.feature` file whose `Scenario:` header is
  immediately preceded, within the same scenario block, by an
  `@<area>.<surface>.<nn>` tag
- **THEN** the materialized `Scenario` record's `title` equals the
  header's label text, joined to that same tag's `scenario_id`

#### Scenario: source_digest is a stable sha256 over file bytes
- **WHEN** sync scans the SAME unmodified `.feature` file twice
- **THEN** both scans compute the identical `source_digest` value, and
  that value is the sha256 hex digest of the file's exact byte content

### Requirement: Core Scenario index is general; covered/surface_ref are plugin-extensible, not core
The `Scenario` ledger-index SHALL carry only GENERAL, feature-corpus-derived
fields — `project_id`, `scenario_id`, `title`, `description`, `source_digest`.
`canon inventory sync` SHALL NOT DERIVE index fields from an
`upstream`/`InventoryEntry.covered_by` inventory — that donor-inventory→coverage
mapping is a donor-repo porting concern, NOT core canon (a general tool).
Coverage/surface enrichment (`covered`, `surface_ref`) is NOT precluded — it is
PLUGIN-EXTENSIBLE: a porting plugin (the s16 plugin mechanism, modeled on
the donor vocabulary project's / `canon-vocab`'s capability manifest) MAY own a plugin-namespaced
OVERLAY record carrying that data per `(project_id, scenario_id)`, which canon's
foreign-namespace handling (S2 `scan_corpus` skip+report) preserves without
clobbering and a plugin-aware read-time projection joins onto the core index.
Core neither hardcodes nor precludes it. Coverage remains the gate's own
authority (`uncovered-cell`) regardless.

#### Scenario: Sync derives the index from features alone, ignoring any inventory
- **WHEN** `canon inventory sync` runs against a root (whether or not an
  `upstream`/`covered_by` inventory directory happens to be present)
- **THEN** the materialized `Scenario` records are derived from the `.feature`
  corpus alone — sync never DERIVES index fields from
  `InventoryEntry.covered_by`/`upstream` (S11 validation of whatever family docs a
  project has is a separate, orthogonal concern), and no coverage/surface field
  is written to the core index

### Requirement: One Scenario ledger-index record per (project_id, scenario_id)
`canon inventory sync` SHALL materialize exactly one `Scenario`
ledger-index record per `(project_id, scenario_id)` pair, upserted
through the normal append-only Tier write path — never embedding the
family document's own rich body (steps, provenance, the `covered_by`
list) in the index record.

#### Scenario: Sync materializes one index record per scanned scenario
- **WHEN** sync scans a root whose features declare N distinct
  `scenario_id`s
- **THEN** sync writes exactly N `Scenario` records, one per
  `(project_id, scenario_id)` pair, each carrying `title` and `source_digest`
  and no other family-document content

### Requirement: Logical idempotence of inventory sync
Re-running `canon inventory sync` over an unchanged corpus SHALL write
zero new records; sync SHALL fold to the LATEST record per
`(project_id, scenario_id)` key and treat a re-sync as a no-op exactly
when the `source_digest` (and the title derived from it) match the latest
folded record. A changed `.feature` file SHALL produce exactly one new
`Scenario` record per affected scenario — an append, never an
overwrite.

#### Scenario: Re-sync of an unchanged corpus writes nothing
- **WHEN** `canon inventory sync` runs twice in a row against the same
  root with no changes to any `.feature`/inventory file in between
- **THEN** the second run writes zero new `Scenario` records

#### Scenario: A changed feature file produces exactly one new record
- **WHEN** a single `.feature` file is edited, changing its content
  and therefore its `source_digest`, and sync re-runs
- **THEN** sync writes exactly one new `Scenario` record for each
  scenario_id in that file, appended alongside — never overwriting —
  the prior record
