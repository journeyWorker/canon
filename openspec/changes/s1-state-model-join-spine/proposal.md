## Why

Every canon capability from S2 onward (storage tiers, ingest adapters, the
trust-spine gate, strategy memory, reward wiring, the dashboard) reads and
writes the same handful of record kinds — Change, Task, Scenario, Session,
Run, Event, Handoff, Review, Divergence, Trajectory, StrategyItem,
EvidenceRecord. Today those kinds exist as three uncoordinated systems in
the donor monorepo (openspec `tasks.md` checkboxes, the donor CLI's handoff queue's hosted-Postgres CAS table, the
`.superpowers/sdd/progress.md` prose ledger) and a fourth, richest-but-
isolated shape in the donor parity harness's ledger — and, per the design's core finding,
**no artifact can join to the session/cost/trajectory that produced it**.
S1 defines the one closed, versioned type set (`canon-model`) and the one
join-key table every later spec reads from, so canon becomes the format
authority (decision 4) instead of adding a fifth uncoordinated shape.

## What Changes

- Add `canon-model`: closed, versioned Rust types (serde + JSON-schema
  export) for `Change`, `Task`, `Scenario`, `Session`, `Run`, `Event`,
  `Handoff`, `Review`, `Divergence`, `Trajectory`, `StrategyItem`,
  `EvidenceRecord`. Every record carries the envelope `{schema: <int>, kind,
  at, actor}`, where `actor` is `{agent_id, role, session_id?, model?}` —
  never a bare `by` string.
- Publish the join spine as both code (the eight key types/newtypes) and a
  generated Markdown doc (`docs/join-spine.md` or equivalent, built from the
  same source the types live in, never hand-maintained separately).
- Define `Handoff`'s state-machine core — `id`, `state`
  (`pending|in-progress|done|abandoned`), `chainId`/`parentHandoffId`/`seq`,
  `claimedBy` CAS fields, `openspecChangeSlug` — wire-compatible with the
  matching columns of the donor monorepo's `handoffs` Postgres table,
  so the donor CLI's handoff queue
  and canon agree on one state machine. This covers the state-machine
  core only, not every column of the live table (see Impact).
- Add a per-domain Handoff body template registry (기획/디자인/개발/테스트/…),
  referenced from `canon.yaml`, that renders and validates the free-form
  body while the state-machine fields above stay canonical and fixed.
- Define stable failure-class strings (never renamed without a coordinated
  fixture + hook migration, mirroring the donor parity harness's `FAILURE_CLASSES`
  discipline) and the "malformed evidence is no evidence" rule (skip +
  violation, never crash — the donor parity harness's `_ledger_problem` pattern).
- Export versioned JSON-schemas for every record kind so non-Rust
  consumers (CI scripts, the S9 dashboard, other repos' tooling) can
  validate without linking the Rust crate.

## Capabilities

### New Capabilities

- `canon-model-schema`: the closed, versioned record-kind set with its
  `{schema, kind, at, actor}` envelope, serde round-trip, and published
  JSON-schemas.
- `join-spine`: the eight join keys (`change_id`, `task_id`, `scenario_id`,
  `session_id`, `run_id`, `handoff_id`, `sha`/`pr`, `regime_key`), their
  grammars, and a generated join-spine document built from the same source
  as the types.
- `handoff-state-machine`: the `Handoff` type's state-machine core
  wire-compatible with the matching columns of the donor monorepo's `handoffs` table,
  plus the per-domain body template registry.
- `evidence-integrity`: malformed-evidence skip+violation handling and the
  stable failure-class string contract.

### Modified Capabilities

_None — S0 shipped no record types; nothing existing to modify._

## Impact

- New `canon-model` crate (already scaffolded as a stub by S0; this change
  gives it its first real types) — every later crate (`canon-store`,
  `canon-ingest`, `canon-gate`, `canon-learn`, `canon-report`) depends on it.
- New generated `docs/join-spine.md` (or `crates/canon-model/JOIN_SPINE.md`)
  build step.
- No changes to the donor monorepo's `handoffs` table itself —
  S1 conforms `Handoff`'s state-machine core to that table's matching
  columns; migrating the donor CLI's handoff queue to actually call canon is a
  later cutover (design §3, "canon conforms to its table"), out of scope
  here.
- [Risk] `Handoff` models the donor monorepo's `handoffs` state-machine core (13
  fields, design D4) but not the full live row: the donor monorepo's `trigger` column
  is `NOT NULL` with no default and has no `Handoff` field, so canon
  cannot construct a valid donor `INSERT` as-is; the donor monorepo's `created_at`/
  `created_by_session_id`/`created_by_branch`/`created_by_worktree`/
  `created_by_host`/`refs_extra` have no `Handoff` analog, so reading a
  real donor row loses them; and canon's own envelope
  (`schema`/`kind`/`at`/`actor`) has no donor column, so
  `serde_json::to_value(handoff)` is not the donor's column set. →
  Mitigation: not S1's to close — S1 only defines the type. **S4**
  (artifact/handoff ingest, the change that actually reads/writes the donor
  monorepo's live `handoffs` table) MUST own bridging this gap: map `trigger` on
  write (or document why a canon-originated handoff can always supply
  one) and preserve `created_by_*`/`refs_extra` on read (carry-through
  or an explicit, reasoned drop) — not silently lose them.
- No changes to the donor parity harness's ledger tooling; S1 only aligns canon's failure-class and
  malformed-evidence conventions with it, in preparation for S11's
  migration of the donor parity harness's corpus onto canon-validated formats.
