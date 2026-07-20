## ADDED Requirements

### Requirement: ProjectId newtype grammar
canon-model SHALL define a `ProjectId` newtype whose grammar is
`[a-z0-9][a-z0-9-]*` — lowercase ASCII alphanumerics and hyphens only,
first character alphanumeric, no underscore (keeping the `__`
natural-key separator unambiguous) — constructed only through a
validating parse, exactly like every other join-spine newtype.

#### Scenario: A valid project id parses
- **WHEN** `ProjectId::parse` is called with `"world-app"` (or any
  string matching `[a-z0-9][a-z0-9-]*`)
- **THEN** parsing succeeds and returns a `ProjectId` wrapping that
  value

#### Scenario: An id containing an underscore or uppercase is rejected
- **WHEN** `ProjectId::parse` is called with a string containing `_`
  (e.g. `"world_app"`) or an uppercase letter (e.g. `"World"`)
- **THEN** parsing fails with a grammar error, and no `ProjectId` is
  constructed

### Requirement: project_id required on Scenario/Review/Divergence, optional on EvidenceRecord
`Scenario`, `Review`, and `Divergence` SHALL each carry a REQUIRED
`project_id: ProjectId` field; `EvidenceRecord` SHALL carry an OPTIONAL
`project_id: Option<ProjectId>` field. A `Scenario`/`Review`/
`Divergence` record deserialized without a `project_id` SHALL fail to
parse; an `EvidenceRecord` without one SHALL parse successfully with
`project_id = None`.

#### Scenario: A Scenario/Review/Divergence record missing project_id fails to deserialize
- **WHEN** a `Scenario`, `Review`, or `Divergence` JSON record is
  deserialized with no `project_id` field present
- **THEN** deserialization fails — the record is rejected, never
  silently defaulted to an absent or inferred project_id

#### Scenario: An EvidenceRecord parses with or without project_id
- **WHEN** an `EvidenceRecord` JSON record is deserialized once with a
  `project_id` field present and once with it entirely absent
- **THEN** both deserialize successfully — the present case yields
  `Some(ProjectId)`, the absent case yields `None`

### Requirement: Composite project-prefixed natural keys in resolve_partition
`canon-store::partition::resolve_partition` SHALL prefix every
`Scenario`/`Review`/`Divergence` natural key with `<project_id>__`:
`Scenario`'s natural key becomes `<project_id>__<scenario_id>`,
`Review`'s becomes `<project_id>__<scenario_id>__<pin>`, and
`Divergence`'s becomes
`<project_id>__<scenario_id>__<run_seq>__<round>`. The prefix SHALL
always be applied, even for a single-project corpus, and this composite
key SHALL NOT introduce a new Hive `project=` partition directory —
`PartitionKey.area`'s shape is unchanged.

#### Scenario: Scenario natural key is project-prefixed
- **WHEN** `resolve_partition` resolves a `Scenario` record with
  `project_id = "world"` and `scenario_id = "world.firstbuy-hotdeal.26"`
- **THEN** the resolved natural key equals
  `"world__world.firstbuy-hotdeal.26"`

#### Scenario: Review and Divergence natural keys keep their prefix alongside their existing suffix
- **WHEN** `resolve_partition` resolves a `Review` record
  (`project_id`, `scenario_id`, `pin`) and a `Divergence` record
  (`project_id`, `scenario_id`, `run_seq`, `round`)
- **THEN** the `Review` natural key equals
  `<project_id>__<scenario_id>__<pin>` and the `Divergence` natural key
  equals `<project_id>__<scenario_id>__<run_seq>__<round>`

#### Scenario: No new Hive project= dimension is introduced
- **WHEN** a `Scenario` record's git-tier path is resolved
- **THEN** the path is
  `kind=scenario/area=<area>/<project_id>__<scenario_id>__<digest12>.json`
  — there is no additional `project=<project_id>/` path segment

### Requirement: Two roots sharing a scenario_id stay distinct records
Two `Scenario` records SHALL stay distinct when they share a
`scenario_id` but come from different `project_id`s — each resolves to
its own natural key and storage path, never merged or treated as
duplicates of one another.

#### Scenario: Same scenario_id under two projects stays two records
- **WHEN** root A (`project_id = "app-a"`) and root B
  (`project_id = "app-b"`) each declare a scenario tagged
  `world.firstbuy-hotdeal.26`
- **THEN** sync materializes two distinct `Scenario` records —
  `app-a__world.firstbuy-hotdeal.26` and
  `app-b__world.firstbuy-hotdeal.26` — at two distinct storage paths,
  neither deduplicating nor overwriting the other

### Requirement: Clean-cutover migration; a project_id-less record reads as malformed
`project_id` migration SHALL be a clean, fixture-only cutover — no
`Option`-guarded legacy branch is introduced — because zero real
`Scenario`/`Review`/`Divergence` producers exist prior to this change.
A stray on-disk record lacking `project_id` for one of these three
kinds SHALL be treated as malformed evidence — no evidence — never as
a legitimately-identified record with an inferred or default project.

#### Scenario: A stray pre-cutover record lacking project_id reads as malformed=no-evidence
- **WHEN** `GitTier::read` encounters an on-disk
  `Scenario`/`Review`/`Divergence` file with no `project_id` field
- **THEN** the record fails deserialization and is reported as
  malformed evidence — excluded from the returned record set, never
  silently accepted with a defaulted or inferred `project_id`
