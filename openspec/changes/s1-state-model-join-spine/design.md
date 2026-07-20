## Context

S0 scaffolded `crates/canon-model` as a compiling stub (one marker constant,
one test — design D5 of that change). S1 gives it its first real content:
the closed record-kind set every later spec depends on. This is the
design's single most-cited gap: "no artifact can join to the
session/cost/trajectory that produced it" (design §1, Logging row) and "two
stores, no documented join key" (the two prior session/event stores). S1 is where that
stops being true, by construction — the join spine (design §S1) is not a
convention several systems are asked to follow; it is the type system.

Donors (design §3): the donor parity harness's ledger record shape and its
`_ledger_problem`/`FAILURE_CLASSES` discipline; the donor monorepo's `handoffs`
table for its state-machine column shape (not the full row — see D4); the
vendored upstream launcher's `session_id` as its own cost join key (donor row, design §3).

## Goals / Non-Goals

**Goals:**
- One closed set of record types, each carrying the same
  `{schema, kind, at, actor}` envelope, with serde (de)serialization and
  JSON-schema export from the same source (no hand-maintained schema
  drifting from the Rust types).
- The eight-key join spine (design §S1 table) expressed as real types
  (newtypes over `String`/`Uuid`/`Ulid`, not bare strings threaded through
  function signatures) plus a generated document — generation, not authored
  prose, so the doc can never silently drift from the code.
- `Handoff`'s 13 non-body state-machine fields (D4) byte-compatible on the
  wire with the matching columns of the donor monorepo's `handoffs` Postgres table, so
  canon and the donor CLI's handoff queue agree on one state machine without a
  translation layer desyncing *that* subset. This is state-machine-core
  compatibility, not a full-row translation-free write path: the donor monorepo's
  `trigger` column (`NOT NULL`, no default) and its
  `created_at`/`created_by_*`/`refs_extra` columns have no `Handoff`
  field, and canon's own envelope fields have no donor column — S4 (the
  change that actually reads/writes the live table) owns bridging that
  gap (see Risks).
- Stable, greppable failure-class strings and the "malformed evidence is no
  evidence" rule as a `canon-model` primitive (a validation function every
  later crate calls), not something each consumer re-implements.

**Non-Goals:**
- Implementing any storage tier (S2 owns `canon-store`) — S1 only defines
  what gets stored, not where.
- Implementing the per-domain Handoff body template *renderer* logic beyond
  a registry interface — actually authoring the 기획/디자인/개발/테스트
  templates is content work that can land incrementally after this change;
  S1 ships the registry contract (`canon.yaml`-referenced, vocabulary-aware
  once S10 lands) and at least one working template as a proof of the
  contract (fixture requirement, task group 5).
- Cutting the donor CLI's handoff queue over to call canon's `Handoff` type. S1
  conforms to the existing table; the cutover is explicitly a later,
  separate change (design §3: "canon **conforms** to its table").
- Typed task/spec vocabulary atoms (S10's job) — S1's `Task`/`Scenario`
  types carry the join keys and envelope; S10 later adds authored-vocabulary
  validation on top without changing S1's wire shape.

## Decisions

**D1 — The record-kind set is closed, not extensible via a generic
`Record<K, V>`-style escape hatch.** All twelve kinds (`Change`, `Task`,
`Scenario`, `Session`, `Run`, `Event`, `Handoff`, `Review`, `Divergence`,
`Trajectory`, `StrategyItem`, `EvidenceRecord`) are named Rust enum variants
or structs in `canon-model`, matching design §1's "canon is the format
authority" decision (decision 4) and its "closed, versioned types" framing
(design §S1 opening line). Rationale: an open/dynamic kind set is exactly
what let the donor monorepo accumulate three uncoordinated management systems — a closed
set forces every new artifact family (S11's donor parity harness migration, a future
consumer repo) to either fit an existing kind or land as an explicit,
reviewed `canon-model` change. Alternative considered: a `kind: string` +
untyped `payload: serde_json::Value` escape hatch for forward-compat.
Rejected — it would silently recreate the "no documented join key" problem
inside canon itself, since nothing would force a new payload shape to carry
`actor`/join keys.

**D2 — Envelope fields are on every record via a shared `Envelope` struct
composed in, not duplicated per-type.** `{schema: u32, kind: RecordKind, at:
DateTime<Utc>, actor: Actor}` where `Actor { agent_id: String, role:
RoleId, session_id: Option<SessionId>, model: Option<String> }`. `schema` is
the per-kind version integer (bumped on any breaking field change to that
kind; `canon fmt`/`canon migrate`, S11, key off it). Every record type is
`#[serde(flatten)] envelope: Envelope` plus its own fields — never a bare
`by: String` field anywhere in `canon-model` (design §S1's explicit
rejection of that shape, citing it as "the donor parity harness audit's biggest
cross-family gap").

**D3 — JSON-schema export and the join-spine doc are both generated at
build/test time from the same Rust source, never hand-authored.** Use
`schemars` (derive `JsonSchema` alongside `Serialize`/`Deserialize`) to
emit one `.schema.json` per record kind into a `schemas/` output directory;
a small `canon-model` binary target (or `xtask`-style script) walks the
same type registry to emit `JOIN_SPINE.md` — one row per join key, each
row's grammar and "joins" column pulled from doc comments on the
corresponding newtype, so the table in this change's own proposal
(design §S1's table, reproduced below) cannot drift from the types that
implement it once both are generated from one source. `canon fmt --check`
(S11) and `canon selftest` (S5/§8) both re-run this generation and diff
against the committed output, catching drift as a CI failure rather than a
silent doc rot.

Join spine (generated; this table is the design-time contract the
generator must reproduce byte-for-byte):

| Key | Grammar | Joins |
|---|---|---|
| `change_id` | openspec slug | change ↔ tasks ↔ specs |
| `task_id` | `<change_id>#<n>` | task ↔ evidence ↔ trajectory |
| `scenario_id` | `<area>.<surface>.<nn>` (never renumbered) | spec ↔ test ↔ ledger ↔ divergence |
| `session_id` | agent-CLI UUID (the vendored upstream launcher's join key) | session ↔ cost ↔ run ↔ trajectory |
| `run_id` | ULID | run ↔ events ↔ manifest |
| `handoff_id` | existing donor CLI grammar | handoff ↔ session ↔ change |
| `sha` / `pr` | git | reward signals ↔ trajectory |
| `regime_key` | `<role>/<repo>/<area>/<hash>` | strategy write ↔ retrieval (identical at both ends) |

**D4 — `Handoff`'s state-machine fields are a closed, fixed struct mirroring
the donor monorepo's `handoffs` table column-for-column; the body is an opaque, domain-templated
blob.** `Handoff { id, state, chain_id, parent_handoff_id, seq, claimed_by,
claimed_at, completed_at, abandoned_at, openspec_change_slug,
research_vendor_slug, tags, title, body: HandoffBody }` where `state` is a
closed 4-variant enum (`Pending|InProgress|Done|Abandoned`, matching
the donor monorepo's `handoffs` table's runtime-checked string values exactly so canon and
the donor CLI's handoff queue agree on the same four `state` strings) and
`HandoffBody { domain: DomainId, template_version: u32, fields:
serde_json::Value }` where the registry (D5) owns validating `fields`
against `domain`'s current template schema. This is a state-machine-core
mapping, not a full-row one — the donor monorepo's `trigger`/`created_at`/`created_by_*`/
`refs_extra` columns have no field here (Risks). Rationale: the state
machine (design §S1: "stays wire-compatible … so both tools read and write
one queue" — scoped to these 13 non-body fields, not the donor monorepo's full column
set) must be rigid; the body ("deliberately not fixed … freely definable")
must not be, and conflating them in one flat struct would force either
over-constraining the body or under-constraining the state machine.

**D5 — The per-domain template registry is a trait + `canon.yaml`-referenced
manifest lookup, not a hardcoded match on `domain`.** `trait
HandoffTemplate { fn domain(&self) -> DomainId; fn validate(&self, fields:
&serde_json::Value) -> Result<(), Vec<EvidenceViolation>>; fn render(&self,
fields: &serde_json::Value) -> String; }`; `canon.yaml`'s
`handoff_templates:` key lists which templates are registered for a given
consumer repo. S1 ships the trait, the registry lookup, and one concrete
template (기획, the simplest — title + summary + acceptance-criteria fields)
as the fixture proof; S10 later makes template definitions themselves
vocabulary-authored (design §S10: "handoff body templates (S1) also become
vocabulary-defined here") without changing this trait's shape.

**D6 — Failure-class strings live as `canon-model` constants (a
`FailureClass` enum with a fixed `as_str()` mapping), re-exported for every
crate that raises `EvidenceViolation`s.** Mirrors the donor parity harness's
`FAILURE_CLASSES` tuple-of-exact-strings discipline ("grep these EXACT
strings — never rename without updating fixtures + hooks together").
`EvidenceRecord` validation (`canon-model::validate_evidence`) returns
`Result<(), EvidenceViolation>` where `EvidenceViolation { class:
FailureClass, subject: String, detail: String }` — the "malformed evidence
is no evidence" rule (design §7) is implemented once, here, as a function
every later crate (`canon-gate` in S5, `canon-ingest` in S3/S4) calls rather
than reimplements. A validation failure is always `Ok(skip) +
Violation`, never a `panic!`/`Result::Err` that aborts a batch read —
mirrors the donor parity harness's `_load_ledger` "malformed records are skipped
here (never crashes)" contract exactly.

## Risks / Trade-offs

- [Risk] A closed record-kind enum (D1) means adding a thirteenth kind is a
  breaking `canon-model` change, not a drop-in extension. → Mitigation:
  that friction is intentional (D1's rationale) — new kinds are meant to be
  rare and reviewed, and `schema` versioning (D2) plus `canon migrate` (S11)
  exist precisely to manage controlled breaking changes.
- [Risk] Generating the join-spine doc from doc comments (D3) is only as
  good as those comments; a careless doc-comment edit silently changes
  published documentation. → Mitigation: `canon selftest`/`fmt --check`
  diff the generated output against the committed file in CI, so an
  unintentional change fails the build rather than merging silently.
- [Risk] `HandoffBody.fields` as `serde_json::Value` (D4) reintroduces an
  untyped escape hatch inside an otherwise-closed model. → Mitigation:
  scoped narrowly — only the body payload is untyped, and D5's registry
  validates it against a template schema before any write is accepted;
  the state-machine fields (the wire-compat surface with the donor monorepo) stay fully
  typed.
- [Risk] The `session_id` newtype (D3) fixes one grammar ("agent-CLI
  UUID", the join-spine table) but S3's adapters do not derive it
  uniformly at the source: Claude Code and Codex transcripts trust the
  transcript filename stem, while omp/pi transcripts trust only an
  in-file header field (per the vendored upstream launcher's vendor-audit
  cross-check against this design, candidate C2). A `SessionId` newtype alone
  cannot catch a wrong-source derivation — it validates shape, not
  provenance. → Mitigation: not S1's to fix (S1 owns the type, not the
  per-adapter derivation) but flagged here so S3's design treats
  "derive `session_id` identically regardless of source" as a first-
  class requirement of adapter conformance, not an implementation detail
  each adapter is free to improvise.
- [Trade-off] Shipping only one concrete Handoff template (기획) in this
  change, not all of 기획/디자인/개발/테스트, is a deliberate scope cut —
  the registry contract (D5) is what S1 must prove; authoring the other
  three domains' templates is unblocked follow-on content work, not a gate
  on this change's completion.

## Migration Plan

No existing canon data to migrate (S0 shipped no record instances). The donor monorepo's
`handoffs` table itself is untouched by this change — S1 defines a Rust
type whose state-machine core reads/writes the matching columns
compatibly (not the full row — D4, Risks); no schema migration runs
against hosted Postgres here. The eventual cutover of the donor CLI's handoff queue to call canon
than its own CAS SQL) is out of scope (Non-Goals) and will be its own
change once S1 ships.

## Open Questions

- None new. §10 Q3 (the donor harness's dev-namespace cutover) and Q5
  (the donor vocabulary project's plugin lift mechanism) are owned by S6 and S10 respectively, not
  S1 — S1's `Task`/`Scenario` shapes are deliberately generic enough that
  neither later decision requires reopening this design.
