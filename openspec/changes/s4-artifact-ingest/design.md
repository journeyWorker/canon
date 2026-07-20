## Context

S4 extends `canon-ingest` (introduced by S3) with four source adapters whose
job is not just normalization (S3's concern) but **verdict derivation**: turning
"this artifact exists" into "this is success/failure signal for role X". This is
the ingest half of the design's central flywheel (§4): `ingest → trajectories +
verdicts → distill → statistical promotion → …`. S6/S7 (role-namespaced strategy
memory, reward wiring) consume S4's verdict stream directly; S4 does not
distill or promote anything itself — it only produces the normalized signal.

Grounded in the on-disk/table shapes documented by the design §3 donor table
and the donor parity harness's vendor audit. The donor consumer repo's `spec/ledger/`/
`spec/divergences/` trees and the donor monorepo's `handoffs` table are the DONOR shapes
these adapters normalize — **never a live path/DSN this repo hardcodes** (S4
foundation rescope, operator directive 2026-07-11: S4 never reads
a live donor consumer-repo path or a live donor event-store connection). Every
adapter's actual source root is `canon.yaml`-configured
(`crate::artifact_adapter::ArtifactSourceConfig`, S4 foundation); the donor consumer repo
is simply the FIRST configured source and the origin of the frozen fixture
corpus S4's golden-file test uses:

- **Ledger records** (`spec/ledger/kind=<kind>/…`) are one JSON object per
  file. `review`/`design-review`/`code-review`/`clear` records are
  Hive-partitioned `kind=<kind>/area=<area>/<scenario_id>.json`; `run`/`drill`
  records are flat `kind=<kind>/<timestamp>-<lane>-<sha>-<rand>.json` with no
  `area=` level (parity.py `_ledger_layout_problem` enforces exactly this
  split). A sampled `review` record today: `{"schema":1,"kind":"review",
  "scenario_id":"idolive.hub.01","covered_ref":"routes/…#RouteComponent",
  "pin":"9c93d024b","reviewer":"reviewer-idolive-hub","at":"…"}`. A sampled
  `code-review` record carries a `"verdict":"faithful"` field and both
  `port_ref`/`covered_ref`. A sampled `run` record carries `"result":"pass"`,
  a bare `"by":"flutter-test-machine"` string (not a structured actor — the
  exact gap S11 later closes), and `"evidence":[]`.
- **Divergence JSONL** (`spec/divergences/lane=<lane>/area=<area>/
  surface=<surface>/*.jsonl`) mixes a `"type":"manifest"` header line
  (round metadata + `reviewed_ids`) with `"type":"review"` lines per
  scenario, each carrying `status` (`"open"` et al.), `port_ref`, `covered_ref`,
  and a structured `aspects: [{what, covered, port, ref}]` array — the richest
  artifact family member (design §5 S11 audit table: "structured aspects,
  digest anchoring ✓").
- **canon's own handoffs table** (S1's `Handoff` type,
  `crates/canon-model/src/handoff.rs`, wire-compatible field-for-field with
  the prior event store's `handoffs` table schema): `id`, `state`
  (`pending|in-progress|done|abandoned`), `chainId`/`parentHandoffId`/`seq`,
  `claimedBy` (CAS claim), `openspecChangeSlug`. S4's handoff adapter reads
  this table via `canon-store`'s Postgres tier (`Tier::read`) — **never**
  the donor monorepo's live hosted Postgres connection (operator rescope directive: S4 never reads
  live donor state).
- **openspec task state**: `tasks.md` rows flip `- [ ] <id> …` →
  `- [x] <id> … — ✅ <evidence>` (the donor CLI's task-flip logic:
  `flipTaskDone`/`flipTaskDefer`/`flipTaskDrop`); a defer/drop rewrites the
  row instead of checking it, carrying a rationale annotation.

## Goals / Non-Goals

**Goals:**
- Four adapters (ledger, divergence, handoff, openspec task state — each
  reading a `canon.yaml`-configured source; the donor consumer repo / prior event store are
  reference sources and fixture origins, never hardcoded live paths) each
  normalizing their source into canon-model events keyed by the S1 join
  spine (`scenario_id`, `handoff_id`, `change_id`/`task_id`).
- A verdict-derivation step applying the design §5 S4 mapping table
  (reproduced in `specs/review-verdict-mapping/spec.md`) to produce
  `{role, polarity, becomes}` verdict records from normalized events.
- Severity/area tags on the source artifact folded into a `regime_key`
  (`<role>/<repo>/<area>/<hash>`, S1) attached to each verdict, so S6/S7 can
  retrieve by the identical key at write and read time.
- Golden-file fixture: a frozen ledger+divergence sample (captured from
  the donor consumer repo as a point-in-time, checked-into-canon fixture corpus — never
  a live read) produces an exact, checked-in verdict-stream JSON, diffed
  byte-for-byte.
- Idempotent re-ingest across all four adapters (S3's digest pattern reused,
  not re-invented).

**Non-Goals:**
- Rewriting or migrating the donor consumer repo's on-disk ledger/divergence format — S4
  adapters read the **current** (pre-migration) Hive shape verbatim; S11 owns
  the in-place migration and ships any adapter-facing compatibility shim if
  the migrated shape changes field names S4 depends on (tracked as an S11
  task, not duplicated here).
- Distillation, promotion, or reward computation over the verdict stream —
  that is S6 (strategy memory) and S7 (reward wiring + statistical
  promotion); S4 stops at emitting the verdict record.
- A generic "any repo's review tool" adapter interface — S4 ships exactly the
  four sources named in design §5 S4; a plugin-style adapter registration
  (S10) is out of scope here, consistent with S3's D1.

## Decisions

**D1 — Ledger adapter reads the Hive layout as-is, including its known gaps.**
The ledger adapter (source root `canon.yaml`-configured, GENERIC — never
hardcoded to a live donor consumer-repo path, S4 foundation rescope) walks
`kind=<kind>/[area=<area>/]*.json`
exactly as `parity.py`'s `_load_ledger`/`_ledger_layout_problem` do today,
including the `run`/`drill` flat-under-`kind=` exception. A record missing
fields S4 needs for verdict derivation (e.g. a `run` record's absent
`actor`/`session_id`, per the S11 audit) still normalizes to an `Event` with
those fields `None` — the verdict is still derivable from `result`/`kind`
alone; S11's later schema upgrade backfills the richer fields without
requiring an S4 adapter rewrite (the adapter reads by field name with
`Option<T>`, not a fixed-arity struct match). Rationale: S4 must ingest the
corpus that exists today, not the one S11 will produce — sequencing S4 before
S11's migration (design §6 wave order: both are W1, parallel) means the
adapter cannot assume the migrated shape.

**D2 — Divergence adapter treats `manifest` and `review`/`remediation` JSONL
line types as distinct event kinds sharing one file.**
Each `.jsonl` file is read line-by-line; a `"type":"manifest"` line becomes a
non-verdict `Event` (round bookkeeping — `reviewed_ids`, `reviewer`, `round`);
a `"type":"review"` line with `"status":"open"` becomes the "design-review
finding" / "code-review finding" row of the verdict mapping table (failure,
guardrail candidate); a `"type":"remediation"` line (post-fix re-review)
followed by a `resolved` status becomes the "remediation + later resolved"
row (success, strategy candidate). Rationale: this is the literal shape on
disk (design §5 S11 audit: "divergences: Hive, richest artifact — structured
aspects, digest anchoring ✓") — no reshaping needed at ingest, only
type-dispatch.

**D3 — handoff adapter keys on the existing `handoffs.id`, never re-derives
a handoff identity.**
The adapter reads **canon's own** `Handoff` table (S1's type,
`crates/canon-model/src/handoff.rs`, wire-compatible with the prior event store's
`handoffs` schema field-for-field) via `canon-store`'s Postgres tier
(`Tier::read`) — **never** a live donor event-store / hosted Postgres connection (operator
rescope directive 2026-07-11) — and emits one `Event` per state transition
it observes (row insert = created, `claimedBy` set = claimed,
`state='done'`/`'abandoned'` = terminal). `handoff_id` in every emitted
event is the table's own `id` column verbatim — S1's join-spine grammar
("existing donor-CLI grammar") is authoritative here, not re-derived. A handoff
reaching `done` with an attached `openspecChangeSlug` is *not itself* a
verdict (a handoff is management plumbing, not a review/CI/merge signal) —
only the four artifact families in the design §5 S4 mapping table produce
verdicts; the handoff adapter's events exist so a verdict from another
adapter can later join to the handoff that carried the work (S1 join spine:
`handoff_id | handoff ↔ session ↔ change`).

**D4 — openspec task-state adapter treats a checkbox flip as a signal only
when it carries evidence.**
A `tasks.md` row flipping `- [ ]`→`- [x] … — ✅ <evidence>` normalizes to an
`Event`, but only feeds the "PR merge" / "CI fail" rows of the verdict table
when the evidence string names a merge/CI outcome the adapter can parse (a
PR URL, a CI run link); a flip with prose-only evidence (no parseable
merge/CI reference) still normalizes to an `Event` for the join spine
(`task_id`) but does not synthesize a verdict — inventing a success signal
from an unverifiable checkbox would violate design §7's "malformed evidence
is no evidence" principle at the verdict layer, not just the ledger layer.
A `**DEFERRED**`/`**DROPPED**` rewrite (the donor CLI's `flipTaskDefer`/
`flipTaskDrop` shape) normalizes to an `Event` with no verdict — deferral is
not a failure signal for the implementing role, only a scheduling fact.

**D5 — Verdict `regime_key` derivation is table-driven from source tags, not
per-adapter logic.**
Every adapter emits its normalized `Event` with whatever severity/area tags
its source already carries (ledger `scenario_id`'s `<area>` component,
divergence `area=`/`surface=` partition keys, PR/CI labels), plus a
content-digest `hash` computed the same way S3's adapters already do
(idempotence, task 6.1). A single shared
`regime_key(role, repo, area, hash) -> String` function (used by all four
adapters, and later reused by S6/S7/S8 verbatim — S1: "canonical regime
keys — same at write+read"; S6 design decision 2: "role leads the tuple,
the primary retrieval axis") derives `<role>/<repo>/<area>/<hash>`.
Rationale: S1 already fixes this as a cross-cutting invariant ("Failure
classes are stable strings… canonical regime keys — same at write+read");
duplicating the derivation per-adapter would risk exactly the write/read
divergence S1 exists to prevent.

**D6 — Source roots are `canon.yaml`-configured, never hardcoded (S4
FOUNDATION rescope, operator directive 2026-07-11).**
The S4 design as originally authored pointed the ledger/divergence adapters
at a hardcoded donor consumer-repo `spec/**` path and the handoff adapter at
the donor monorepo's live hosted Postgres `handoffs` table — both VIOLATE canon's own
don't-read-live-consumer-state posture. The FOUNDATION wave corrects this:
`crate::artifact_adapter::ArtifactSourceConfig` (canon-ingest) is the
generic, `canon.yaml`-sourced configuration surface every adapter resolves
its source from (`ledger_root`/`divergences_root`/`openspec_root`, all
`Option<PathBuf>`, no compiled-in default); `crate::artifact_adapter::ArtifactAdapter`
is the frozen trait (`resolve_source`/`parse`) wave-2's four adapters
implement against; `crate::artifact_adapter::ArtifactSourceHandle` abstracts
over a filesystem root (ledger/divergence/openspec-task) and an
already-fetched record batch (the handoff adapter — `canon-ingest` has no
`canon-store` dependency, so its wave-2 driver resolves the live
`Tier::read` query and hands the rows in, rather than this crate opening
its own connection). `crate::verdict::derive_verdict` (the pure
`{role,polarity,becomes}` mapping) and `canon_model::ids::regime_key` (the
canonical join-key serialization) are both FOUNDATION-shipped, frozen for
wave-2 to call without modification.


## Risks / Trade-offs

- **Risk:** S4 and S11 land in the same wave (W1, parallel per design §6);
  if S11's migration changes a field name the S4 ledger/divergence adapters
  read, the adapter silently stops deriving verdicts for the changed field.
  **Mitigation:** D1's `Option<T>`-by-field-name reading (not fixed-arity
  struct decoding) degrades gracefully to a missing-field skip rather than a
  parse failure; the golden-file fixture (design §8) catches a verdict-count
  regression either way, and S11's own acceptance criterion (parity.py still
  passing after migration) is the second independent check.
- **Risk:** the verdict mapping table conflates "success" for very different
  underlying evidence strength (an agent-reviewed `@reviewed` promotion vs. a
  human `@ratified` one both read as "review-record… success").
  **Trade-off accepted:** S4 emits the verdict with whatever trust-level tag
  the source record carries as a passthrough field on the verdict (not
  collapsed away); S6/S7's statistical promotion (design §7 spec, MaTTS
  pattern) is where trust-weighting of a verdict actually happens — S4 is
  intentionally a thin, literal mapping, not a judgment layer.
- **Risk:** the handoff adapter reads canon's own Postgres-tier state
  (mutable, shared across concurrent ingest runs) and the openspec task
  adapter reads a consumer repo's own `openspec/changes/` tree (mutable,
  team-shared within that repo) rather than a static fixture — a fixture
  corpus must be a frozen snapshot, not a live query, or `canon selftest`
  becomes non-deterministic.
  **Mitigation:** the S4 fixture corpus captures a point-in-time export of
  both sources (a `handoffs` table dump + a `tasks.md` snapshot) exactly as
  S3's fixture captures sanitized transcript samples, never a live
  connection during tests.
