## Context

S0 scaffolded `crates/canon-store` as a compiling stub; S1 gave
`canon-model` its first real record types. S2 is where those records get a
home. Design decision 5 fixes the tier split; the architecture diagram
(design §4) fixes what lives where:

```
git (repo-local)        Postgres (hot, shared)      R2 (cold, shared)
authored specs          in-flight task/handoff       raw transcripts
promoted strategies     state, trust state,          full event log,
evidence ledger         recent events                parquet marts,
(Hive, append-only)                                  raw trajectories
```

Donors (design §3): the donor parity harness's Hive-partitioned ledger + append-only
discipline + its `_ledger_layout_problem`; the prior session store's
DuckLake shape (PG catalog + R2 parquet) for the cold tier and
the prior event store's Drizzle hot-state shape for the pg tier — explicitly "what NOT
to repeat: two stores without a join key" (S1 already closed that gap;
S2's job is to not reopen it by giving each tier its own untyped ad hoc
schema). The donor parity harness's DuckDB view definitions for the `stg_/int_/mart_`
layering convention.

## Goals / Non-Goals

**Goals:**
- One `canon-store` trait (`Read`/`Write`/`Age` operations) with three
  conforming adapters, so `canon-ingest` (S3), `canon-gate` (S5), and
  `canon-learn` (S6) write tier-agnostic code and only `TierPolicy`
  decides which physical tier a given record kind lands in.
- Git-tier Hive layout enforcement that makes a misfiled record a detected
  violation, generalizing the donor parity harness's `_ledger_layout_problem` from
  its ledger-specific four kinds to canon's full closed
  record-kind set (S1).
- `TierPolicy` in `canon.yaml` driving both initial placement and aging
  (`canon tier age`), with digest-based idempotence so re-running aging on
  already-aged records is a no-op, not a duplicate write.
- `canon query` as one fan-out/merge read path, and `stg_/int_/mart_`
  DuckDB views as a read-only query convenience mirroring
  the donor parity harness's three-layer convention — never a second source of
  truth (the views read the tiers; they do not cache or duplicate them).

**Non-Goals:**
- Choosing LanceDB vs. parquet-only for the cold trajectory store — that's
  §10 Q2, explicitly deferred to S6's design.
- Provisioning a dedicated Postgres instance or R2 bucket for canon. §10
  Q1 is recorded as an open question (this change's tasks.md) with the
  design's own recommended default (reuse the donor's hosted Postgres + R2 with `canon_*`
  prefixes) implemented now; a dedicated-infra migration is a future,
  separate change if the team ever revisits the question.
- Real-time collaboration/locking beyond what the tiers give for free
  (design §11 non-goal, inherited unchanged here).
- Building `canon-ingest`'s adapters (S3) or `canon-gate`'s trust spine
  (S5) — S2 only guarantees they will have a tier-agnostic store to write
  through.

## Decisions

**D1 — `canon-store` exposes one trait, not three tier-specific APIs
callers branch on.** `trait Tier { fn write(&self, record: &dyn
StoredRecord) -> Result<WriteReceipt, StoreError>; fn read(&self, query:
&TierQuery) -> Result<Vec<RawRecord>, StoreError>; fn age(&self, policy:
&AgingRule) -> Result<AgeReport, StoreError>; }`, implemented by `GitTier`,
`PgTier`, `R2Tier`. `TierPolicy` (D3) picks which `Tier` impl handles a
given record kind at write time; callers never match on tier by name.
Rationale: this is what makes `canon-ingest`/`canon-gate`/`canon-learn`
tier-agnostic — a kind's tier assignment is a policy-file change (design
§S5's D7 pattern — "tightening coverage is a policy diff, never a corpus
retag" — applied here to storage routing instead of trust requirements),
never a call-site rewrite.

**D2 — The git tier enforces Hive layout as a hard write-time and
read-time check, generalizing `_ledger_layout_problem`.** The donor parity
harness's function switches on
`run|drill` (flat, no `area=`) vs. `review|design-review|code-review|clear`
(nested `area=<area>/<scenario_id>.json`) and returns a `str | None`
problem description. `GitTier`'s layout check generalizes this switch
over canon's full closed kind set (S1 D1): every kind declares its own
partition template (a subset take `kind=<kind>/<file>.json`, area-scoped
kinds take `kind=<kind>/area=<area>/<id>.json`, matching the donor parity harness's
two shapes) via a `partition_template()` method next to each record
type's definition in `canon-model` (so the layout rule lives beside the
type it governs, not duplicated in `canon-store`). A file at the wrong
path for its kind is malformed evidence — `canon_model::validate_evidence`
(S1 D6) returns a `FailureClass::Malformed` violation with a `layout`
subclass, mirroring `_ledger_layout_problem`'s exact "misfiled = malformed"
verdict; it is never silently reparsed at the "correct" implied location.

**D3 — `TierPolicy` lives in `canon.yaml`, is declarative, and drives both
placement and aging from one source.** Shape:
```yaml
tiers:
  git: { root: canon/ledger }
  pg:  { dsn_env: CANON_PG_DSN, schema: canon_v1 }
  r2:  { bucket_env: CANON_R2_BUCKET, prefix: canon/ }
routing:
  evidence-record: git
  strategy-item: git
  handoff: pg
  session: pg
  event: pg
  trajectory: r2
aging:
  handoff: { after: 30d, to: r2 }
  event:   { after: 7d,  to: r2 }
```
`canon tier age` reads `aging` entries, selects records past their
threshold in the `pg` tier, writes them to the `r2` destination keyed by a
content digest (SHA-256 of the canonical serialized record), and only then
deletes the pg row — the digest is checked before every write so re-running
`canon tier age` on already-aged records is a no-op (idempotent), never a
duplicate r2 object. Rationale for declarative-YAML over code: this is the
exact "policy diff, never a corpus retag" shape design §S5 (D7) establishes
for trust requirements; S2 reuses it for storage routing so both concerns
share one mental model for consumer-repo operators editing `canon.yaml`.

**D4 — `canon query` fans out per-tier and merges by record identity, not
by a federated SQL layer.** `canon query --kind <k> [--since <t>]` resolves
`<k>`'s tier(s) from `TierPolicy` (a kind may have already-aged records
split across pg and r2), issues each tier's native read (`GitTier` globs +
parses, `PgTier` runs a `sqlx` query, `R2Tier` scans parquet via `arrow`),
and merges results ordered by `at` — no cross-tier JOIN is attempted inside
`canon query` itself; that's what the DuckDB views (D5) are for when a
caller genuinely needs relational joins across kinds/tiers.

**D5 — The `stg_/int_/mart_` DuckDB views are read-only query convenience
over the same physical files/exports the tiers already produce, layered
exactly like the donor parity harness's SQL view layering.** `stg_*` views read raw sources
(`read_text`/`read_parquet` glob over the git tier's Hive files and the
r2 tier's parquet exports — the r2 tier's DuckLake-compatible layout,
design §S2, means these globs are the same shape the prior session store's marts
already read); `int_*` views mirror `canon-gate`'s (S5) derivation logic in
SQL so the dashboard (S9) never contradicts the gate, exactly as
the donor parity harness's header states for its own `int_*` layer ("SQL mirrors
of the donor parity harness's helpers"); `mart_*` views are the persona-facing
surface. These views never write back to a tier — `canon fmt`/`canon
migrate`/tier writes are the only sanctioned mutators (design §7).

**D6 — §10 Q1 default (reuse hosted Postgres + R2 with `canon_*` prefixes) is
implemented now, tracked as an open question, not silently closed.** The
design doc explicitly recommends reuse ("Recommend: reuse with prefixes;
revisit at team scale") while leaving it open. S2 implements the
recommendation (`PgTier`'s `schema: canon_v1` default, `R2Tier`'s
`prefix: canon/` default) so the change is unblocked, and records the
question verbatim in this change's tasks.md as an OPEN QUESTION row (not a
resolved decision) so a future team-scale revisit is not mistaken for
something S2 already litigated.

**D7 — Tier-aging predicates (D3's `aging:` block) MAY be expressed as CEL
via S13's `canon-policy` crate once S13 lands; the static `{after, to}` map
shown in D3 remains the permanent fallback, never removed.** A consumer
repo that needs a richer aging rule than a flat duration
(`record.kind == "run" && age_days(record.at) > 30`, design doc §5 S13's
own worked example) gets that expressiveness by upgrading one `aging`
entry to a CEL predicate string, evaluated by `canon-policy`'s same
bindings `canon fmt`/`canon context` use (S13 invariant: no second
registration site) — `canon tier age` itself does not grow a second aging
mechanism; it dispatches to CEL only when an entry is a predicate string,
else the existing static comparison.

## Risks / Trade-offs

- [Risk] Reusing the donor's hosted Postgres instance (D6) couples canon's hot-tier
  availability to the donor's database's operational health. → Mitigation: this
  is the explicit, tracked trade-off in §10 Q1; `canon.yaml`'s `dsn_env`
  indirection (D3) means swapping to a dedicated instance later is a
  config change, not a code change.
- [Risk] Per-kind `partition_template()` (D2) living in `canon-model`
  creates a dependency from `canon-model` (S1, storage-agnostic types) to
  storage layout concerns. → Mitigation: `partition_template()` returns a
  pure, storage-agnostic path template string
  (`"kind={kind}/area={area}/{id}.json"`); `canon-model` still never
  imports `canon-store` or knows about git/pg/r2 — only `GitTier`
  interprets the template against a filesystem.
- [Risk] `canon tier age`'s digest-based idempotence needs canonical
  serialization (stable field ordering) to be a reliable dedup key. →
  Mitigation: reuse `canon-model`'s serde `Serialize` impl with
  `serde_json`'s (or a canonical-JSON crate's) deterministic key ordering,
  the same serialization every schema-export/round-trip test (S1) already
  exercises.
- [Trade-off] `canon query`'s per-tier fan-out (D4) does not JOIN across
  kinds — a caller needing e.g. "sessions joined to their trajectories"
  must use the DuckDB views (D5), not `canon query` directly. Accepted:
  keeps the Rust query path simple and fast for the common single-kind
  case; the views exist precisely for the relational case.
- [Risk] The donor parity harness's SQL-layering donor (D5) is easy to misread as also
  endorsing DuckDB's `hive_partitioning=true` read option for the git
  tier's Hive-laid-out files — it does not. A repo-wide grep of
  the donor parity harness's `spec/`/`tools/` finds ZERO invocations of
  `hive_partitioning=true`; `stg_ledger_records` instead globs
  `read_text('spec/ledger/**/*.json')` and derives `kind`/`area` from the
  JSON payload itself (`coalesce(j ->> '$.kind', 'run')`), trusting
  content over path, with a SEPARATE static pass (not a DuckDB read
  option) enforcing that a record's directory agrees with its own
  payload-derived kind/area. → Mitigation: D2's layout enforcement and
  D5's `stg_*` views MUST follow the donor's ACTUAL mechanism —
  content-trusted column extraction (with a `run`-kind default for
  area-less records) plus D2's independent layout-conformance check — not
  DuckDB's `hive_partitioning=true` flag, which the donor never validated
  end-to-end; if canon ever wants real `hive_partitioning=true`
  column-pruning, that is a deliberate, separately-evaluated choice, not
  an inherited default.

## Migration Plan

No existing canon-tier data to migrate (S0/S1 shipped no persisted
records). Provisioning the `canon_v1` Postgres schema and the `canon/` R2
prefix are additive to the donor's existing hosted Postgres instance/R2 bucket — no
existing donor table or object is modified or moved. Rollback is dropping
the `canon_v1` schema and the `canon/` R2 prefix; nothing external depends
on them before this change ships (S3+ are the first consumers).

## Open Questions

- ~~§10 Q1 (verbatim): "Reuse the donor's hosted Postgres instance + R2 bucket with
  `canon_*` prefixes, or provision dedicated ones? (Recommend: reuse with
  prefixes; revisit at team scale.)"~~ **RESOLVED (2026-07-10, production
  evidence):** the donor already runs two independently-owned table sets on
  ONE hosted Postgres instance — the donor's Drizzle config's
  `tablesFilter: ["!ducklake_*"]` lets DuckLake's ~38 catalog tables and
  the prior event store's application tables coexist in one `public` schema by
  name-prefix filtering — AND at the infra level, the prior session store's CronJobs
  read the SAME database-url K8s Secret key the prior event store's own
  app-server pods use. Reuse-with-`canon_*`-prefixes (D6) is therefore the PROVEN
  pattern, not just the recommended one — implemented per D6, and no longer
  tracked as open.
