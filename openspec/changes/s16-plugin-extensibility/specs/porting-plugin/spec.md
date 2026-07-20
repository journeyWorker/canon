## ADDED Requirements

### Requirement: The porting plugin re-adds covered/surface_ref as an overlay, never as a core field
The `porting` plugin SHALL declare a `coverage` overlay attached to
`core_kind: scenario` on `join_key: [project_id, scenario_id]`, with
fields `covered: bool` and `surface_ref: {list: string}` — the exact
two fields `canon_model::Scenario` shipped in s15 P1 and removed in
P3a. `canon_model::Scenario` SHALL gain neither field back; the data
lives EXCLUSIVELY as a `porting.coverage` overlay record.

#### Scenario: The porting plugin's manifest declares the P1-shipped field shape
- **WHEN** `canon/plugins/porting/plugin.yaml` is resolved
- **THEN** its one overlay's fields are exactly `covered: bool` and
  `surface_ref: {list: string}` — no more, no fewer

#### Scenario: canon_model::Scenario carries no coverage field, with or without the porting plugin installed
- **WHEN** `canon_model::Scenario`'s struct definition is inspected,
  regardless of whether a `porting` plugin manifest is present in any
  project
- **THEN** it declares exactly the six s15 P3a fields (`envelope`/
  `project_id`/`scenario_id`/`title`/`description`/`source_digest`) —
  no `covered`, no `surface_ref`

### Requirement: The porting plugin derives covered/surface_ref from the donor inventory's covered_by join, inverted per scenario
`PortingOverlaySource` SHALL read a spec root's `inventory/**/*.yaml`
(`InventoryFile`/`InventoryEntry.covered_by`) and, for each
`(project_id, scenario_id)` the root's `.feature` corpus declares,
emit one overlay candidate whose `covered` is `true` if-and-only-if
that `scenario_id` appears in AT LEAST ONE `InventoryEntry.covered_by`
list anywhere in the root's inventory, and whose `surface_ref` is the
list of every inventory-entry key whose `covered_by` contains it
(empty when `covered` is `false`).

#### Scenario: A scenario appearing in one covered_by list projects covered=true with its surface_ref
- **WHEN** the inventory declares `idolive.hub.hub-header: {
  covered_by: [idolive.hub.01] }` and the root's features declare
  `scenario_id: idolive.hub.01`
- **THEN** `PortingOverlaySource` emits a candidate for `(project_id,
  idolive.hub.01)` with `covered: true` and `surface_ref:
  ["idolive.hub.hub-header"]`

#### Scenario: A scenario appearing in NO covered_by list projects covered=false with an empty surface_ref
- **WHEN** a root's features declare `scenario_id: world.hotdeal.99`,
  which appears in no inventory entry's `covered_by`
- **THEN** `PortingOverlaySource` emits a candidate for `(project_id,
  world.hotdeal.99)` with `covered: false` and `surface_ref: []`

#### Scenario: A scenario covered by multiple inventory entries collects every surface_ref
- **WHEN** TWO inventory entries both list `world.map.01` in their
  `covered_by`
- **THEN** the emitted candidate's `surface_ref` contains BOTH
  entries' keys

### Requirement: Coverage authority stays canon-gate's uncovered-cell check; the overlay is convenience, never a second authority
`canon-gate`'s `uncovered-cell` verdict SHALL read NOTHING from any
`porting.coverage` overlay record, before or after this change — the
overlay is a denormalized convenience for READ consumers (`canon
query --plugin porting`, reports, dashboards), never a second coverage
authority. A `canon plugin sync porting` run SHALL NEVER change any
`canon-gate` verdict.

#### Scenario: canon gate check's uncovered-cell verdict is unaffected by porting sync
- **WHEN** `canon gate check` runs against a repo BEFORE `canon plugin
  sync porting` has ever run, and again AFTER it has run and written
  overlay records
- **THEN** every `uncovered-cell` verdict (and every other gate
  verdict) is byte-identical between the two runs

#### Scenario: canon-gate's source carries no read of the porting namespace
- **WHEN** `canon-gate`'s crate source is inspected for any reference
  to the plugin-specific names `porting`, `porting.coverage`, or
  `scan_namespaced_kind` — canon-gate's own core `coverage` module and
  its `CoverageCheck`/`uncovered-cell` authority are UNAFFECTED and
  intentionally excluded from this check
- **THEN** none of the three plugin-specific names exists — canon-gate
  has zero code path that can read an overlay record of any kind,
  while its own core coverage-check code remains untouched

### Requirement: The porting plugin never mutates core Scenario records
`canon plugin sync porting` SHALL NEVER write to, or alter the
content of, any `kind=scenario/` file; it SHALL write ONLY under
`kind=porting.coverage/`. `canon query --plugin porting` (a read-only
command) SHALL NEVER write to any file at all.

#### Scenario: A porting sync run touches no core Scenario file
- **WHEN** `canon plugin sync porting` runs against a repo with
  existing `Scenario` records
- **THEN** every `kind=scenario/**/*.json` file's content and mtime
  are unchanged after the run (`Scenario` records are area-scoped on
  disk, e.g. `kind=scenario/area=<area>/...json`); only new files
  appear under `kind=porting.coverage/`

#### Scenario: A projection query touches no file at all
- **WHEN** `canon query --kind scenario --plugin porting` runs (a
  read-only command)
- **THEN** zero files under `kind=scenario/` or `kind=porting.coverage/`
  are created, modified, or deleted
