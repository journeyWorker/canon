## MODIFIED Requirements

### Requirement: Core Scenario index is general; covered/surface_ref are plugin-extensible, not core
The `Scenario` ledger-index SHALL carry only GENERAL, feature-corpus-
derived fields — `project_id`, `scenario_id`, `title`, `description`,
`source_digest`. `canon inventory sync` SHALL NOT DERIVE index fields
from an `upstream`/`InventoryEntry.covered_by` inventory — that
donor-inventory→coverage mapping is a donor-repo porting concern,
NOT core canon (a general tool). Coverage/surface enrichment
(`covered`, `surface_ref`) is NOT precluded — it IS plugin-extensible:
the s16 `porting` plugin (`plugin-overlay-registry`/
`plugin-overlay-records`/`plugin-overlay-projection`/`porting-plugin`
capabilities) owns a `porting.coverage` overlay record per
`(project_id, scenario_id)`, which canon's foreign-namespace handling
(`GitTier::scan_corpus`) preserves without clobbering and `canon query
--plugin porting` (a plugin-aware read-time projection) joins onto the
core index. Core neither hardcodes nor precludes it. Coverage remains
the gate's own authority (`uncovered-cell`) regardless.

#### Scenario: Sync derives the index from features alone, ignoring any inventory
- **WHEN** `canon inventory sync` runs against a root (whether or not
  an `upstream`/`covered_by` inventory directory happens to be present)
- **THEN** the materialized `Scenario` records are derived from the
  `.feature` corpus alone — sync never DERIVES index fields from
  `InventoryEntry.covered_by`/`upstream`, and no coverage/surface field
  is written to the core index

#### Scenario: The plugin-extensible promise is now backed by a working overlay
- **WHEN** the `porting` plugin's `canon/plugins/porting/plugin.yaml`
  is installed and `canon plugin sync porting` has run against a root
  `canon inventory sync` already indexed
- **THEN** `canon query --kind scenario --plugin porting` projects
  `covered`/`surface_ref` onto every `Scenario` record from that
  root's `porting.coverage` overlay — the SAME two fields P1 shipped
  as core fields and P3a removed, now sourced entirely outside
  `canon-model`

#### Scenario: Coverage remains the gate's authority even with the overlay installed
- **WHEN** `porting`'s overlay reports `covered: false` for a scenario
  that `canon-gate`'s OWN evidence-based coverage check considers
  covered (or vice versa — the two sources are independently derived
  and may disagree)
- **THEN** `canon gate check`'s `uncovered-cell` verdict is governed
  ENTIRELY by its own evidence-based check; the overlay's `covered`
  value has no effect on that verdict either way
