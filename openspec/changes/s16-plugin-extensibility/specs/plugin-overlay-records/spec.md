## ADDED Requirements

### Requirement: Overlay records live under a namespaced kind=<namespace>.<kind>/ directory, never a core RecordKind directory
A plugin-aware write SHALL place an overlay record under
`kind=<namespace>.<kind>/`, where `<namespace>.<kind>` is the plugin's
declared overlay identity — a string `RecordKind::ALL` never contains.
`<namespace>` and each overlay `<kind>` SHALL each be a kebab token
matching `[a-z0-9]+(-[a-z0-9]+)*`, so `<namespace>.<kind>` is exactly
two dot-joined kebab tokens — no `/`, `..`, or other path separator is
ever constructible from a conforming pair. `write_namespaced`'s
`natural_key` argument SHALL be a single safe path component, REJECTED
if it contains `/`, `\`, `..`, or begins with `.`, or is an absolute
path — checked BEFORE any filesystem path is constructed.
`write_namespaced` SHALL REJECT, loud, any attempt whose
`<namespace>.<kind>` string fails this grammar, equals a core
`RecordKind::as_str()` value, or whose `natural_key` fails the
path-safety check above; core's `RecordKind` closure SHALL remain
exactly twelve members, unaffected by any number of installed plugins.

#### Scenario: A plugin's overlay record is written under its own namespaced directory
- **WHEN** the `porting` plugin's writer calls
  `write_namespaced("porting.coverage", "root__world.hotdeal.01",
  body)`
- **THEN** the record lands at `kind=porting.coverage/
  root__world.hotdeal.01__<digest12>.json`, a path canon-store's core
  scan treats as foreign-namespace (per the `scenario-spine-layout`
  MODIFIED requirement)

#### Scenario: A namespaced-kind string colliding with a core RecordKind is rejected
- **WHEN** a misconfigured writer calls `write_namespaced("scenario",
  ...)` — a string equal to `RecordKind::Scenario.as_str()`
- **THEN** the write is REJECTED before touching disk, and no file is
  written under the core `kind=scenario/` directory

#### Scenario: A namespaced_kind or natural_key violating the path-safety grammar is rejected
- **WHEN** a writer calls `write_namespaced("porting/coverage", ...)`
  (a `/`-separated `namespaced_kind`) or `write_namespaced(
  "porting.coverage", "../../etc/passwd", body)` (a `natural_key`
  containing `..` and `/`)
- **THEN** the write is REJECTED before any filesystem path is
  constructed and before touching disk

### Requirement: An overlay write reuses canon-store's existing append-only, content-digest-suffixed algorithm
`write_namespaced` SHALL generalize the SAME algorithm `GitTier::write`
already uses for the twelve core kinds — a human-legible natural key
(the join-key value(s), `__`-joined) plus a 12-hex content-digest
suffix — so a byte-identical resubmission resolves to the SAME path
(deduped) while a logically different overlay record sharing the same
join key resolves to a DIFFERENT path (a genuine append). A write
targeting an EXISTING path SHALL be rejected — overlay records are
unconditionally append-only, exactly like every core kind. The natural
key SHALL be DERIVED by the plugin-aware writer from the validated
overlay body's own join-key field values immediately after
`validate_overlay_body` succeeds — NEVER an independently computed or
caller-supplied string free to diverge from the body it accompanies,
mirroring canon-store core's `resolve_partition`, which derives a
record's on-disk path FROM the record's own fields, never from an
out-of-band argument. `write_namespaced` SHALL REJECT, loud, any write
whose `natural_key` argument does not equal this join-key derivation —
a filename and its body's join-key fields can never disagree.

#### Scenario: A byte-identical overlay resubmission dedupes
- **WHEN** the same overlay body is written twice via
  `write_namespaced` for the same join key
- **THEN** the second write resolves to the identical path as the
  first and is reported as deduped, never a rejected-duplicate error
  and never a second distinct file

#### Scenario: A logically different overlay for the same join key appends, never overwrites
- **WHEN** two DIFFERENT overlay bodies (e.g. `porting`'s coverage
  flips from `false` to `true`) are written for the SAME `(project_id,
  scenario_id)` join key
- **THEN** both land at DIFFERENT paths under `kind=porting.coverage/`
  — the second write is a genuine append, and the first record's file
  is never rewritten or deleted

#### Scenario: An overlay whose body join-key fields disagree with its target natural_key is rejected
- **WHEN** a write's `natural_key` argument does not equal the
  `__`-joined join-key field values already present in the (validated)
  `body` — e.g. `natural_key = "root__world.hotdeal.01"` while `body`
  carries `scenario_id: "world.hotdeal.99"`
- **THEN** the write is REJECTED before touching disk — a filename and
  its body's join-key content can never diverge

### Requirement: A plugin-aware writer validates an overlay body against the manifest's declared schema before write
A plugin-aware writer SHALL treat a candidate overlay body as exactly
three field groups: (a) the `OverlayEnvelope` (`schema`/`kind`/`at`/
`actor`), (b) the REQUIRED join-key field(s) named by its
`OverlayDecl`'s `attaches_to.join_key` (e.g. `project_id`,
`scenario_id` — the fields `project_overlay` needs to join an overlay
record back to its core record), and (c) its `OverlayDecl`'s declared
`fields`. `validate_overlay_body` SHALL check all three groups are
present and structurally typed, and SHALL reject any field outside the
union of (a), (b), and (c) — an envelope or join-key field is NEVER
mistaken for an undeclared field. This validation SHALL run BEFORE calling
`write_namespaced`; a body that fails validation SHALL NEVER reach
disk.

#### Scenario: A well-formed overlay body passes validation and is written
- **WHEN** a candidate `porting.coverage` body supplies
  `project_id`/`scenario_id` (the declared join-key fields) alongside
  `covered: bool` and `surface_ref: [string]` exactly as `porting`'s
  manifest declares
- **THEN** validation succeeds — the join-key fields are recognized as
  group (b), not rejected as undeclared — and the writer proceeds to
  `write_namespaced`

#### Scenario: A body missing a declared field is rejected before write
- **WHEN** a candidate body omits `surface_ref` entirely
- **THEN** validation fails with a diagnostic naming the missing
  field, and `write_namespaced` is never called for that candidate

#### Scenario: A body carrying an undeclared field is rejected before write
- **WHEN** a candidate body includes a field outside the union of its
  envelope fields (a), join-key fields (b), and manifest-declared fields
  (c) — e.g. a stray `notes: "..."` that is none of these
- **THEN** validation fails — the overlay kind's field set is closed
  to (a) ∪ (b) ∪ (c), exactly as `RecordKind`'s own twelve-kind closure is
  closed at the core level
