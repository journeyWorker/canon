## Why

canon is a SELF-COMPLETE harness (s15's own framing): `install canon` and
the native ledger loop works without depending on an external tool. s15
delivered pillars 1+2 (native ledger management + self-improvement over
ledger + reviews) and, along the way, built the SEAM this change fills —
deliberately, not by accident. Three places in s15's own delivered code
name s16 as the intended consumer:

- `canon-store::GitTier::scan_corpus` (`git_tier.rs:138-171`) treats an
  unrecognized `kind=<x>/` directory as a **foreign-namespace notice**,
  never a malformed-core violation — the `scenario-spine-layout` spec's
  own words: "the forward-compatibility seam that lets a future s16
  plugin kind coexist without breaking an s15 consumer."
- `RecordKind` (`envelope.rs:20-33`) is a FROZEN, structurally-asserted
  12-member closed set (`RecordKind::ALL.len() == 12`, asserted in three
  independent places). s15 design D1: a 13th kind is "a reviewed,
  breaking canon-model change — never a `kind: String` escape hatch",
  scoped to the CORE `canon.` namespace only: "s16 plugin kinds live in
  a separate namespaced registry and are never `RecordKind` variants."
- `canon_model::Scenario` (`records.rs:106-123`) shipped `covered`/
  `surface_ref` in s15 P1 (`488c0093`), then DROPPED them in P3a
  (`d084e850`) precisely because canon has no general populator for
  them — a donor-repo-specific `upstream`/`InventoryEntry.covered_by`
  join is a PORTING concern, not a general spec-planning fact. The doc
  comment left on the type is explicit: "a future s16 porting plugin
  owns it as a foreign-namespace overlay record, never as a field that
  core re-materializes here, because a plugin cannot safely own a field
  that core clobbers on every sync." `inventory-materialization`'s own
  Requirement ("Core Scenario index is general; covered/surface_ref are
  plugin-extensible, not core") makes the SAME promise in spec form,
  naming the donor vocabulary project/canon-vocab manifest pattern as the model to follow.

**The gap this change closes:** the seam exists and is TESTED (three
`scan_corpus` unit tests pin the unknown-kind/known-kind boundary), but
nothing POPULATES it. There is no plugin manifest, no overlay write
path, no read-time projection, and no concrete plugin proving the
mechanism against a real donor corpus. Without s16, s15's `covered`/
`surface_ref` doc comments and spec language describe a mechanism that
does not exist — a donor-repo consumer that wants coverage-by-scenario
back has no supported way to get it, and `inventory-materialization`'s
plugin-extensible promise stays a forward reference, never a proof.

**Precedent that this is buildable, not speculative:** `canon-vocab`
(S10) already retargeted the donor vocabulary project's plugin/manifest/resolution
architecture at a DIFFERENT canon domain (task-atom authoring
vocabulary) — one `plugin.yaml` shape, one `resolve_snapshot(project_dir,
profile) -> (CapabilitySnapshot, Vec<Diagnostic>)` entry point every
consumer shares, source-imported (not a git dependency) because canon
must stay standalone. s16 mirrors that SAME architecture for a
DIFFERENT surface (ledger-record overlays, not authoring vocabulary) —
the two `plugin.yaml` files are NEVER the same manifest or the same
directory, disambiguated explicitly below and in `design.md` (R4) so a
future reader never conflates canon-vocab's `canon/vocab/<id>/
plugin.yaml` with s16's `canon/plugins/<id>/plugin.yaml`.

**Scope discipline (companion to s15's own non-goals):** s15's proposal
already drew the line — "Plugin-defined kinds/projections … and
corpus-authoring scaffolds (`canon scenario new`) → s16"; "[external
plan/corpus] import of any dialect … → s17". This change is EXACTLY
that s16 scope: it extends the ledger/inventory STRUCTURE (new overlay
kinds joined onto core records, a generator for the hand-authored
corpus s15's pipeline consumes) — it is NOT s17's job of importing a
foreign PLANNING dialect (openspec/superpowers/donor-JSON) into canon's
own format. Coverage AUTHORITY stays exactly where s15 left it —
`canon-gate`'s `uncovered-cell` check — an overlay is convenience,
never a second authority.

## What Changes

- **Ledger-overlay plugin manifest + registry.** A NEW `canon/plugins/
  <id>/plugin.yaml` (a plugin `id` + a NAMESPACE + one-or-more overlay
  declarations: the overlay kind name, the CORE record kind + join key
  it attaches to, and its projected field(s) with structural types)
  resolved through ONE entry point, `resolve_plugin_snapshot`,
  mirroring `canon-vocab::resolve_snapshot`'s architecture (fail-soft,
  total, one snapshot every consumer shares) — a NEW crate,
  `canon-plugin`, not a canon-vocab extension (a distinct manifest
  content-domain; canon-vocab's `plugin.yaml` stays the
  authoring-vocabulary surface, untouched).
- **Overlay record write, validated against the manifest.** Overlay
  records live under `kind=<namespace>.<kind>/`, a namespaced string
  the frozen `RecordKind` type does not — and must never — recognize.
  Two narrow `GitTier` additions — `write_namespaced`/
  `scan_namespaced_kind` — generalize the EXISTING append-only,
  content-digest-suffixed algorithm (`partition.rs`) off an arbitrary
  namespaced string instead of the closed `RecordKind` enum; a
  plugin-aware writer validates a candidate overlay body's fields
  against the plugin's declared schema BEFORE calling it.
- **Read-time projection.** A pure `project_overlay` JOIN: given a
  resolved plugin snapshot, the core index records, and a plugin's own
  overlay records, project the declared field(s) onto the core view in
  memory — core records on disk are NEVER rewritten. Fail-soft: an
  absent plugin, an absent manifest, or an absent overlay record for a
  given join key all degrade to the unmodified core view, never a
  panic or error. `canon query --kind <k> --plugin <id> [--json]`
  (extending S2's existing `canon query`) is the first read consumer.
- **`porting` plugin — the acceptance vehicle.** One concrete
  `canon/plugins/porting/plugin.yaml` (namespace `porting`, overlay
  kind `coverage`, attached to `Scenario` on `(project_id,
  scenario_id)`, fields `covered: bool` + `surface_ref: list<string>`)
  plus ONE concrete `OverlaySource` adapter (mirrors S3's per-client
  `ArtifactAdapter`/s15's per-kind flywheel-adapter pattern — core
  stays generic, exactly one source-specific implementation exists)
  that reads the donor `inventory/` YAML (`InventoryEntry.covered_by`,
  already S11-validated as ordinary corpus hygiene by
  `canon-fmt::check`) and inverts it into one `covered`/`surface_ref`
  overlay record per `(project_id, scenario_id)` — re-adding, as an
  OVERLAY, exactly the two fields s15 P1 shipped and P3a dropped from
  core. `canon plugin sync porting [--spec-root <dir>]` runs it.
  Coverage AUTHORITY is untouched: `canon-gate`'s `uncovered-cell`
  check reads NOTHING from this overlay, ever.
- **Corpus-authoring scaffold.** `canon scenario new <area>.<surface>.
  <nn> --title <label> --feature <path>` (and `canon feature new
  <area>.<surface> --title <label>`) generate a `.feature` stub
  carrying the `# canon:` provenance comment + a `@<area>.<surface>.
  <nn>`-tagged `Scenario:` header, so authors produce the
  S11-validated corpus `canon inventory sync` already consumes. No new
  DSL, no ledger record write — the scaffold's ONLY output is the same
  hand-authored `.feature` file format `canon-fmt::gherkin::scan`
  already reads.
- **Companion skill** `canon/skills/canon-plugins/SKILL.md` — author a
  plugin manifest → validate an overlay write → project a read → the
  `porting` plugin as a worked example.

### Added Capabilities

- `plugin-overlay-registry`: `canon/plugins/<id>/plugin.yaml` manifest
  (id + namespace + overlay declarations: kind, `attaches_to
  {core_kind, join_key}`, projected `fields`), resolved through ONE
  entry point (`resolve_plugin_snapshot`) into a `PluginSnapshot`,
  mirroring canon-vocab's manifest/`resolve_snapshot` architecture but
  as a distinct crate/directory/schema (never the same surface as
  canon-vocab's authoring-vocabulary `plugin.yaml`).
- `plugin-overlay-records`: overlay records under `kind=<namespace>.
  <kind>/`, written via two narrow `canon-store` additions
  (`write_namespaced`/`scan_namespaced_kind`) that generalize the
  existing append-only, content-digest-suffixed algorithm off a string
  instead of `RecordKind`; a namespaced-kind string colliding with a
  core `RecordKind::as_str()` value is rejected, never written; a
  plugin-aware writer validates a candidate body against the manifest's
  declared schema before every write.
- `plugin-overlay-projection`: a pure, deterministic, fail-soft
  read-time JOIN (`project_overlay`) projecting a plugin's declared
  overlay field(s) onto the core view in memory, exposed via `canon
  query --plugin <id>`; core records are read-only inputs, never
  mutated; core neither hardcodes nor precludes any overlay's fields.
- `porting-plugin`: the acceptance vehicle — a concrete
  `canon/plugins/porting/plugin.yaml` + `PortingOverlaySource` adapter
  that inverts the donor `inventory/` YAML's `covered_by` join into
  per-`(project_id, scenario_id)` `covered`/`surface_ref` overlay
  records, proving the mechanism end-to-end without any core
  (`canon-model`/`canon-store`/`canon-gate`) code naming `porting`.
  Coverage authority stays `canon-gate`'s `uncovered-cell`; core
  `Scenario` records are never mutated.
- `corpus-authoring-scaffold`: `canon scenario new`/`canon feature new`
  generate an S11-conformant `.feature` stub (provenance comment +
  tagged header) — a template generator only, no DSL, no ledger-record
  write, feeding the same hand-authored corpus s15's pipeline consumes.

### Modified Capabilities

- `inventory-materialization` (s15): the "Core Scenario index is
  general; covered/surface_ref are plugin-extensible, not core"
  requirement's PROMISE (a porting plugin MAY own this data) becomes a
  PROOF — the `porting-plugin` capability is a concrete instance a
  reader can point at, and a new scenario asserts coverage stays the
  gate's authority even with the overlay installed.
- `scenario-spine-layout` (s15): the "unrecognized `kind=<x>/`
  directory is skipped and reported as foreign-namespace" requirement
  gets its first REAL consumer — the plugin-overlay write/read
  primitives exercise the exact seam s15 built and tested against a
  synthetic `plugin-widget` fixture, now proven against a real
  `porting.coverage` overlay directory.

### Explicit non-goals

- No new `RecordKind` variant — the 12-member closure (design D1,
  `envelope.rs:41-54`) is UNCHANGED; a namespaced overlay kind is never
  a `RecordKind`.
- No core field for `covered`/`surface_ref` (or any other overlay
  data) — `canon_model::Scenario` stays the general 6-field index P3a
  landed; this change never re-adds them to core.
- No core (`canon-model`/`canon-store`/`canon-gate`) code that names
  `porting` or any other specific plugin — the generic manifest/write/
  validate/project primitives are the ONLY thing core-adjacent crates
  gain; `porting` is exactly one `OverlaySource` implementation behind
  that generic surface, the same shape S3's per-client adapters and
  s15's per-kind flywheel adapters already use.
- No plugin execution sandbox, dependency-version graph, or
  marketplace — a plugin is a declarative YAML manifest plus (for a
  data-producing plugin like `porting`) ONE hand-written,
  canon-cli-registered adapter; there is no general "run arbitrary
  plugin code" mechanism.
- No CEL/expression layer for overlay field types or an `applies_when`
  gate — overlay field types are the SAME bare-scalar/`{enum}`/
  `{list}` structural shape `canon-vocab::manifest::types::Type`
  already validates (`type_accepts`), reused by inspiration, not
  import; canon-vocab's own reserved `Type::AppliesWhen` (its D7) stays
  reserved there too.
- No external plan/corpus import of any dialect (openspec, superpowers,
  donor-JSON re-homing) — unchanged s17 scope.
- No change to coverage AUTHORITY — `canon-gate`'s `uncovered-cell`
  check is untouched; an overlay is read by NOTHING inside
  `canon-gate`.
- No write access from a plugin into ANY `kind=<x>/` directory
  recognized by `RecordKind` — `write_namespaced` REJECTS (loud, not
  silent) a namespaced-kind string colliding with a core
  `RecordKind::as_str()` value.
- No Gherkin parser extension beyond the existing line-scan; the
  corpus scaffold emits a plain-text template, never a generated AST.

## Impact

- **New crate `canon-plugin`**: manifest loader (`plugin.yaml` parse —
  a schema distinct from canon-vocab's), `resolve_plugin_snapshot`,
  overlay schema validation, `project_overlay`.
- **`canon-store`**: two narrow `GitTier` additions
  (`write_namespaced`, `scan_namespaced_kind`) generalizing the
  existing digest-suffix/append-only algorithm off a string kind; a
  namespaced-kind collision with a core `RecordKind::as_str()` value is
  rejected at write time.
- **`canon-cli`**: `canon plugin sync <id>` (generic `OverlaySource`
  dispatcher, one registered `porting` adapter), `canon scenario new`/
  `canon feature new` (template generator), `canon query --plugin
  <id>` (projection read consumer).
- **`canon-model`/`canon-gate`**: UNCHANGED. Zero new `RecordKind`
  variants, zero new core fields, zero new gate logic — the closure
  design D1 promised stays intact.
- **New companion skill** `canon/skills/canon-plugins/SKILL.md` +
  install-lock bump.
- **Record-kind stability, restated:** `RecordKind::ALL` stays 12
  members after this change, structurally asserted the same way s15
  asserts it — this is the acceptance bar for "s16 never breaks an
  s15 consumer."
