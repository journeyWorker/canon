# Design — s15 spec-ledger unification

## Current state (accurate baseline, verified)

- **Self-terminating; format authority exists.** The vendored upstream launcher project = attributed port;
  openspec CLI = dev-only validator the Rust never invokes. S11 `canon fmt`
  validates the structured `<root>/{features,inventory,ledger,divergences,
  policy.yaml}` corpus (`canon-model::family::*` + Hive layout); S1 models the
  records on the `scenario_id` join spine; S2 partitions with content-derived
  keys (`partition.rs` recomputes `area` from `scenario_id`, never directory).
- **Three non-communicating pipelines (the gap):** (1) S1 `Scenario`/`Review`/
  `Divergence` have ZERO producers (fixtures only); (2) S11 `canon fmt` is
  read-only — `canon migrate` (writer) was killed 2026-07-10, so a validated
  corpus never reaches the ledger; (3) S4 artifact adapters feed S6 from the
  donor's RAW JSON/JSONL, bypassing S1 + S11. Governance is looser than the
  donor's single `parity.py`.
- **Already-scoped, dropped work this recovers:** `records.rs:333-338` assigns
  the divergence fold to canon-gate (which has no divergence code);
  `canon-gate/src/lib.rs:32-45` is an explicit "INTERFACE REQUESTS to S1" block
  for `lifecycle`/`flagged`/`evidence_sha`/`surface_ref`; `promote.rs:216` stamps
  `run_seq` onto `EvidenceRecord` as an untyped raw-JSON companion key.

## Architecture — s15 core (three layers, one pipeline)

```
   (s16 plugin: extend ledger/inventory structure — separate change)
   (s17 integration: import openspec/superpowers/external-ledger — separate change)

L1 authoritative corpus + native producers   specs.roots[] (canon.yaml)
   │ Gherkin features/ + YAML inventory/ ── S11 validate ──▶ family docs AUTHORITATIVE
   │ canon inventory sync   ── materialize ──▶ Scenario index record (join spine, Hive)
   │ canon review add       ── attest ──▶ Review record
   │ canon divergence {stage,promote,resolve,defer} ── promote assigns run_seq ──▶ Divergence record
   ▼
L2 review-parity pipeline (S5)    gate reads NATIVE Scenario/Review/Divergence fields
   │ trust ladder · staleness · divergence fold_to_current_state
   ▼
L3 ReasoningBank flywheel (S6)    native records-source adapter → Trajectory → StrategyItem
   └───────────── retrieve guidance ──▶ future authoring
```

s15 = pillars 1 (native ledger management) + 2 (self-improvement over ledger +
reviews). Handoff (3) + per-role evolution (4) are existing pillars this joins
with; plugin extensibility (5) is s16; external import is s17.

## Decisions

- **D1 — Family docs vs ledger records; canonical-kind closure.** `Feature`/
  `Inventory`/`Ledger`/`Policy` are `family` DOCUMENTS under a spec root;
  `Scenario`/`Review`/`Divergence`/`Change`/`Task`/`EvidenceRecord` are LEDGER
  records. s15 materializes records FROM documents, adds NO `RecordKind`. Framing:
  `RecordKind` is the CORE vocabulary, closed here (a core kind stays a reviewed,
  breaking change — never a `kind: String` escape hatch); the closure is scoped
  to the `canon.` namespace, and s16 plugin kinds live in a separate namespaced
  registry, never `RecordKind` variants. **Reader rule s15 must implement:** an
  unrecognized `kind=<x>/` directory is skipped + reported (foreign namespace),
  never malformed core evidence — this is what keeps s16 from breaking s15
  consumers.
- **D2 — `Scenario` is a ledger INDEX; family docs authoritative.** `Scenario` =
  envelope + `project_id` + `scenario_id` + `title` (kept — the header label is a
  free denormalized nicety) + the existing optional `description` (kept as-is,
  additive contract — NOT removed) + `source_digest` (general; NO
  `covered`/`surface_ref` — the index is general, coverage is the gate's own).
  Rich
  facts (steps, tags beyond the id tag, provenance payload, `covered_by`, upstream/
  original refs) stay in the S11-validated family documents. Donor-faithful
  (`ledger-reader.md`: the donor ledger record is an index/pointer, never embeds
  the body); one source of truth (no drift); ~200-byte append-only records.
- **D3 — `specs.roots[]` config (new shape; stable-literal id).** `canon.yaml
  specs.roots[]` = `[{ id, root }]` (monorepo: one per nested project; `root`
  defaults to `specs`; unset → single `{ id: root, root: specs }`). `id` is a
  STABLE LITERAL — NEVER the checkout directory name (that splits identity across
  clones: content-not-path applied to config). Only the fail-loud/default
  SEMANTICS reuse `IngestSourceConfig::load`; the named-root-list is a new shape.
  (`load_artifact_source_config`'s fail-SOFT load is the pattern NOT to copy.)
- **D4 — Gherkin: deliberate line-scan subset; `.feature` authoritative.**
  `sync` reuses `canon-fmt::gherkin::scan` + `resolve::resolve_feature`, extended
  only to SURFACE what the scan already reads — attach each `@<area>.<surface>.
  <nn>` tag to its following header, expose the header label as `title`
  (retention, not a parser). canon does NOT parse the Gherkin body. `source_digest`
  is a full sha256-hex over the `.feature` file bytes — a NEW `SpecDigest` value
  type, NOT `ids::Sha` (a 40-hex git sha). File-granularity: any `.feature` edit
  re-materializes an index record per scenario in that file — accepted (the
  line-scan can't isolate per-scenario bodies without becoming a parser, and
  doc-changed IS the spec-side staleness signal; logical idempotence bounds churn).
- **D5 — `inventory sync` = validate → materialize, logically idempotent.** Per
  root: (1) `canon-fmt::check(root)` — ANY violation aborts the whole root (no
  partial sync); (2) scan features (deriving `title` + `source_digest` from the
  `.feature` corpus ALONE — NO `upstream`/`InventoryEntry.covered_by` read, an
  the donor consumer repo donor porting concern, not core canon); (3) upsert one `Scenario`
  index per `(project_id, scenario_id)` via the normal Tier write. Idempotence
  is LOGICAL: fold latest-per-key, no-op when `source_digest` (and the derived
  `title`) match (byte-level `WriteReceipt.deduped` handles identical
  resubmission; a fresh `at` alone changes the digest, so the logical fold is
  required). Coverage is the gate's OWN authority (`uncovered-cell`); the index
  carries no coverage field.
- **D6 — `project_id`: REQUIRED field, clean cutover.** A `ProjectId` newtype
  (`[a-z0-9][a-z0-9-]*`, no `_` — keeps `__` unambiguous alongside Review's
  `{scenario_id}__{pin}` and the `__{digest12}` suffix). REQUIRED on `Scenario`/
  `Review`/`Divergence` (zero real producers exist — every construction site is
  test/fixture — so "migration" = updating fixtures in-change; an Option branch
  would guard records that never existed, and a stray dogfood record correctly
  reads as malformed=no-evidence). OPTIONAL on `EvidenceRecord` (real records may
  exist via `canon gate promote`; folded into gate cell keys when present).
  `resolve_partition` prefixes the natural_key (PartitionKey shape unchanged, no
  `project=` Hive dimension, ALWAYS prefix even single-project): Scenario
  `<project_id>__<scenario_id>`, Review `…__<pin>`, Divergence
  `…__<run_seq>__<round>`. Content-only `resolve_partition` REQUIRES it be a real
  field (not a threaded-in-memory logical key).
- **D7 — Native verdict records-source adapters → S6.** Register ONE
  records-source `ArtifactAdapter` PER verdict kind — `Review` and `Divergence`
  — each a single-`RecordKind` registry entry + a
  `record_kind_for_records_adapter` arm (the existing dispatch maps one adapter
  id to exactly one `RecordKind`; `read_records_for` is already generic),
  mirroring the `Handoff` handle-based shape; verdicts fold into `Trajectory`
  via `store_trajectory` + `rebuild_namespace`. `Scenario` is NOT a source (a
  no-verdict index `sync` materializes). Regime derivation (spelled out —
  `Trajectory` is `regime_key`-only, no `scenario_id`): role ←
  `envelope.actor.role`, area ← `scenario_id.area()`, repo ← root, verdict via
  `verdict::derive_verdict` over Review/Divergence status. **Config + XOR:** a
  new `artifacts.native_records: bool` (default false) switch enables the
  verdict adapters against canon's own tiers, XOR-exclusive with the
  raw-artifact path fields (`ledger_root`/`divergences_root`/`openspec_root`) —
  config validation rejects both together before any read (the two paths'
  verdict rows differ slightly so `trajectory_content_digest` won't dedupe →
  double-count). The switch scopes ONLY the native verdict adapters; the
  existing `Handoff` Records adapter is UNAFFECTED.
- **D8 — `Divergence` fold + status machine (IN s15).** `DivergenceStatus +=
  StillDivergent, Deferred{reason, expiry}` (additive; old `{open,resolved}`
  fixtures still parse). `resolved-invalid` is fold-DERIVED — a variant of a
  SEPARATE `FoldedState` output enum, never a persisted `DivergenceStatus` (the
  on-disk event is never rewritten; only the fold's interpretation downgrades,
  `divergence-log.md` §3.6). PURE `fold_to_current_state(records, live_bindings,
  as_of)` in canon-model: rank by `run_seq: TotalOrder` (sole primary), `round`
  tiebreak-only (never `Ord`), group by `(project_id, scenario_id)`, `as_of` for
  `Deferred`-expiry purity (S7 PromotionGate convention). The resolved-binding
  re-check (the scenario's CURRENT app `sha` vs the `sha` the divergence
  resolved against — the SOLE live-checkable axis, no TOCTOU; WHO/WHEN are
  immutable provenance handled by `run_seq` ranking; a `digest` axis reserved
  for a future source) is passed
  as INPUT (canon-model can't depend on canon-store); canon-gate/canon-report own
  fetching — one validator, two callers.
- **D9 — Close the gate → canon-model field interface (IN s15).** Move onto
  `EvidenceRecord` as OPTIONAL, typed fields:
  `lifecycle: Option<TrustLifecycle>`, `flagged: Option<FlaggedOverlay>`,
  `evidence_sha: Option<Sha>`, `surface_ref: Vec<String>`, and
  `run_seq: Option<TotalOrder>` (the untyped companion `promote.rs:216` stamps —
  the 5th field). `Review` already carries `reviewer`/`pin`/`provenance_ref`
  natively — it needs only `project_id`. The gate then reads typed fields off
  `ctx.evidence`, DELETING `trust.rs::{TrustLadderTag, trust_ladder_tag_of}`,
  `staleness.rs::{SurfaceHint, surface_hint_of}`, and the second `GitTier`
  construction in `fold_latest_green_cells`. **Read is THREE-way (the pre-s15
  `trust.rs` semantics):** an ABSENT field → its documented safe default
  (absent `lifecycle` = `draft`, absent `flagged` = unflagged, absent
  `evidence_sha` = staleness-UNRESOLVABLE, absent `surface_ref` = empty, absent
  `run_seq` = none) — legitimate, so old promoted records never become
  malformed; a PRESENT well-formed field → its typed value; a PRESENT
  malformed field → `GateContext.violations` (`malformed-evidence`). **Serde
  landmine:** `Option<T>` + default-for-MISSING-key is exactly right and gives
  absent→default. What is FORBIDDEN is any mechanism — a `#[serde(default)]`
  that swallows a PRESENT malformed value, or an error-eating custom
  deserializer — that collapses present-malformed→absent (a corrupt tag would
  dodge `flagged`/`unreviewed-promotion`); present-malformed MUST surface as a
  VISIBLE violation via the two-phase raw key-presence check, not be dropped or
  defaulted. The human-only `flagged` ratchet (`trust.rs::attempt_clear`,
  clearing-actor role check) is untouched.
- **D10 — Native VERDICT producers + promote-to-Divergence.** `canon review add`
  (writes `Review`, actor-attributed, provenance-ref enforced) and `canon
  divergence {stage,promote,resolve,defer}` (staging JSONL-equivalent → `promote`
  assigns the monotonic `run_seq`). Extend `canon-gate::promote` (hardcoded to
  `EvidenceRecord`) to `RecordKind::Divergence`, partition axis `(project_id,
  role, surface)`; refusals never consume a `run_seq`; the fold groups by
  `(project_id, scenario_id)`. Corpus AUTHORING stays OUT (family docs
  authoritative, `sync` indexes; a `canon scenario new` writing both a stub AND a
  record = a second producer that drifts — the fragmentation s15 kills). VERDICT
  authoring is IN because nothing else can ever produce `Review`/`Divergence`
  natively; without it 2 of 3 kinds have no producer and the gate review index is
  empty forever.
- **D11 — Hoist `fold_latest_by_key`; delete `migrate_write`.** s15 needs the
  last-wins-by-`envelope.at` fold in ≥4 places (sync upsert-check, divergence
  fold, gate staleness, flywheel) — HOIST a generic `fold_latest_by_key` into
  canon-store (generalizing `canon-gate::ledger::latest_verdicts`) rather than a
  fourth local copy. Delete `canon-store::git_tier::migrate_write` (dead code,
  zero non-test callers) so no overwrite exception survives the change that makes
  the append-only path the sole sanctioned write.
- **D12 — Review index is project-aware (gate consumer of D6).**
  `trust.rs::review_index` + `TrustLadderCheck`'s `unreviewed-promotion`
  decision SHALL match a `Review` to an `EvidenceRecord` by the composite
  `(project_id, scenario_id)` when the evidence carries `Some(project_id)` — a
  review for one project never satisfies another project's same-`scenario_id`
  evidence (the isolation D6's composite identity promises). `project_id = None`
  legacy evidence falls back to the bare-`scenario_id` match (no regression);
  `Review.project_id` is required so every review carries a concrete project.

## Sequencing (identity-before-producers is load-bearing)

- **P1 — canon-model schema wave (no behavior):** `ProjectId`/`TotalOrder`/
  `SpecDigest` newtypes; `project_id` on the scenario spine (+Option on
  `EvidenceRecord`); `Scenario` index fields; `DivergenceStatus` +2; the 5
  OPTIONAL `EvidenceRecord` native fields (three-way read); pure
  `fold_to_current_state`; fixture
  updates. Everything depends on this.
- **P2 — canon-store wave:** composite natural_keys in `resolve_partition`; hoist
  `fold_latest_by_key`; unknown-`kind=` skip+report; DELETE `migrate_write`.
- **P3a — sync (∥ P3b after P2):** `specs.roots` config (fail-loud); gherkin
  surfacing; `source_digest`; drop Scenario `covered`/`surface_ref`; `canon inventory sync`.
- **P3b — gate wave (∥ P3a):** gate reads native fields THREE-way, deletes
  companions (rewiring `promote.rs`'s `trust_ladder_tag_of` caller BEFORE the
  delete); review index made project-aware; `promote` extended to `Divergence`;
  `canon review add` + `canon divergence …`; fold consumed by gate + S9
  burn-down.
- **P4 — flywheel:** per-kind (`Review`/`Divergence`) records-source adapters
  (registry + arms + regime derivation); `native_records` config switch + XOR
  check (`Handoff` unaffected).
- **P5 — closure:** selftest fixture corpora (two-sided exact-set oracle,
  rebindable-roots `SyncCtx`, frozen-incident slot); companion skill; docs.

## Risks

- **R1 identity-before-producers (highest):** if `sync` ships before `project_id`,
  the first real sync mints legacy-keyed records and the free-migration window
  closes. P1/P2 strictly before P3a.
- **R2 flywheel double-count:** S4 raw + native over the same corpus → near-but-
  not-identical verdict rows, `trajectory_content_digest` won't dedupe. XOR wiring
  at config validation.
- **R3 serde default swallows malformed trust fields (D9):** two acceptance
  tests — (a) a present-but-garbage `flagged` lands the record in violations,
  never green; (b) an `EvidenceRecord` with the field ABSENT reads as the
  documented default (unflagged/`draft`), never malformed — the `Option<T>`
  default fires ONLY for a missing key, never for a present malformed value.
- **R4 covered ≠ coverage:** `Scenario.covered` is declared-by-inventory; the
  gate's `uncovered-cell` stays authoritative. Name/doc as an index fact.
- **R5 gherkin scope creep:** surfacing stays a line-scan retention change; the
  supported subset is pinned (D4).
- **R6 default project_id from checkout dir → cross-clone identity split.** Stable
  literal default (D3/D6).
- **R7 run_seq monotonicity** assumes a serialized `promote` (single-writer) —
  documented (same assumption `promote.rs` already makes for evidence).
- **R8 file-granularity source_digest churn:** one `.feature` edit re-materializes
  every scenario index in that file — bounded, harmless under logical idempotence;
  stated so append volume isn't misread as a bug.

## Testing

- Config: default `{id: root, root: specs}`; multiple roots; malformed `specs:`
  fails loud; missing → default; a root wired both raw-artifact AND native fails
  the XOR check.
- Gherkin scan: tag↔header linkage + title surfaced; same `scenario_id`s/area as
  `canon fmt`; no-canon-tag `.feature` = reported gap, not panic; `source_digest`
  is a stable sha256 over file bytes.
- sync: materializes one `Scenario` per `(project_id, scenario_id)` with correct
  `source_digest` + `title` (general index; no `covered`/`surface_ref`); re-sync
  unchanged → zero writes; changed doc → one new record; two roots sharing a
  `scenario_id` stay DISTINCT; malformed corpus → no writes + loud error.
- producers: `canon review add` writes an attributed `Review`; `canon divergence
  stage`+`promote` assigns a monotonic `run_seq`, `resolve`/`defer` transition;
  refusal consumes no `run_seq`.
- divergence fold: lower `run_seq` at a higher `round` folds correctly; a
  `Resolved` binding whose live ledger record changed downgrades to
  `ResolvedInvalid` at fold time (no TOCTOU); `Deferred` honors `as_of` expiry.
- gate: reads the 5 native fields THREE-way — an ABSENT field → its documented
  safe default (`draft`/unflagged/staleness-unresolvable/empty/none), so an old
  promoted record with no native fields is legitimate, never malformed; a
  PRESENT well-formed field → typed; a PRESENT malformed field → `malformed-
  evidence`, never silently absent/default; `flagged` ratchet still blocks an
  agent clear; the review index matches by `(project_id, scenario_id)` so a
  review for one project does NOT satisfy another project's same-`scenario_id`
  evidence, while a `project_id = None` legacy record still matches by bare
  `scenario_id`.
- flywheel: a `Review` AND a `Divergence` in the SAME run BOTH produce
  `Trajectory` records via their per-kind records-source adapters (neither kind
  dropped by single-kind dispatch; regime derived via `attach_regime_key`
  inputs; parity with the S4 path); `native_records:true` + a raw-artifact path
  fails the XOR check; the `Handoff` adapter is unaffected by the switch.
- selftest: `inventory-*`/native-record fixture corpora with two-sided exact-set
  oracles (missing AND extra) + a frozen-incident slot, registered in `canon
  selftest`.
