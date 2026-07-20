## Why

canon is a SELF-COMPLETE harness (s15's own framing): `install canon` and
the native ledger loop works without depending on an external tool. s15
delivered the inventory/scenario spine, s16 delivered plugin extensibility —
and s15's own proposal named exactly one remaining sibling pillar: "**s17**
adds the integration layer (openspec / superpowers / external-ledger IMPORT
— a secondary connector, like Jira importing GitHub issues)"
(`s15-spec-ledger-unification/proposal.md:10-13`). s16 restated the same
boundary twice while shipping — importing "a foreign PLANNING dialect
(openspec/superpowers/donor-JSON) into canon's own format" is "NOT s16's
job" (`s16-plugin-extensibility/proposal.md:63-65`), and "s17
(integration/import of a foreign planning dialect) is the remaining sibling,
still untouched by this change" (`design.md:90-92`). This change is that
pillar.

**The gap this change closes: the join spine's TOP row is empty.** S1
shipped `Change`/`Task` as two of the twelve closed record kinds (join-spine
rows: change ↔ tasks ↔ specs; task ↔ evidence ↔ trajectory), S2 shipped
their storage + routing (`canon.yaml` routes `change: git`, `task: pg` —
nothing to re-add), and S4's `openspec-task` VERDICT adapter already keys
`ArtifactEvent`s by `<change-slug>#<n>` `task_id` values derived from
`openspec/changes/*/tasks.md`
(`crates/canon-ingest/src/artifact_adapters/openspec_task.rs`). But nothing
POPULATES the plan side of those joins: outside test fixtures, the only
`Task` producer is canon-vocab's one-at-a-time typed compile
(`crates/canon-vocab/src/compile.rs`, the S10 `canon gate task` path) and
there is NO `Change` producer at all (verified: every
`Change::new`/`RecordKind::Change` construction site in the workspace is a
test, a fixture, or a schema walker). A repo that plans in openspec — canon
itself carries 18 live change dirs — has verdict trajectories keyed by
`task_id` with no `Task` or `Change` record to join against; every
plan ↔ evidence ↔ trajectory query hits a hole where the plan should be.

**Precedent that this is buildable, not speculative:** the connector shape
is already proven TWICE in the same crate. `canon ingest sessions` (S3)
imports agent-CLI transcripts through a `SessionAdapter` trait + static,
declaration-ordered registry; `canon ingest artifacts` (S4/S14) imports
divergence/ledger/task-state/handoff artifacts through an `ArtifactAdapter`
trait + static registry
(`crates/canon-ingest/src/{adapter,registry,artifact_adapter,artifact_registry}.rs`).
Both normalize a foreign format onto the closed record model, persist
through canon-store's tiered write path, dedupe by content digest, and gate
re-scans behind per-source watermark cursors
(`crates/canon-cli/src/{ingest,artifact_ingest}.rs`,
`crates/canon-store/src/cursor.rs`). s17 is the THIRD ingest family — plans
— one more trait + registry pair of the SAME shape, not a new architecture.
A new dialect is one registry entry, never a re-plumb.

**Scope discipline (the s16 lesson, restated):** a foreign plan maps ONTO
the existing twelve kinds — `Change` + `Task` here — or its construct is
DROPPED with a named diagnostic. No thirteenth `RecordKind`, no new core
field, no authority change: coverage stays `canon-gate`'s `uncovered-cell`
check, promotion stays S7's, the gate stays S5's, and `canon inventory sync`
stays the ONLY `Scenario` producer (s15 P3a). Import is a connector, never
authority.

## What Changes

- **Plan-connector family in `canon-ingest`.** A `PlanAdapter` trait
  (`dialect_id()`, `resolve_source`, `parse`) + a static,
  declaration-ordered `plan_registry`, mirroring the session/artifact
  families (S3 design D1's "trait + static table, no dynamic plugin
  loading" shape; S4's `ArtifactSourceKind`/`ArtifactDispatchOutcome`
  explicit-diagnostic discipline). One shared normalization target,
  `PlanParseOutcome`: `Change` + `Task` candidates, per-construct
  unmapped-drop diagnostics (a NAMED count per dropped construct kind,
  never a silent skip), and a malformed count ("malformed evidence is no
  evidence" — a change dir or row the dialect cannot parse is skipped AND
  counted, never a crash).
- **openspec dialect adapter — the reference dialect + acceptance vehicle.**
  `openspec/changes/<slug>/` → ONE `Change` record (`change_id` = the dir
  basename, verbatim; `title` = the slug; `summary` = the first paragraph
  under proposal.md's `## Why`); `tasks.md` checkbox rows → `Task` records
  (`task_id` = `<change_id>#<n>` — the EXACT derivation S4's verdict adapter
  already uses, so plan rows and verdict trajectories actually join;
  `status` from `- [ ]`/`- [x]`; `evidence_note` from the ` — ✅ ` suffix or
  a `**DEFERRED to §<to>**`/`**DROPPED**` annotation); `ChangeStatus`
  derived deterministically from the checkbox tallies + archive location —
  the proposal → tasks → archive flow `ChangeStatus`'s own doc comment
  already mirrors (`records.rs`). Spec-delta `#### Scenario:` blocks and
  design.md prose are NOT imported — dropped with named diagnostics (see
  non-goals for why overlay is not an option either).
- **One openspec checkbox grammar, two consumers.** `openspec_task.rs`'s
  local row-grammar mirror (`parse_row`, annotation + evidence handling) is
  extracted into a shared canon-ingest module consumed by BOTH the S4
  verdict adapter and the new plan adapter — code motion with ZERO behavior
  change to the verdict adapter (its existing tests pin that);
  `canon-gate::checkbox` remains canon's format AUTHORITY for the row shape,
  and canon-ingest still takes no canon-gate dependency (the operator
  directive `openspec_task.rs`'s module doc already records).
- **CLI wiring: `canon ingest plans`.** A third `IngestCommand` variant
  beside `sessions`/`artifacts`: reads `canon.yaml`'s new top-level `plans:`
  section (`sources: [{dialect, root}]` — an ABSENT section means zero
  sources scanned, never a hardcoded default; a PRESENT section parses
  STRICTLY with `deny_unknown_fields`, failing loud on a typo or an unknown
  dialect id, mirroring `ingest.rs`'s `RawIngest`), applies the S3
  watermark-cursor gate per source (`canon-store::cursor`, unchanged
  sources are skipped wholesale), persists every candidate through
  `TierRegistry::persist` with the DuplicatePath-tolerant idempotent write
  (`ingest.rs::persist_idempotent`'s exact discipline), and degrades to the
  documented `unwritten` seam when a routed tier is unreachable (task
  routes to pg — no live DSN prints the normalized bundle, never a fatal
  error for the git-routed changes already written). A targeted
  `--dialect <id> --source <path>` pair bypasses config for a one-shot
  import; either flag without the other fails loud.
- **Reclassification, zero code.** s15's proposal already reclassified the
  S4 donor-JSON verdict adapters (ledger/divergence/openspec-task) as "s17
  connectors, untouched" (`s15-spec-ledger-unification/proposal.md:201-203`)
  — this change formally adopts them as the EVIDENCE-side members of the
  integration pillar and changes none of them: plan import (this change)
  brings the PLAN side, S4's adapters keep bringing the VERDICT side, and
  the two meet on the join spine's `task_id`.
- **Companion skill** `canon/skills/canon-plan-import/SKILL.md` — configure
  a `plans:` source → import → query the Change/Task rows → join against
  S4's verdict trajectories, with the openspec dialect as the worked
  example; materialized via `canon skills install` + install-lock bump.

### Added Capabilities

- `plan-import-connector`: the generic third ingest family — `PlanAdapter`
  trait + static declaration-ordered registry, `canon.yaml` `plans:`
  config surface, `canon ingest plans` CLI, watermark-gated idempotent
  persistence through canon-store's validated write path,
  connector-never-authority invariants, and the closed-kind mapping rule
  (a foreign construct with no home among the twelve kinds is dropped with
  a NAMED diagnostic — never a 13th `RecordKind`, never a silent skip).
- `openspec-plan-dialect`: the concrete openspec mapping — change dir →
  `Change`, tasks.md rows → `Task` with `task_id` parity against the S4
  verdict adapter's derivation, deterministic `ChangeStatus` derivation
  (proposed/in_progress/completed/archived), the shared checkbox-grammar
  module (one grammar, two consumers, zero verdict-layer behavior change),
  and the named drop diagnostics for spec-delta scenarios + design prose.

### Explicit non-goals

- No new `RecordKind` variant — the 12-member closure (s15 design D1,
  `envelope.rs`) is UNCHANGED and stays structurally asserted
  (`RecordKind::ALL.len() == 12`, three independent assertion sites). A
  foreign plan construct with no home kind is dropped with a named
  diagnostic; it never becomes a kind, a core field, or an untyped payload.
- No `Scenario` records from plan import — `canon inventory sync` stays the
  ONLY `Scenario` producer (s15 P3a's single-producer discipline). An
  openspec spec-delta `#### Scenario:` block fails the `scenario_id`
  grammar (`<area>.<surface>.<NN>`) outright AND has no `.feature` corpus
  backing; it is dropped with the named `spec-delta-scenario` diagnostic.
  It is not an s16 overlay either: an overlay attaches to an EXISTING core
  `Scenario` record on `(project_id, scenario_id)` (s16 restricts
  `attaches_to.core_kind` to `scenario` only) — a spec-delta scenario has
  no such core record to attach to, so overlay is structurally unavailable,
  not merely declined.
- No verdict-layer change — the S4 `openspec-task` VERDICT adapter's
  behavior is byte-identical (the shared-grammar refactor is code motion,
  pinned by its existing tests); the frozen `derive_verdict` table,
  `ArtifactAdapter` trait, and `ArtifactEvent` contract are untouched.
- No change to coverage/promotion/gate AUTHORITY — `canon-gate`'s
  `uncovered-cell` check, S5's trust ladder, and S7's promotion read
  NOTHING importer-specific; `canon gate check` verdicts are byte-identical
  with and without a plan import having run (an acceptance test).
- No superpowers dialect adapter in this change — its mapping is SKETCHED
  in design.md (D9) and deferred to a named follow-up wave
  (`plan-dialect-superpowers`): a superpowers plan checklist has no
  canon-gate-authored format authority yet, and pinning a speculative
  grammar here would freeze the wrong thing. The `PlanAdapter` registry is
  the seam it lands in — one entry, no re-plumb — proven in this change by
  a fixture dialect adapter in tests.
- No donor-JSON plan re-homing — the donor JSON artifacts canon knows
  (ledger/divergence/handoff/task-state) are EVIDENCE, already ingested by
  S4 as verdicts; no donor PLAN-JSON corpus is concretely pinned today.
  Deferred to a named follow-up wave (`plan-dialect-donor-json`) until a
  concrete donor corpus exists to map.
- No `--watch` for `canon ingest plans` — a plan import is an
  operator-triggered pull (the Jira-importing-GitHub analogy), not a
  streaming source like session transcripts; the cursor gate makes a
  re-run cheap regardless. Adding `--watch` later is additive CLI surface,
  not a design change.
- No new storage tier, mart, packaging, or Docker infra — `Change`/`Task`
  persistence, routing (`change: git`, `task: pg`), schema registry
  bindings, and the report marts that read them all exist since S2/S9;
  s17 only produces records into them.
- No write path outside canon-store's validated tiered write — imported
  records go through `TierRegistry::persist` exactly like session/artifact
  ingest; there is no direct-file, bypass, or unvalidated write anywhere
  in this change.

## Impact

- **`canon-ingest`**: new `plan_adapter.rs` (trait + `PlanParseOutcome`),
  `plan_registry.rs` (static registry, declaration-ordered),
  `plan_adapters/openspec.rs` (the reference dialect), and
  `openspec_rows.rs` (the shared checkbox-row grammar module —
  `artifact_adapters/openspec_task.rs` becomes its second consumer, zero
  behavior change).
- **`canon-cli`**: `IngestCommand::Plans` + a `plans.rs` driver (strict
  `plans:` config parse, cursor gate, `persist_idempotent`, `unwritten`
  seam, human/JSON output) — the one place canon-ingest's plan family and
  canon-store meet, mirroring `ingest.rs`/`artifact_ingest.rs`.
- **`canon-model` / `canon-store` / `canon-gate` / `canon-learn` /
  `canon-vocab` / `canon-plugin`**: UNCHANGED. Zero new kinds, zero new
  core fields, zero new gate logic, zero new store primitives —
  `RecordKind::ALL` stays 12 members, structurally asserted, the same
  acceptance bar s16 held itself to.
- **New companion skill** `canon/skills/canon-plan-import/SKILL.md` +
  install-lock bump.
- **Join-spine payoff, restated:** after one `canon ingest plans` +
  `canon ingest artifacts` pass over the same repo, `task_id` joins
  plan-side `Task` records to verdict-side trajectories with no schema
  work — the two sides were built for each other by S1 and finally both
  populated.

