# Design — s16 plugin extensibility

## Current state (accurate baseline, verified)

- **The seam s15 built (three citations, exact).**
  `canon-store::GitTier::scan_corpus` (`git_tier.rs:138-171`) walks
  every `kind=<x>/` directory; a `<x>` matching one of `RecordKind::
  ALL`'s 12 wire strings is scanned exactly as before, anything else is
  pushed onto `CorpusScanResult.foreign_namespaces` and never descended
  into or validated — three unit tests (`git_tier.rs:425-496`) pin
  this: an unknown kind never contributes a `malformed-evidence`
  violation, a malformed record under a KNOWN kind still fails loud,
  and two known kinds in the same scan are unaffected by the rule.
- **The closure that makes the seam SAFE.** `RecordKind`
  (`envelope.rs:18-33`) is a 12-variant enum; `RecordKind::ALL.len() ==
  12` is asserted independently in `envelope.rs:200`, `canon-store/
  tests/git_tier_all_kinds.rs:42`, and `canon-policy/src/
  registry.rs:249`. s15 design D1: a 13th CORE kind is "a reviewed,
  breaking canon-model change — never a `kind: String` escape hatch";
  the SAME paragraph scopes that closure to the `canon.` namespace and
  names s16's plugin kinds as living in "a separate namespaced
  registry ... never `RecordKind` variants."
- **The dropped field, with an explicit doc-comment pointer.**
  `canon_model::Scenario` (`records.rs:106-123`) carries `envelope +
  project_id + scenario_id + title + description + source_digest`
  ONLY. P1 (`488c0093`) shipped `covered: bool` + `surface_ref:
  Vec<String>`; P3a (`d084e850`) removed both, with the type's own doc
  comment stating the replacement mechanism verbatim: "a future s16
  porting plugin owns it as a foreign-namespace overlay record, never
  as a field that core re-materializes here, because a plugin cannot
  safely own a field that core clobbers on every sync."
  `canon-cli::inventory`'s module doc (`inventory.rs:26-29`) repeats
  the same pointer at the call site that would otherwise have
  populated it.
- **The spec that already promises this mechanism.**
  `inventory-materialization`'s Requirement "Core Scenario index is
  general; covered/surface_ref are plugin-extensible, not core" (s15
  spec) already names the mechanism this change builds — "a porting
  plugin (the s16 plugin mechanism, modeled on the donor vocabulary project's / canon-vocab's
  capability manifest) MAY own a plugin-namespaced OVERLAY record ...
  which canon's foreign-namespace handling ... preserves without
  clobbering and a plugin-aware read-time projection joins onto the
  core index." This change is that requirement's MODIFIED delta —
  turning a forward reference into a working proof.
- **The precedent this mirrors, already built and tested.**
  `canon-vocab` (S10, `crates/canon-vocab/src/`) source-imports
  the donor's manifest crate + the donor's span crate as canon-owned modules (NOT a
  crate dependency — canon must stay standalone) and exposes exactly
  ONE resolution entry point, `resolve_snapshot(project_dir, profile)
  -> (CapabilitySnapshot, Vec<Diagnostic>)` (`resolve_snapshot.rs:
  44-94`), pure/total/never-panics, every consumer sharing the SAME
  snapshot rather than computing a partial view. `canon-vocab`'s own
  `plugin.yaml` lives at `canon/vocab/<id>/plugin.yaml`
  (`manifest/project.rs:4-6`) — a DIFFERENT surface (task-atom
  authoring vocabulary: directives + enums) from what s16 builds
  (ledger-record overlays), addressed explicitly in D2/R4 below.

## Architecture — s16 plugin layer (sits beside s15's core, never inside it)

```
   s15 core (three layers, one pipeline) -- UNCHANGED by this change
   L1 native corpus + producers   L2 review-parity gate   L3 flywheel

   ------------------------- the seam s15 built --------------------------
   kind=<core>/...  (12 closed RecordKind dirs)   kind=<ns>.<kind>/...  (plugin dirs)
   scan_corpus: scanned + validated                scan_corpus: skipped + foreign-namespace notice
                                                     ^
                                                     | canon-store: write_namespaced / scan_namespaced_kind
                                                     | (generalizes GitTier's digest-suffix/append-only algorithm)
   +----------------------------------------------------+---------------------------------------------+
   |  s16 plugin layer (canon-plugin, new crate + canon-cli wiring)                                    |
   |                                                                                                    |
   |  canon/plugins/<id>/plugin.yaml --resolve_plugin_snapshot--> PluginSnapshot                        |
   |       | (namespace, overlay kind, core_kind + join_key, fields)                                    |
   |       v                                                                                             |
   |  OverlaySource::produce_overlays (ONE per data-producing plugin, e.g. PortingOverlaySource)         |
   |       | validate_overlay_body (against the manifest's declared fields)                             |
   |       v                                                                                             |
   |  write_namespaced --> kind=<ns>.<kind>/{join_key}__{digest12}.json                                  |
   |                                                                                                      |
   |  READ: project_overlay(core records, scan_namespaced_kind(...), decl) --> projected view             |
   |        (pure, in-memory JOIN; core records on disk are NEVER rewritten; fail-soft when absent)       |
   +------------------------------------------------------------------------------------------------------+

   corpus-authoring scaffold (canon scenario new / canon feature new): a template generator feeding
   the SAME hand-authored .feature corpus s15's `canon inventory sync` already consumes -- sits
   upstream of L1, writes no ledger record.
```

s16 = pillar 5 (plugin extensibility), the sibling s15's own proposal
scoped out. s17 (integration/import of a foreign planning dialect) is
the remaining sibling, still untouched by this change.

## Decisions

- **D1 — Plugin kinds are namespaced strings; RecordKind's 12-closure
  stays frozen; s15's foreign-namespace seam IS the coexistence
  mechanism.** An overlay's on-disk kind is `<namespace>.<kind>` (e.g.
  `porting.coverage`) — a plain string, never a `RecordKind` variant.
  No canon-model change adds a 13th kind; the frozen `RecordKind::ALL`
  (12 members, structurally asserted) is the acceptance bar for this
  whole change. Coexistence needs ZERO new core logic because s15
  already built and tested the exact mechanism this relies on:
  `scan_corpus` treats any `kind=<x>/` whose `<x>` isn't one of the
  twelve as foreign-namespace, skip + report, never malformed. s16's
  ENTIRE contribution to "how does a core reader tolerate a plugin
  kind" is: nothing — the seam already does the job.
- **D2 — Manifest model mirrors canon-vocab/the donor vocabulary project: one resolution
  entry, a NEW crate, disambiguated from canon-vocab's OWN `plugin.yaml`
  surface.** A NEW crate, `canon-plugin`, houses the manifest schema/
  loader/`resolve_plugin_snapshot` — mirroring canon-vocab's
  architecture (plugin.yaml + one resolution entry + one snapshot
  type, fail-soft/total/never-panics) but NOT importing `canon-vocab`
  as a dependency: the two are DIFFERENT manifest content-domains
  (canon-vocab: directives/enums for task-atom authoring; canon-plugin:
  namespace/overlay declarations for ledger records), and conflating
  them would mean one crate now serves two unrelated vocabularies —
  exactly the "second, independently-computed view" both architectures
  explicitly forbid, just relocated to a worse place. Directory split
  is deliberate and disjoint: canon-vocab's plugins live at
  `canon/vocab/<id>/plugin.yaml`; s16's live at `canon/plugins/<id>/
  plugin.yaml`. A ledger-overlay manifest declares `id`, `namespace`,
  and `overlays: [{kind, attaches_to: {core_kind, join_key}, fields:
  [{name, type}]}]`; `type` reuses canon-vocab's `Type` STRUCTURAL
  shape (bare scalar / `{enum}` / `{list}`) by inspiration, not
  dependency — canon-plugin declares its own small `Type` mirroring
  `type_accepts`'s algorithm.
- **D3 — Overlays are read-time JOINED, never merged into core.**
  `project_overlay(core_records, overlay_records, decl)` is a PURE,
  in-memory function: core records are read-only inputs, the returned
  projected view is a NEW in-memory structure, and no core `Scenario`
  (or any other core kind) file is ever opened for writing by any
  plugin code path. This is the load-bearing invariant the whole
  change protects (assignment's own framing: "PROJECTS overlay fields
  onto the core view WITHOUT mutating the core record"). Enforced in
  TWO independent places, not one: (a) architecturally — `canon-plugin`
  never imports `canon-store`'s WRITE surface for core kinds, only
  `write_namespaced`/`scan_namespaced_kind`; (b) at write time —
  `write_namespaced` rejects a namespaced-kind string equal to any core
  `RecordKind::as_str()` value (R5 below), so even a misconfigured
  manifest cannot alias a core directory.
- **D4 — Overlay write validates against the manifest schema before
  write; reuses canon-store's append-only/content-digest algorithm via
  two narrow new primitives.** `RecordKind` is closed (D1), so the
  EXISTING typed `Tier::write`/`StoredRecord::kind() -> RecordKind`
  path CANNOT literally be reused for an overlay record — that API is
  contractually scoped to the twelve core kinds. Rather than inventing
  a second storage model, `GitTier` gains two NARROW additions that
  generalize the SAME algorithm `write`/`scan_kind_where` already
  implement (`partition.rs`'s natural-key + 12-hex content-digest
  suffix, append-only reject-on-existing-path) off an arbitrary
  namespaced STRING instead of the closed enum:
  `write_namespaced(namespaced_kind: &str, natural_key: &str, body:
  RawRecord) -> Result<WriteReceipt, StoreError>` and
  `scan_namespaced_kind(namespaced_kind: &str) -> Result<(Vec<(PathBuf,
  RawRecord)>, Vec<EvidenceViolation>), StoreError>`. A plugin-aware
  writer calls `validate_overlay_body(decl, body)` — checking every
  declared field present + structurally typed, no undeclared field
  accepted (a CLOSED field set per overlay kind, mirroring
  `RecordKind`'s own closure philosophy one level down) — BEFORE
  `write_namespaced`, never after. Overlay records compose their OWN
  `OverlayEnvelope { schema, kind: String, at, actor }` (reusing
  `canon_model::Actor`, which is not closed) rather than
  `canon_model::Envelope`, whose `kind: RecordKind` field is closed and
  therefore cannot represent a namespaced string.
- **D5 — The porting plugin is the acceptance vehicle; no core
  special-casing.** `porting` is exactly ONE `OverlaySource`
  implementation (`produce_overlays(spec_root) -> Vec<OverlayCandidate>`)
  registered behind the SAME generic dispatch shape s15's D7 already
  established for verdict adapters ("Register ONE records-source
  ArtifactAdapter PER verdict kind ... mirroring the Handoff
  handle-based shape") and S3 already established for session-ingest
  clients (`ClaudeCode`/`Codex`/`Hermes`). `canon-model`, `canon-store`
  (beyond D4's two generic primitives), and `canon-gate` gain ZERO
  code that names `porting`, `coverage`, or any other plugin-specific
  string — every reference to `porting` lives in exactly two places:
  `canon/plugins/porting/plugin.yaml` (data) and one
  `PortingOverlaySource` Rust type (canon-cli, the outermost wiring
  layer, same as where `ClaudeCode`/`Codex`/`Hermes` live). This proves
  the mechanism generalizes: a SECOND donor-porting plugin would add
  its own manifest + adapter the same way, touching nothing this
  change built for `porting` specifically.
- **D6 — Corpus scaffold is a template generator, no DSL.** `canon
  scenario new`/`canon feature new` emit a `.feature` file using the
  SAME tag-then-header shape `canon-fmt::gherkin::scan` already reads
  (s15 D4) — a `# canon:` provenance comment, a `@<area>.<surface>.
  <nn>` tag, a `Scenario:`/`Feature:` header. They write NO ledger
  record; the author still runs `canon fmt --check` then `canon
  inventory sync` afterward, exactly as for a hand-authored file. This
  mirrors s15 D10's boundary precisely ("Corpus AUTHORING stays OUT
  [of native producers] ... a `canon scenario new` writing both a stub
  AND a record = a second producer that drifts — the fragmentation s15
  kills") — s16's scaffold is a text-template convenience, never a
  second path to a `Scenario` record; `canon inventory sync` remains
  the ONLY thing that ever writes one.

## Risks

- **R1 clobbering core (highest, mitigated by D3 in two independent
  ways):** `project_overlay` is a pure, in-memory function (no write
  surface at all); `write_namespaced` REJECTS, loud, any namespaced-kind
  string equal to a core `RecordKind::as_str()` value, so a
  misconfigured manifest cannot alias `kind=scenario/` even by
  accident. Acceptance test (porting-plugin spec): a core `Scenario`
  file's bytes are identical before/after any `canon plugin sync`/
  `canon query --plugin` run.
- **R2 overlay staleness:** `porting`'s overlay source (the donor
  `inventory/` YAML) can drift out of sync with the core `.feature`
  corpus's own `source_digest` between a `canon inventory sync` and a
  `canon plugin sync porting` — the overlay is a SNAPSHOT at sync time,
  not a live join. Accepted (D5's own framing: the overlay is
  convenience, never authority); mitigated only by re-running `canon
  plugin sync` after a corpus change, exactly like re-running `canon
  inventory sync` itself.
- **R3 plugin-absent determinism:** `resolve_plugin_snapshot`/
  `project_overlay` never panic; an absent `canon/plugins/` directory,
  an absent manifest, or an absent overlay record for a join key all
  degrade to the unmodified core view — mirrors `resolve_snapshot`'s
  OWN "pure, total, never panics" contract verbatim (`resolve_snapshot.
  rs:41-43`).
- **R4 the two `plugin.yaml` surfaces' naming confusion:** DIFFERENT
  directories (`canon/vocab/<id>/` vs `canon/plugins/<id>/`),
  DIFFERENT schemas (directives/enums vs namespace/overlays), a
  distinct crate (`canon-plugin`, no dependency on `canon-vocab`); this
  document, the proposal, every spec, and the companion skill state the
  distinction explicitly so a future reader searching "plugin.yaml"
  finds both, disambiguated, rather than assuming one subsumes the
  other.
- **R5 namespace/kind collision with core:** `write_namespaced` rejects
  a namespaced-kind string equal to any `RecordKind::as_str()` value
  (e.g. a misconfigured manifest declaring `namespace: canon` + `kind:
  scenario` would otherwise silently alias `kind=scenario/`, corrupting
  the core directory) — checked BOTH at manifest resolution (D2, a
  load-time diagnostic) AND at write time (D4, defense in depth; a
  manifest could theoretically be edited between resolution and write).
- **R6 duplicate plugin/overlay-kind ids:** mirrors canon-vocab's
  `LoadError::DuplicateId` (`manifest/loader.rs`) — a second
  `canon/plugins/<id>/` with the same `id` drops the later package,
  reported as a diagnostic, never a silent overwrite; two DIFFERENT
  plugins declaring the SAME `<namespace>.<kind>` string is rejected at
  resolution (an ambiguous write target neither plugin should silently
  win).
- **R7 overlay schema drift:** a manifest's declared fields can change
  shape after overlay records are already on disk. `project_overlay`
  reads PER-RECORD, fail-soft — a record that no longer matches the
  CURRENT manifest schema is skipped + diagnosed, never aborting the
  whole projection for every other (still well-formed) record.
- **R8 coverage-authority confusion:** restated for s16 specifically
  (s15's own R4 already states "covered ≠ coverage" for the dropped
  core fields). `porting`'s overlay is READ BY NOTHING inside
  `canon-gate` — `canon-gate`'s source carries zero reference to
  `porting`, the `coverage` overlay kind, or `scan_namespaced_kind`. A
  future consumer that wires an overlay field into a gate DECISION is
  explicitly OUT of this change's scope (an Explicit non-goal); doing
  so would need its own reviewed change.

## Sequencing

- **P1 — registry/manifest:** `canon-plugin` crate scaffold; `plugin.
  yaml` schema/loader (`canon/plugins/<id>/`, sorted, duplicate-id
  drops the later package); `resolve_plugin_snapshot`; the overlay
  `Type` structural-shape checker (canon-vocab-inspired, not
  dependent); the namespace/`RecordKind`-collision + duplicate-overlay-
  identity checks at resolution time. No write, no read yet — this
  wave is pure manifest plumbing, matching s15's "identity-before-
  producers" discipline (a plugin's declared SHAPE must resolve
  correctly before anything writes or reads against it).
- **P2 — overlay write + validate (after P1):** `GitTier::
  write_namespaced`/`scan_namespaced_kind`; the RecordKind-collision
  rejection at write time (defense in depth alongside P1's
  resolution-time check); `validate_overlay_body`; `OverlayEnvelope`.
- **P3 — projection (after P2):** `project_overlay`; the fail-soft
  contract (absent plugin/manifest/overlay-record, malformed-record
  skip); `canon query --plugin <id>` wiring.
- **P4 — porting plugin (after P1-P3, the acceptance vehicle):**
  `canon/plugins/porting/plugin.yaml`; `PortingOverlaySource`
  (donor-inventory `covered_by` inversion); `canon plugin sync <id>`
  (generic dispatcher, `porting` the sole registered source);
  coverage-authority-untouched + core-never-mutated acceptance tests.
- **P5 — corpus-authoring scaffold (independent of P1-P4, may run in
  parallel):** `canon scenario new`/`canon feature new`; round-trip
  through `canon fmt --check` + `canon inventory sync`.
- **P6 — closure:** selftest fixture corpora (a synthetic
  `canon/plugins/<id>/plugin.yaml` + overlay records, registered in
  `canon selftest`); companion skill
  `canon/skills/canon-plugins/SKILL.md`; doc reconciliation (verify
  s15's `inventory-materialization`/`scenario-spine-layout` forward
  pointers still read true against what actually got built).

## Testing

- Manifest: a well-formed plugin resolves its declared shape exactly;
  a missing required field fails to load (excluded, not defaulted); an
  absent `canon/plugins/` dir resolves an empty snapshot, never a
  panic; a duplicate plugin id drops the later package with a
  diagnostic; a namespaced-kind or manifest `id` colliding with a core
  `RecordKind::as_str()` value is rejected at resolution.
- The two `plugin.yaml` surfaces: canon-vocab's loader ignores
  `canon/plugins/`; canon-plugin's loader ignores `canon/vocab/`; a
  ledger-overlay manifest misplaced under `canon/vocab/<id>/` fails
  canon-vocab's OWN load (missing `directives`/`enums`), never silently
  parsed as a valid authoring-vocabulary plugin.
- Overlay write: a byte-identical resubmission dedupes to the same
  path; a logically different overlay for the same join key appends
  (never overwrites) at a new path; a namespaced-kind colliding with a
  core `RecordKind` is rejected before touching disk; a body missing a
  declared field / carrying an undeclared field / a wrong-typed field
  is rejected before `write_namespaced` runs.
- Projection: a core record with a matching overlay projects the
  declared fields; a core record with NO overlay projects unmodified
  (no default/guessed value invented); an absent/uninstalled plugin
  degrades to the unmodified core view plus a diagnostic, never a
  process error; a malformed overlay record is skipped (diagnosed)
  without aborting projection for sibling records; the SAME core+
  overlay pair projects byte-identically across repeated runs; a core
  record's on-disk file is byte-identical before/after any projection
  read.
- porting plugin: a scenario in ≥1 `covered_by` list projects
  `covered: true` + the union of matching surface_ref keys; a scenario
  in none projects `covered: false` + an empty list; `canon plugin
  sync porting` run twice over an unchanged inventory writes zero new
  overlay records (logical idempotence, mirroring `canon inventory
  sync`'s own D5); `canon gate check`'s `uncovered-cell` verdict (and
  every other gate verdict) is byte-identical with and without a
  `canon plugin sync porting` run having happened; `canon-gate`'s
  source carries no reference to `porting`/`coverage`/
  `scan_namespaced_kind`.
- corpus scaffold: a generated `.feature` entry round-trips through
  `canon fmt --check` clean and materializes via `canon inventory
  sync` exactly as a hand-authored entry would; the scaffold command
  itself writes zero ledger records; a duplicate tag is rejected loud,
  never silently appended twice; `canon feature new` never overwrites
  an existing file.
- selftest: plugin-manifest/overlay fixture corpora with a rebindable
  project directory (mirroring s15's rebindable-roots `SyncCtx`
  pattern), registered in `canon selftest`.
