# Tasks — s16 plugin extensibility

Sequencing follows design.md: **P1 (manifest identity) lands strictly
before P2/P3 (write/read)** — a plugin's declared shape must resolve
correctly before anything writes or reads against it, mirroring s15's
own identity-before-producers discipline. P4 (porting) depends on
P1-P3; P5 (corpus scaffold) is independent and may run in parallel.

## 1. plugin manifest + registry (P1)

- [ ] 1.1 New crate `canon-plugin`: `plugin.yaml` schema (`id`,
      `namespace`, `overlays: [{kind, attaches_to: {core_kind,
      join_key}, fields: [{name, type}]}]`), loader scanning
      `canon/plugins/<id>/plugin.yaml` (mirrors canon-vocab's
      `load_plugins_dir` shape: sorted, per-package, duplicate-id drops
      the later package as a diagnostic, never a silent overwrite).
- [ ] 1.2 `resolve_plugin_snapshot(project_dir) -> (PluginSnapshot,
      Vec<Diagnostic>)` — the ONE resolution entry point; pure, total,
      never panics (mirrors `canon_vocab::resolve_snapshot`'s
      contract). No second, independently-computed plugin view exists
      anywhere in the crate or its consumers.
- [ ] 1.3 Overlay field `Type` — the same bare-scalar/`{enum}`/`{list}`
      structural shape `canon-vocab::manifest::types::Type` validates,
      reused by INSPIRATION not import (own small type in
      `canon-plugin`; no `canon-vocab` crate dependency, keeping the
      two surfaces genuinely separate per design D2/R4).
- [ ] 1.4 Reject at resolution: a manifest's `namespace` or any
      overlay `kind` failing the kebab-token grammar
      (`[a-z0-9]+(-[a-z0-9]+)*`); a manifest's `<namespace>.<kind>`
      string equal to any `RecordKind::as_str()` value; two plugins
      declaring the same `<namespace>.<kind>`; an `attaches_to.
      core_kind` naming anything other than `scenario` (s16 supports
      `core_kind: scenario` only — generic projection over other core
      kinds is explicit FUTURE work).
- [ ] 1.5 canon-plugin unit tests: manifest round-trip; a missing
      required manifest field fails to load (excluded, not defaulted);
      duplicate-id drop; RecordKind-collision rejection; an absent
      `canon/plugins/` dir degrades to an empty, valid snapshot (never
      a panic).

## 2. overlay write + validation (P2 — after P1)

- [ ] 2.1 `GitTier::write_namespaced(namespaced_kind: &str, natural_key:
      &str, body: RawRecord) -> Result<WriteReceipt, StoreError>` —
      generalizes the existing digest-suffix (`partition.rs`) +
      append-only reject-on-duplicate-path algorithm off a string
      instead of `RecordKind`; path `kind={namespaced_kind}/
      {natural_key}__{digest12}.json`. `namespaced_kind` and
      `natural_key` are checked against the path-safety grammar BEFORE
      this path is constructed; `natural_key` is the `__`-joined
      join-key field values the caller derives from `body` immediately
      after 2.4's validation succeeds — never an independently
      supplied string — and `write_namespaced` rejects a `natural_key`
      that doesn't match this derivation.
- [ ] 2.2 `GitTier::scan_namespaced_kind(namespaced_kind: &str) ->
      Result<(Vec<(PathBuf, RawRecord)>, Vec<EvidenceViolation>),
      StoreError>` — the read-side twin, mirroring `scan_kind_where`'s
      walk.
- [ ] 2.3 `write_namespaced`/`scan_namespaced_kind` REJECT a
      `namespaced_kind` equal to any `RecordKind::as_str()` value
      (defense in depth alongside 1.4's resolution-time check), a
      `namespaced_kind`/`natural_key` failing the path-safety grammar
      (non-kebab token; `natural_key` containing `/`, `\`, `..`, a
      leading `.`, or an absolute path), or a `natural_key` that
      doesn't equal the `__`-joined join-key field values already
      present in `body`.
- [ ] 2.4 `validate_overlay_body(decl: &OverlayDecl, body: &RawRecord)
      -> Result<(), Vec<Diagnostic>>` — checks three field groups all
      present + structurally typed: the `OverlayEnvelope`
      (`schema`/`kind`/`at`/`actor`), the REQUIRED join-key field(s)
      named by `decl.attaches_to.join_key`, and `decl`'s declared
      `fields` (1.3's `type_accepts`-style check); rejects any field
      outside the union of join-key-fields and declared-fields (a
      closed set, but a join-key field is never mistaken for
      undeclared). A plugin-aware writer calls this BEFORE deriving
      `natural_key` from the validated join-key fields and calling
      `write_namespaced`, never after.
- [ ] 2.5 `OverlayEnvelope { schema, kind: String, at, actor }`
      (canon-plugin's own envelope type — `canon_model::Envelope.kind`
      is `RecordKind`-typed and closed, so overlay records CANNOT
      compose it) flattened into every overlay record body.
- [ ] 2.6 canon-store/canon-plugin tests: a write round-trips through
      `scan_namespaced_kind`; a byte-identical resubmission dedupes to
      the same path; a logically different overlay for the same join
      key appends at a new path, never overwriting; a namespaced-kind
      colliding with a core `RecordKind` is rejected at write time; a
      `namespaced_kind` or `natural_key` containing a path separator,
      `..`, or a leading `.` is rejected loud before any path is
      constructed; a `natural_key` disagreeing with the body's own
      join-key field values is rejected loud; a body missing a
      declared field / carrying a field outside (join-key ∪ declared)
      / a wrong-typed field is rejected loud, never silently written.

## 3. read-time projection (P3 — after P2)

- [ ] 3.1 `project_overlay(core: &[Scenario], overlay_raw: &[RawRecord],
      decl: &OverlayDecl) -> BTreeMap<(ProjectId, ScenarioId),
      serde_json::Map<String, Value>>` — pure, fold-latest-by-
      `(join_key, at)` (reusing `canon_store::fold_latest_by_key`'s
      pattern); core records are read-only inputs, NEVER rewritten.
      s16 concretely projects onto `Scenario` only (`core_kind:
      scenario`, enforced at 1.4's manifest resolution); a generic
      `project_overlay` over other core kinds is explicit FUTURE work,
      out of scope for this change.
- [ ] 3.2 Fail-soft: an absent plugin/manifest, an absent overlay
      record for a given join key, or a malformed overlay record
      (fails 2.4's schema against the CURRENT manifest) all degrade to
      the unmodified core view for that record — never a panic, never
      an aborted whole-projection.
- [ ] 3.3 `canon query --kind <k> --plugin <id> [--json]` (extends S2's
      existing `canon query`, `main.rs::Command::Query`): resolves the
      plugin snapshot, projects the declared overlay fields onto each
      queried record before printing/emitting.
- [ ] 3.4 Tests: a core record with a matching overlay record projects
      the declared fields; a core record with NO overlay record
      projects unmodified; a malformed overlay record is skipped +
      diagnosed, sibling records still project; a core record's
      on-disk file is byte-identical before/after a projection read;
      `canon query` without `--plugin` is byte-identical to its
      pre-s16 output (no regression).

## 4. porting plugin (P4 — after P1-P3, the acceptance vehicle)

- [x] 4.1 `canon/plugins/porting/plugin.yaml`: `id: porting`,
      `namespace: porting`, one overlay `kind: coverage` attached to
      `core_kind: scenario`, `join_key: [project_id, scenario_id]`,
      fields `covered: bool` + `surface_ref: {list: string}` — the
      exact two fields s15 P1 shipped and P3a removed from core
      (`records.rs`, commit `d084e850`).
- [x] 4.2 `PortingOverlaySource` (an `OverlaySource` impl, canon-cli):
      reads a root's `inventory/**/*.yaml`
      (`canon_model::family::inventory::{InventoryFile,
      InventoryEntry}`, the SAME S11-validated files `canon-fmt::check`
      already covers as ordinary corpus hygiene) and, for every
      `(project_id, scenario_id)` `canon inventory sync` would index
      from that root's `.feature` corpus, emits one overlay candidate:
      `covered = scenario_id` appears in ANY `InventoryEntry.
      covered_by`; `surface_ref` = every inventory-entry key whose
      `covered_by` contains it (empty when uncovered).
- [x] 4.3 `canon plugin sync <plugin-id> [--spec-root <dir>]`
      (`Command::Plugin { action: PluginCommand::Sync }`): a GENERIC
      dispatcher matching `plugin-id` against registered
      `OverlaySource` impls (today: exactly `porting`) — validates each
      candidate (2.4) then writes it (2.1); the dispatcher itself never
      special-cases `porting`.
- [x] 4.4 Tests: a covered scenario projects `covered: true` + its
      surface_ref list; an uncovered scenario projects `covered:
      false` + an empty list; `canon plugin sync porting` run twice
      over an unchanged inventory writes zero new overlay records
      (logical idempotence, mirroring `canon inventory sync`'s own
      D5); `canon gate check`'s `uncovered-cell` verdict (and every
      other verdict) is byte-identical with and without a `canon
      plugin sync porting` run; a core `Scenario` record's on-disk
      bytes are byte-identical before/after a `canon plugin sync
      porting` + `canon query --plugin porting` round-trip; canon-gate's
      source carries no reference to the plugin-specific names
      `porting`/`porting.coverage`/`scan_namespaced_kind` (its own core
      `coverage` module is untouched).

## 5. corpus-authoring scaffold (P5 — independent, may run in parallel)

- [x] 5.1 `canon scenario new <area>.<surface>.<nn> --title <label>
      --feature <path>`: appends (or creates) a `.feature` file with a
      `# canon:` provenance comment + a `@<area>.<surface>.<nn>`-tagged
      `Scenario: <label>` header + a placeholder step block — the
      exact tag-then-header shape `canon-fmt::gherkin::scan` already
      reads (s15 D4); writes NO ledger record.
- [x] 5.2 `canon feature new <area>.<surface> --title <label>`: creates
      a NEW `.feature` file (provenance comment + header, zero
      scenarios) for a not-yet-started surface; fails loud rather than
      overwriting an existing file.
- [x] 5.3 Tests: a generated `.feature` round-trips through `canon fmt
      --check` clean; `canon inventory sync` materializes exactly the
      tagged scenario, same as a hand-authored entry; `canon scenario
      new` against an EXISTING tag is rejected loud (never a silent
      duplicate); `canon feature new` against an existing file is
      rejected loud, file unchanged.

## 6. closure (P6)

- [x] 6.1 Selftest fixture corpora (a synthetic `canon/plugins/<id>/
      plugin.yaml` + overlay records, mirroring s15's rebindable-roots
      pattern) registered in `canon selftest`.
- [x] 6.2 Companion skill `canon/skills/canon-plugins/SKILL.md` (author
      a manifest → validate an overlay write → project a read →
      `porting` worked example); materialize via `canon skills
      install` + install-lock bump.
- [x] 6.3 Reconcile design docs / capability skills touched — verify
      s15's `inventory-materialization`/`scenario-spine-layout` forward
      pointers to s16 still read true against what actually got built,
      not just what was promised; `bunx openspec validate --strict`
      green for this change.

## 7. Verification

- [x] 7.1 `cargo build --workspace` + `cargo clippy --workspace
      --all-targets -- -D warnings` + `cargo test --workspace
      --no-fail-fast` (bare, no pipe masking) all green.
- [x] 7.2 `bunx openspec validate --strict s16-plugin-extensibility`
      green.
- [x] 7.3 `canon selftest` all suites green, including the new
      plugin-manifest/overlay fixtures.
