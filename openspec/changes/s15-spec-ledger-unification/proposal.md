## Why

canon is a SELF-COMPLETE harness: `install canon` and the whole loop works
natively; external tools are ADAPTED (integration), never required. s15 delivers
the CORE that makes "native" true — pillars 1 (native ledger management) and 2
(self-improvement over the ledger + reviews). Two siblings follow in sequence:
**s16** adds the plugin system that extends the ledger/inventory structure (e.g.
a porting plugin overlaying `covered`/`surface_ref` onto the general Scenario
index from a donor inventory, via a plugin-namespaced overlay record);
**s17** adds the integration layer (openspec / superpowers / external-ledger
IMPORT — a secondary connector, like Jira importing GitHub issues). Neither is in
s15; the s15 pipeline starts at the hand-authored, S11-validated corpus, not at
an external plan.

Not the gap: canon is already self-terminating (the vendored upstream launcher project is an attributed port,
the openspec CLI is a dev-only validator the Rust never invokes), and the tight
regime's FORMAT AUTHORITY already exists — S11 `canon fmt` validates a structured
feature/inventory/ledger/divergence/policy corpus against `canon-model::family::*`
schemas + Hive layout; S1 models the records on the `scenario_id`
(`<area>.<surface>.<nn>`) join spine; S2 stores them Hive-partitioned with
content-derived keys. Governance is structured YAML/JSONL/Gherkin + schema +
layout — NOT markdown.

**The gap: canon fragmented the donor's ONE parity pipeline into THREE
non-communicating implementations** — governance LOOSER than the donor's single
`parity.py` even though each piece is better-typed:

1. **S1 native records are unauthored.** `Scenario`/`Review`/`Divergence` are
   defined but have ZERO producers — every construction site is a test/fixture.
2. **S11 validation is read-only and terminal.** `canon fmt` validates a corpus
   but authors nothing (the `canon migrate` writer was killed by operator
   directive 2026-07-10), so a validated corpus never reaches the ledger.
3. **S6 ReasoningBank is fed only by an S4 bypass.** The artifact adapters parse
   the donor's RAW pre-migration JSON/JSONL and fold verdicts into trajectories,
   bypassing BOTH S1 records AND S11 — the flywheel runs over an external mirror.

Consequences: the S5 gate operates on bare `EvidenceRecord`s with trust-ladder/
staleness fields living as canon-gate-private companions on raw re-scans (its own
documented INTERFACE REQUESTS to S1, `lib.rs:32-45`); `Divergence` carries
`run_seq`/`round`/`status` but has no fold-to-current-state and only `{Open,
Resolved}`; and `records.rs:333-338` already assigns the divergence fold to
canon-gate, which has zero divergence code. s15 recovers this already-scoped,
dropped work and unifies the three into one native pipeline, implementing the
donor consumer repo's parity patterns FAITHFULLY (per
the donor parity-harness audit), never a naive subset.

## What Changes

- **`specs.roots[]` config.** A NEW `canon.yaml` shape: a list of `{ id, root }`
  (a monorepo declares one per nested project; `root` defaults to `specs`; the
  default when unset is a single `{ id: root, root: specs }`). The `id` is a
  STABLE LITERAL — never the checkout directory name (that would split identity
  across clones). Only the fail-loud-on-malformed / default-on-missing SEMANTICS
  reuse `canon-cli::ingest::IngestSourceConfig::load`; the named-root-list shape
  is new. (Drive-by: `load_artifact_source_config`'s fail-SOFT load is the
  anti-pattern NOT to copy.)
- **`canon inventory sync`.** For each configured root: run S11 validation
  (`canon-fmt::check`) — ANY violation aborts the whole root (malformed = no
  evidence, never a partial sync); scan `features/` via canon-fmt's existing
  line-scan (surfacing each `@<area>.<surface>.<nn>` tag↔header + the header
  label as title — retention, not a new parser); and upsert one `Scenario`
  ledger-INDEX record per `(project_id, scenario_id)` — carrying `title` +
  `source_digest`, derived from the `.feature` corpus ALONE (no `upstream`/`covered_by`
  read; that donor-inventory→coverage mapping is a PORTING-PLUGIN concern, s16,
  never core) — through canon-store's append-only write. Logically idempotent:
  fold latest per key and no-op when `source_digest` (+ the derived `title`) match.
- **Native VERDICT-record producers.** `canon review add` (writes a
  `canon_model::Review`, actor-attributed) and `canon divergence
  {stage,promote,resolve,defer}` (staging → `promote` assigns the monotonic
  `run_seq`). WITHOUT these, `Review`/`Divergence` have no native producer and the
  gate's review index reads an eternally-empty set (every cell
  `unreviewed-promotion` forever). Corpus AUTHORING stays OUT (the family docs are
  authoritative; `sync` indexes them) — only VERDICT records, which have no doc
  counterpart, get native producers.
- **Extend staging→promote to `RecordKind::Divergence`.** `canon-gate::promote`
  is hardcoded to `EvidenceRecord`; the fold has no `run_seq` assigner without
  this. Partition axis `(project_id, role, surface)`; refusals never consume a
  `run_seq`.
- **Complete the `Divergence` fold + status machine.** `DivergenceStatus +=
  StillDivergent, Deferred{reason, expiry}` (additive; `resolved-invalid` is
  fold-DERIVED as a separate `FoldedState` output, never a persisted variant); a
  PURE `fold_to_current_state(records, live_bindings, as_of)` in canon-model
  (`run_seq: TotalOrder` sole-primary, `round` tiebreak-only never `Ord`, the
  resolved-binding re-check taken as INPUT since canon-model can't depend on
  canon-store — no TOCTOU). Consumed by the S9 divergence burn-down.
- **Close the canon-gate → canon-model field interface.** Move `lifecycle`/
  `flagged`/`evidence_sha`/`surface_ref` + `run_seq` (a 5th, untyped companion key
  `promote.rs` stamps today) onto `EvidenceRecord` as OPTIONAL typed fields; the
  gate then reads them off native records and deletes its companion types + raw
  re-scans. Read is THREE-way — an ABSENT field → its documented safe default (so
  old promoted records stay legitimate, never malformed), a PRESENT well-formed
  field → typed, a PRESENT malformed field → violation. The `Option<T>` default
  fires ONLY for a missing key, NEVER collapsing a present-malformed value into
  absent (that would let a corrupt tag dodge `flagged`/`unreviewed-promotion`).
- **`project_id` composite identity.** A `ProjectId` newtype (`[a-z0-9][a-z0-9-]*`,
  no `_`, keeping `__` unambiguous); a REQUIRED `project_id` field on `Scenario`/
  `Review`/`Divergence` (clean cutover — zero real producers exist, so only
  fixtures migrate; an Option branch would guard records that never existed) and
  an OPTIONAL one on `EvidenceRecord` (real records may exist via promote). S2
  `resolve_partition` prefixes the natural_key: `<project_id>__<scenario_id>`
  (Scenario), `…__<pin>` (Review), `…__<run_seq>__<round>` (Divergence). No new
  Hive `project=` dimension; always prefix, even single-project.
- **Native verdict records-source adapters → S6.** ONE records-source
  `ArtifactAdapter` PER verdict kind (`Review`, `Divergence`) — each a
  single-`RecordKind` registry entry + `record_kind_for_records_adapter` arm
  (matching the existing one-adapter-one-kind dispatch), mirroring the `Handoff`
  handle-based shape — folding native verdicts into `Trajectory` via
  `store_trajectory`. `Scenario` is NOT a source (a no-verdict index `sync`
  materializes). Because `Trajectory` is `regime_key`-only (no `scenario_id`),
  the adapters derive regime inputs like `attach_regime_key` (role ←
  `actor.role`, area ← `scenario_id.area()`, repo ← root). A new
  `artifacts.native_records: bool` switch enables them, XOR-exclusive with the
  raw-artifact path fields (config-validated) so the two paths never
  double-count; the switch scopes ONLY the verdict adapters — the existing
  `Handoff` Records adapter is unaffected.
- **Cleanup.** Delete `canon-store::git_tier::migrate_write` (dead code, zero
  non-test callers — the killed writer's last remnant; the append-only path is
  now the ONLY sanctioned write). Add the forward-compat reader rule: an
  unrecognized `kind=<x>/` directory is skipped + reported (foreign namespace),
  never classified as malformed core evidence.
- **Companion skill** `canon/skills/canon-inventory/SKILL.md` — the unified loop
  (author corpus → `sync` → `review`/`gate` → flywheel).

## Capabilities

### New Capabilities

- `inventory-materialization`: `canon.yaml specs.roots[]` config (stable-literal
  `id`, fail-loud, monorepo multi-root) + `canon inventory sync` validate →
  materialize GENERAL `Scenario` ledger-index records (`title` + `source_digest`,
  derived from the `.feature` corpus alone — no `upstream`/`covered_by` read),
  logically idempotent, fail-loud whole-root. `covered`/`surface_ref` are NOT
  core Scenario fields; they are plugin-extensible (a porting plugin's overlay,
  s16). Coverage stays the gate's `uncovered-cell` authority.
- `scenario-project-identity`: `ProjectId` newtype; required `project_id` on
  `Scenario`/`Review`/`Divergence` + optional on `EvidenceRecord`; composite
  `<project_id>__…` natural keys; clean-cutover fixture migration.
- `native-verdict-lifecycle`: `canon review add` (native `Review` producer) +
  `canon divergence {stage,promote,resolve,defer}` (staging → `promote` assigns
  the monotonic `run_seq`, extended to `RecordKind::Divergence`); full
  `DivergenceStatus` + pure `fold_to_current_state` (derived `ResolvedInvalid`,
  `run_seq` primary, `round` tiebreak-only, no-TOCTOU re-check).
- `native-record-flywheel`: per-kind (`Review`/`Divergence`) records-source
  adapters feeding native verdicts into S6 via `store_trajectory` (regime
  derived like `attach_regime_key`; `Scenario` is a no-verdict index, not a
  source); a `native_records` config switch, XOR-exclusive with S4 raw paths,
  scoped to the verdict adapters (`Handoff` unaffected).
- `spec-ledger-selftest`: fixture corpora with two-sided exact-set oracles
  (missing AND extra), rebindable-roots `SyncCtx`, a frozen-incident slot.

- `gate-native-record-fields`: the gate reads `lifecycle`/`flagged`/
  `evidence_sha`/`surface_ref`/`run_seq` natively off canon-model `EvidenceRecord`
  (not raw-JSON companions), THREE-way — absent → documented default (old records
  stay legitimate), present well-formed → typed, present-malformed → violation
  (never a serde-default collapsing present-malformed→absent); the human-only
  `flagged` ratchet unchanged; the review index matches by `(project_id,
  scenario_id)` (a review for one project never satisfies another project's
  same-`scenario_id` evidence; `None` legacy evidence falls back to bare
  `scenario_id`).
- `scenario-spine-layout`: `Scenario`/`Review`/`Divergence` use the composite
  `<project_id>__…` natural key; an unknown `kind=<x>/` dir is skipped+reported as
  foreign-namespace; `migrate_write` (the overwrite exception) removed.

### Modified Capabilities

_None — this repo's specs are delta-only (the `openspec/specs` baseline is
empty), so behavior changes to S5's gate and S2's layout enforcement are
expressed as the ADDED capabilities `gate-native-record-fields` /
`scenario-spine-layout` above, matching every existing change's ADDED-only
pattern._

## Impact

- `canon-model`: `ProjectId`/`TotalOrder`/`SpecDigest` newtypes (`ids.rs`);
  `project_id` on the scenario spine + `EvidenceRecord`; `Scenario` index fields;
  `DivergenceStatus` +2 variants; `EvidenceRecord` +5 OPTIONAL native fields
  (three-way read: absent→default, present well-formed→typed, present-malformed→violation);
  pure `fold_to_current_state`. All additive to WIRE format except the required
  `project_id` (fixture-only migration).
- `canon-store`: `resolve_partition` composite keys; a hoisted generic
  `fold_latest_by_key` (reused by sync / fold / gate staleness / flywheel);
  unknown-kind skip+report; `migrate_write` deleted.
- `canon-fmt`: gherkin-scan surfacing (tag↔header + title) + a `source_digest`
  helper — no new parser.
- `canon-gate`: native field reads (three-way); project-aware review index;
  `promote` extended to `Divergence`; companion types deleted.
- `canon-cli`: `inventory.rs` + `review`/`divergence` arms + `Command` additions;
  records-source adapter wiring + XOR config check.
- `canon-learn`/`canon-ingest`: records-source adapter alongside the artifact
  adapters.
- New companion skill + install-lock bump.
- **Record-kind stability.** `RecordKind` is canon's CORE vocabulary and is closed
  in this change: s15 adds no kind, and a core kind stays a reviewed, breaking
  `canon-model` change — never a `kind: String` escape hatch. Closure is scoped to
  the core namespace: s16 plugin kinds live in a separate namespaced registry and
  are never `RecordKind` variants; s15 readers treat an unknown `kind=` dir as
  foreign (skip+report), so s16 is not a breaking change to s15 consumers.

### Explicit non-goals

- External plan/corpus import of any dialect (openspec dirs, markdown/superpowers
  checklists, donor-JSON re-homing) → s17; the S4 donor-JSON adapters are
  reclassified as s17 connectors, untouched here.
- Plugin-defined kinds/projections (e.g. a porting plugin overlaying
  `covered`/`surface_ref` onto scenarios) and corpus-authoring scaffolds
  (`canon scenario new`) → s16.
- Migrating canon's own `openspec/changes/**` off the dev-tool validator.
- A Gherkin parser beyond the existing line-scan subset; a spec DSL.
