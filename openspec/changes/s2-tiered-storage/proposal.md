## Why

S1 defined *what* gets stored (the twelve record kinds + join spine); canon
still has nowhere to put them. The design's storage decision (decision 5:
"Storage is tiered: git / Postgres / R2") exists because no single tier
fits every record's access pattern — authored specs and promoted strategies
need git's PR review + versioning; in-flight task/handoff state needs
Postgres's low-latency CAS; raw transcripts and analytical marts need R2's
cheap, DuckDB-queryable bulk storage. S2 builds the one `canon-store` trait
and its three adapters so every later spec (ingest, gate, learn, report)
writes/reads through one interface instead of hand-rolling tier-specific
I/O per spec.

## What Changes

- Add `canon-store`: one storage trait with three adapters — git tier
  (Hive-partitioned files under the consumer repo, append-only, layout
  enforced), pg tier (sqlx, hot, team-shared — reusing the donor's existing hosted
  Postgres instance under a `canon_*` schema per §10 Q1), and r2 tier (cold, parquet
  via arrow, DuckLake-compatible layout so the prior session store's marts can
  join).
- Add `TierPolicy` to `canon.yaml`: which record kinds live in which tier,
  aging rules, and digest-based idempotence for tier transitions.
- Add `canon tier age`: moves records from hot (pg) to cold (r2) per
  `TierPolicy`'s aging rules.
- Add `canon query`: fans out across all three tiers and merges results
  into one read path.
- Ship `stg_/int_/mart_` DuckDB views over parquet + git files, following
  the donor parity harness's DuckDB view layering convention (staging → gate-
  equivalent intermediate → persona-facing mart).
- Generalize the donor parity harness's `_ledger_layout_problem` Hive-layout
  enforcement into `canon-store`'s git-tier adapter: a misfiled record
  (wrong directory, wrong filename) is malformed evidence — a layout
  violation, never silently accepted.
- Record §10 Q1 (reuse the donor's hosted Postgres instance + R2 bucket with `canon_*`
  prefixes vs. provision dedicated ones) as a tracked, non-blocking open
  question in this change's tasks.md; this change implements the
  "recommend: reuse with prefixes" default while leaving the question open
  for a later revisit at team scale.

## Capabilities

### New Capabilities

- `tier-adapter-trait`: one storage trait with git/pg/r2 adapters, each
  conforming to the same read/write/age contract.
- `tier-policy`: `canon.yaml`'s `TierPolicy` (kind→tier routing, aging
  rules, digest idempotence) and `canon tier age`.
- `unified-query`: `canon query`'s cross-tier fan-out/merge and the
  `stg_/int_/mart_` DuckDB view layers.
- `git-tier-layout-enforcement`: Hive-partition layout enforcement for the
  git tier, generalized from the donor parity harness's `_ledger_layout_problem`.

### Modified Capabilities

_None — S0/S1 shipped no storage behavior; nothing existing to modify._

## Impact

- New `canon-store` crate (already scaffolded as a stub by S0; this
  change gives it its first real adapters), depending on `canon-model`
  (S1) for every record type it persists.
- New `canon_*`-prefixed Postgres schema on the donor's existing hosted Postgres instance
  (per §10 Q1's default) — no existing donor schema/table is modified.
- New R2 bucket prefix (`canon_*`) alongside the prior session store's existing
  parquet layout, DuckLake-compatible so the donor's marts can eventually join
  canon's without a second ingestion path.
- No changes to the donor parity harness's ledger tooling itself — S2 only generalizes its
  layout-enforcement *pattern*, in preparation for S11's migration of
  the donor parity harness's ledger onto canon-validated storage.
