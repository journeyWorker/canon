# tiered-storage

> How canon-store's storage rungs (local/hot/cold) and their vendor backends (git/postgres/sqlite/s3) work — adding a record kind's routing/aging rule to canon.yaml, persisting/querying/aging records through TierRegistry, and reading the stg_/int_/mart_ DuckDB views. Use when touching crates/canon-store, editing canon.yaml's tiers/routing/aging section, or querying canon's stored records.

# tiered-storage

`canon-store` (`crates/canon-store`, S2) is the ONE storage trait —
`Tier`, implemented by FOUR vendor backend adapters (`GitTier`/
`PgTier`/`SqliteTier`/`R2Tier`, each reporting its own identity via
`Tier::backend() -> Backend`) — every later spec (`canon-ingest`,
`canon-gate`, `canon-learn`, …) writes and reads through.
`canon.yaml`'s `routing`/`aging` sections name a capability **rung**
(`Rung::{Local, Hot, Cold}` — canon's storage ladder role:
local diffable files → hot live-queryable state → cold bulk
archive), never a vendor backend directly; which `Backend`
(`Git`/`Postgres`/`Sqlite`/`S3`) currently implements a rung is a
SEPARATE declaration, `tiers.<rung>.backend` (s27
`tier-role-backend-split`, design D1). `postgres` and `sqlite` are
BOTH `LiveDb`-class backends (s28 `rung-backend-capability`) — either
one satisfies the `hot` rung's class check; see "Backends and their
config" below for the full rung/backend/config picture (s32
`sqlite-hot-backend`). This skill covers extending `canon.yaml`'s
`TierPolicy`, the library entry points those specs' own CLI commands
eventually wrap, and how to read the DuckDB query convenience layer.

## Backends and their config

| backend    | rung class            | `tiers.<rung>` config                  | credentials / network                                    |
| ---------- | ---------------------- | --------------------------------------- | ---------------------------------------------------------- |
| `git`      | local file (`Rung::Local`, today) | `root:` (relative to canon.yaml dir)   | none — zero network                                        |
| `postgres` | `LiveDb`               | `dsn_env:` (env var naming a DSN) + `schema:` | live network + credentials; degrades loud if unreachable   |
| `sqlite`   | `LiveDb`               | `path:` (relative to canon.yaml dir)    | none — no env-var indirection at all (s32); a local db file carries no secret |
| `s3`       | `BulkArchive`          | `bucket_env:` + `prefix:`               | live network + credentials; degrades loud if unreachable   |

`sqlite` is the ZERO-dependency `hot` default `canon init` scaffolds
(WAL journal mode + a busy timeout applied at connect, covering
concurrent BATCH ingest from a single operator) — it is still a
**single-writer store**: heavy multi-agent concurrent writers should
swap the `hot` rung to `postgres` instead (same `LiveDb` class, a
one-block canon.yaml swap — `canon init`'s scaffold ships the
postgres stanza commented right beside the live sqlite one for
exactly this upgrade path).

## Adding a record kind's tier routing (or changing one)

`canon.yaml`'s `routing:` map is the ONLY place a kind's rung is
decided (tier-policy spec — no `canon-store` caller ever branches on a
literal kind name). Keys are `RecordKind::as_str()`'s stable snake_case
wire strings (`change`, `evidence_record`, `strategy_item`, …) — the
SAME string the record's own `kind` field already serializes to, not a
second kebab-case vocabulary (the design doc's own `canon.yaml`
illustration uses kebab-case as prose sugar only; `canon-store`'s
`policy` module doc explains why the shipped config reuses the tested
wire string instead). Values are one of the three capability rungs —
`local`/`hot`/`cold` — never a backend name.

```yaml
routing:
  scenario: local   # change this to move future `scenario` writes
```

Changing a routing value moves FUTURE writes with **no `canon-store`
source change required** (tier-policy spec's own acceptance scenario) —
`TierRegistry::persist`/`query`/`age_all` re-resolve the policy on every
call. `TierPolicy::from_yaml` fails loud (a config parse error, not a
panic or silent default) on an unknown kind name or an unknown rung
name. A legacy backend name (`git`/`pg`/`r2`) used anywhere a rung is
expected — `routing.*`, `aging.*.to`, or a `tiers.*` key — is REJECTED
with an explicit hint, never silently aliased (`Rung::parse`, s27
design D3):

```
canon.yaml TierPolicy: `git` is a BACKEND name, not a rung — canon.yaml's
`routing`/`aging`/`tiers` keys now name a capability rung
(local/hot/cold); declare the backend separately via
`tiers.<rung>.backend: git`
```

Every one of the twelve record kinds needs a `routing:` entry — an
unrouted kind is a hard `StoreError::UnroutedKind` at the FIRST
write/query attempt for it, never a silently-dropped record.

## Adding/changing an aging rule

`aging:` moves records from their routed rung to a `to:` rung once
`at` exceeds `after`:

```yaml
aging:
  handoff: { after: 30d, to: cold }   # <n>d/h/m/s — one integer, one unit
```

`canon tier age`'s backing implementation is
`TierRegistry::age_all()` — it runs every `aging:` entry once, resolving
BOTH the source rung (from `routing:`) and the destination
(`aging.*.to`) live handles, then calls `Tier::age` on the source
handle. This is content-digest-idempotent (tier-adapter-trait spec):
re-running `age_all()` immediately after a full run finds nothing left
in the source rung to re-select (`AgeReport.moved == 0`); re-running
after a partially-completed run (destination write succeeded, source
delete didn't) re-selects the same record but the destination's
digest-keyed write is a no-op (`AgeReport.already_aged` counts it,
`moved` does not).

A kind currently split across two rungs (some records aged, some not)
is still read correctly — see the next section.

## Persisting and querying records

`TierRegistry` is the entry point (not a tier struct directly):

```rust
// Each backend handle is `Option<_>` — `None` when that rung isn't
// configured, or is configured for a different backend, or the CLI
// simply didn't attach it. `TierRegistry::new` resolves each
// `tiers.<rung>` entry to whichever of the four handles its own
// `backend:` tag names — local/hot/cold are NOT statically pinned
// to git/postgres/sqlite/s3 (design D1's own "not a type-level
// constraint" caveat).
let registry = TierRegistry::new(policy, Some(git_tier), Some(pg_tier), Some(r2_tier), Some(sqlite_tier));

// The common local-first shape: only a git-backed `local` rung is
// configured, zero network needed.
let registry = TierRegistry::new(policy, Some(git_tier), None, None, None);

// Generic, ergonomic write — resolves T::KIND's rung from routing:.
registry.persist(&some_change_record)?;

// canon query --kind <k> [--since <t>]'s backing implementation: fans
// out across BOTH a kind's routed rung AND its aging destination (if
// any), merges by `at`, no cross-tier JOIN.
let result = registry.query(&TierQuery::kind(RecordKind::Handoff).since(cutoff))?;
for violation in &result.violations { /* canon gate (S5) reports these */ }

// canon tier age's backing implementation.
let reports = registry.age_all()?;
```

`GitTier`/`PgTier`/`SqliteTier`/`R2Tier` can also be used directly for
tier-specific needs (`GitTier::migrate_write`, the sole sanctioned
append-only exception `canon migrate` — S11 — will use, or
`TierRegistry::git()` for the same live git handle already wired in —
kept as a dedicated accessor regardless of which rung(s) actually
route to it), but a caller outside `canon-store` should go through
`TierRegistry`, never call e.g. `GitTier::write` directly when a kind
might route elsewhere later.

CLI subcommand wiring (`canon tier age`, `canon query`) into
`canon-cli` shipped in commit `5aa36920` — see the next section for
real invocations. `crates/canon-cli/src/tiers.rs` is the CLI-only glue
that resolves, for each rung `canon.yaml`'s `tiers:` section declares,
which backend implements it, builds the matching live
`GitTier`/`PgTier`/`SqliteTier`/`R2Tier` handle, and hands the set to this crate's
own `TierRegistry`/`Tier` API above.

## Running `canon tier age` / `canon query`

Both subcommands take `--canon-yaml <path>` (default `canon.yaml` in
the current directory) and build their tier handles through
`crates/canon-cli/src/tiers.rs::build_tiers` — the same `TierPolicy`
the sections above cover.

```bash
# Preview what would move (read-only; writes/deletes nothing).
canon tier age --dry-run

# Apply every `aging:` rule once — the destructive move+delete this
# skill's "Adding/changing an aging rule" section describes.
canon tier age

# Fan out a kind's read across every rung it may currently live in
# (its routed rung AND its aging destination, if any) and merge by
# `at`. `--kind` is a `RecordKind::as_str()` wire string — the SAME
# `routing:`/`aging:` vocabulary above (`change`, `evidence_record`,
# `strategy_item`, …).
canon query --kind handoff --since 2026-06-01T00:00:00Z

# Machine-readable output (the merged record bodies) instead of the
# default human table.
canon query --kind trajectory --json
```

`canon tier age`'s move+delete only ever runs against a genuinely
attached rung: `canon-cli` never silently substitutes a local
filesystem for a `canon.yaml`-configured s3-backed rung (e.g.
`tiers.cold`) in a release binary, even with a stray
`CANON_R2_LOCAL_ROOT` env var (the offline integration-test seam
`crates/canon-cli/tests/support::Fixture` uses) set — that override is
compiled in ONLY under `cargo build`'s default (non-`--release`)
profile; a release `canon` binary always attaches a real bucket and
fails loud if it can't (see "Local-first" below).

## Local-first: PgTier/SqliteTier/R2Tier need attachment, GitTier never does

`GitTier` is always available (an ordinary local directory — whichever
rung's `tiers.<rung>` block tags `backend: git`; `local`, by
today's convention: `canon.yaml`'s `tiers.local.root`).
`PgTier::connect(dsn, schema)` and `R2Tier::connect_live(bucket_env,
prefix)` require live credentials/network and FAIL LOUD if invoked
without them (never a silent fallback to git — an explicitly-configured
tier that can't attach is a startup-time hard failure, a contract
inherited from a prior session/event store's storage audit). `SqliteTier::connect(path)`
needs no credentials or network (s32 `sqlite-hot-backend`) — it opens
or creates a local db file (creating missing parent dirs), applies WAL
journal mode + a busy timeout, and still FAILS LOUD the same way on a
genuinely unopenable/corrupt path (permissions, a broken parent
component), never a silent fallback either. A `TierRegistry` built
with `pg: None, r2: None, sqlite: None` and a `canon.yaml` where every
configured `tiers.<rung>` is `backend: git` works with ZERO network
(design §9) — the common OSS-consumer-repo shape before `canon init`
scaffolded a live `hot` rung by default (s32: now sqlite, still zero
network).

For local integration testing of `PgTier` without cloud credentials,
`crates/canon-store/tests/support::LocalPg` spins up a genuinely local,
unix-socket-only, ephemeral Postgres cluster via `initdb`/`pg_ctl` (if
present on `PATH`) — distinct from the `live-pg` Cargo feature, which
gates tests against a real hosted/cloud Postgres instance.

## Reading the DuckDB views (`sql/views.sql`)

Layered `stg_`/`int_`/`mart_`, mirroring a consumer repo's
`spec/specdb.sql` convention (design D5):

- `stg_git_records` / `stg_r2_records` / `stg_records` — thin,
  CONTENT-TRUSTED extraction (`kind`/`at`/`scenario_id` come from each
  record's own JSON/parquet body, never `hive_partitioning=true` or a
  path-derived column — the git/r2 Hive directory layout is
  layout-ENFORCED separately, by canon-store's Rust code, not by
  DuckDB's read options). `stg_records` is exhaustively
  `stg_git_records UNION ALL stg_r2_records`, no third source: it
  scans the git ledger plus whatever local `canon/r2` mirror happens
  to exist at `CANON_R2_ROOT` — Postgres has ZERO SQL view here
  (`canon-report` never opens a live DB connection). See "What `canon
  report` reflects" below for which routed kinds this actually
  surfaces.
- `int_evidence_verdicts` — currently a STUB (a plain tally, not a
  `canon-gate` mirror — S5 hasn't shipped yet). Replace its body
  wholesale, don't extend it, once S5 ships the real derivation to
  mirror.
- `mart_records_by_kind` — persona-facing.

Roots are rebindable via `CANON_GIT_ROOT` / `CANON_R2_ROOT` env vars
(the parity-harness D17 `GateCtx`-equivalent fixture-rebinding pattern):

```bash
CANON_GIT_ROOT=/path/to/canon/ledger \
CANON_R2_ROOT=/path/to/local/r2/root/canon \
  duckdb -init crates/canon-store/sql/views.sql -c "SELECT * FROM mart_records_by_kind;"
```

Note `CANON_R2_ROOT` must point INSIDE the s3-backed rung's configured
prefix (`tiers.cold.prefix` by today's convention, default `canon/`)
— i.e. the directory that directly contains `kind=<kind>/`
subdirectories, not the bucket/local root the prefix is relative to.

These views never write back to a rung (design D5) — `canon fmt`/
`canon migrate`/tier writes are the only sanctioned mutators.

## What `canon report` reflects (the "read directly" boundary)

`canon report`'s marts (`mart_*` above) are built exhaustively on
`stg_records`, which reads ONLY `canon-report`'s own local roots — the
git ledger and a LOCAL `canon/r2` parquet directory (s28
`rung-backend-capability` design D2, correcting s27's
`Backend::offline_file_readable()`, which wrongly counted S3 as
report-visible: `canon tier age` writes cold/S3 records to the LIVE
bucket, not to local `canon/r2`, so an S3-routed kind's data appears
in the report only if a local mirror is separately materialized — no
automatic sync exists today). A kind routed to a rung whose backend
is not `Backend::read_directly_by_report()` — postgres/sqlite always
(`hot`, by today's convention — `task`, `handoff`, `session`, `run`,
`event` in this repo's own `canon.yaml`; sqlite is `hot`'s
zero-dependency default since s32 `sqlite-hot-backend`), and s3/cold unless a local
`canon/r2` mirror happens to be current — is NOT lost: it stays
reachable exclusively through `canon query --kind <kind>` (s22
`query-tier-degradation`) — `canon report`'s offline/deterministic
rendering path never opens a live DB connection or the live bucket
(`crates/canon-report/src/tier_boundary.rs`).

This gap is named loud, not silently under-counted:
`tier_boundary::kinds_not_read_directly` re-parses `canon.yaml`'s
`routing`/`tiers` tables (a pure YAML parse, no socket) and, whenever
that set is non-empty, both (a) renders a `## Kinds not read directly`
section in the report body naming every affected kind, and (b) emits a
matching stderr `WARN` — one derivation feeds both, so they can never
disagree. A malformed or absent `canon.yaml` degrades to an empty set
(fail-soft), never a `canon report` panic or hard failure.
