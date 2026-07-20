## ADDED Requirements

### Requirement: A ledger-overlay plugin manifest declares namespace, overlay kind(s), core attachment, and projected fields
A `canon/plugins/<id>/plugin.yaml` manifest SHALL declare a plugin `id`,
a `namespace` string, and one or more `overlays` entries, each naming
an overlay `kind` (combined with `namespace` as `<namespace>.<kind>` —
the on-disk `kind=` directory string), the CORE `RecordKind` + join-key
field(s) it attaches to (`attaches_to: { core_kind, join_key }`), and
its projected field(s) (`fields: [{name, type}]`). A manifest missing
any of `id`/`namespace`/`overlays[].kind`/`attaches_to.core_kind`/
`attaches_to.join_key` SHALL fail to load, reported as a diagnostic,
never silently defaulted. `namespace` and each overlay `kind` SHALL
each be a kebab token matching `[a-z0-9]+(-[a-z0-9]+)*` — so the
combined `<namespace>.<kind>` on-disk directory string is exactly two
dot-joined kebab tokens, containing no `/`, `..`, or other path
separator; a manifest whose `namespace` or any overlay `kind` fails
this grammar SHALL fail to load with a diagnostic, exactly like a
missing required field. In s16, `attaches_to.core_kind` SHALL be
exactly `scenario`; manifest resolution SHALL REJECT, loud, any
`attaches_to.core_kind` naming any other `RecordKind` — a generic
projection over other core kinds is explicit FUTURE work, a non-goal
of s16 (see `plugin-overlay-projection`).

#### Scenario: A well-formed manifest resolves its declared shape
- **WHEN** `canon/plugins/porting/plugin.yaml` declares `id: porting`,
  `namespace: porting`, and one overlay `kind: coverage` attached to
  `core_kind: scenario` on `join_key: [project_id, scenario_id]` with
  fields `covered: bool` and `surface_ref: {list: string}`
- **THEN** the resolved `PluginSnapshot` carries exactly that plugin,
  with its overlay's `core_kind`, `join_key`, and both declared fields
  intact

#### Scenario: A manifest missing a required field fails to load, not silently defaulted
- **WHEN** a `plugin.yaml` declares an `overlays` entry with no
  `attaches_to.join_key`
- **THEN** manifest loading reports a diagnostic naming the missing
  field, and that plugin is EXCLUDED from the resolved snapshot rather
  than resolving with an empty or guessed join key

#### Scenario: A namespace or overlay kind failing the kebab-token grammar fails to load
- **WHEN** a `plugin.yaml` declares `namespace: Porting_Two` (uppercase
  and an underscore) or an overlay `kind: "coverage/extra"` (a `/`
  inside the token)
- **THEN** manifest loading reports a diagnostic naming the
  grammar-violating value, and that plugin is EXCLUDED from the
  resolved snapshot

#### Scenario: A manifest declaring a non-scenario core_kind fails resolution loud
- **WHEN** a `plugin.yaml` declares an overlay with `attaches_to:
  { core_kind: task, join_key: [project_id, task_id] }`
- **THEN** manifest resolution reports a diagnostic naming `core_kind`
  as unsupported, and that plugin is EXCLUDED from the resolved
  snapshot — s16 projects onto `core_kind: scenario` only

### Requirement: One resolution entry point folds every installed ledger-overlay plugin into a single PluginSnapshot
`canon-plugin` SHALL expose exactly ONE capability-snapshot resolution
function — mirroring `canon-vocab::resolve_snapshot`'s contract — that
scans every `canon/plugins/<id>/plugin.yaml` under a project directory
and folds them into one `PluginSnapshot`; no second, independently-
computed plugin view SHALL exist anywhere in the crate or its
consumers. The resolution SHALL be pure, total, and never panic —
every failure (a missing `canon/plugins/` directory, a malformed
manifest, a duplicate plugin id) degrades to a usable (possibly
plugin-empty) snapshot plus a diagnostic.

#### Scenario: Every consumer of the plugin surface resolves through the same function
- **WHEN** the overlay-write path (`plugin-overlay-records`) and the
  projection-read path (`plugin-overlay-projection`) both need the
  resolved plugin snapshot for the same project directory
- **THEN** both call the identical `resolve_plugin_snapshot(project_dir)`
  function — neither computes its own partial parse of
  `canon/plugins/`

#### Scenario: An absent canon/plugins/ directory resolves an empty, valid snapshot
- **WHEN** `resolve_plugin_snapshot` runs against a project directory
  with no `canon/plugins/` directory at all
- **THEN** it returns a `PluginSnapshot` with zero plugins and zero
  diagnostics — never a panic or an `Err`

#### Scenario: A duplicate plugin id drops the later package, reported as a diagnostic
- **WHEN** two directories under `canon/plugins/` declare the same
  manifest `id`
- **THEN** resolution keeps the first (sorted-order) package, drops
  the second, and reports a diagnostic naming the duplicate — never
  silently overwriting or merging the two

### Requirement: A ledger-overlay plugin.yaml is a distinct surface from canon-vocab's authoring-vocabulary plugin.yaml
A ledger-overlay manifest SHALL live under `canon/plugins/<id>/
plugin.yaml`, a directory distinct from canon-vocab's
authoring-vocabulary manifest at `canon/vocab/<id>/plugin.yaml`;
`canon-plugin` SHALL NOT depend on the `canon-vocab` crate, and
neither manifest schema SHALL be interchangeable with the other (a
`canon/vocab/<id>/plugin.yaml` under `directives`/`enums`/`kind:
{core,project}` is never a valid ledger-overlay manifest, and vice
versa).

#### Scenario: The two plugin.yaml directories coexist without collision
- **WHEN** a project declares BOTH `canon/vocab/my-tasks/plugin.yaml`
  (an authoring-vocabulary plugin) and `canon/plugins/porting/
  plugin.yaml` (a ledger-overlay plugin)
- **THEN** `canon_vocab::resolve_snapshot` resolves only the former
  and `canon_plugin::resolve_plugin_snapshot` resolves only the
  latter — each scan is scoped to its own directory and ignores the
  other's directory entirely

#### Scenario: A ledger-overlay manifest under the vocab directory is not picked up
- **WHEN** a `plugin.yaml` matching the ledger-overlay schema
  (namespace/overlays) is mistakenly placed under `canon/vocab/<id>/
  plugin.yaml`
- **THEN** `canon_vocab`'s loader (expecting `directives`/`enums`
  exports) reports a load diagnostic for that package rather than
  silently treating it as a valid authoring-vocabulary plugin
