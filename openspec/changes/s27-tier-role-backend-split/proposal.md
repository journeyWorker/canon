## Why

`canon-store`'s tier vocabulary conflates ROLE and BACKEND. `TierKind {
Git, Pg, R2 }` (`crates/canon-store/src/policy.rs:24-28`) is BOTH the
name of a capability rung on canon's storage ladder (local files →
hot queryable state → cold bulk archive, `openspec/changes/
s2-tiered-storage/design.md` §4's own architecture diagram) AND the
name of the vendor product that currently implements that rung. Every
`canon.yaml` names a *backend* where it should name a *capability
rung*: `routing: task: pg`, `aging: handoff: { to: r2 }`. A repo cannot
say "task data lives in a hot, live-queryable store" without also
committing to Postgres specifically; swapping the hot tier's vendor (a
different managed Postgres, or eventually a different hot-store
technology entirely) is unrepresentable as a config change — it would
require renaming `TierKind::Pg` itself, i.e. a source change to every
one of its call sites.

**Blast radius, measured (not estimated):** `TierKind` appears in 10
Rust source files (`canon-store` 6, `canon-cli` 3, `canon-report` 1;
`canon-model` has zero occurrences — the conflation is fully contained
to the storage/CLI layer, never leaked into the record types
themselves), 38 total variant uses, and 6 bare `"pg"`/`"r2"` string
literals outside the enum's own `as_str`/`parse`. Grepping
`crates/**/*.rs` for the YAML `routing:` key alone (test fixtures
that write a `canon.yaml` inline) finds 26 files carrying the git/pg/r2
vocabulary as literal config text — the real day-to-day surface a
consumer-repo operator or a canon test author encounters. The three
`target/usage-review/*/canon.yaml` dogfood dummies (loom, eno-drift,
najun-art-dummy) are live, out-of-tree instances of the same shape.

**Second, structural symptom of the same conflation (s25's own
finding, generalized):** `canon-report`'s DuckDB marts exclude the `pg`
tier not because Postgres is special, but because a LIVE DATABASE
CONNECTION breaks the report's offline/deterministic/drift-checkable
contract (S9 decision 11) — `crates/canon-report/src/tier_boundary.rs`
(s25 `report-pg-tier-boundary`) hardcodes this as `TierKind::Pg`
specifically, when the actual boundary is a BACKEND CAPABILITY
("can this backend's data be read via a plain file scan, with no live
connection?" — git's Hive JSON files and S3's parquet exports: yes;
Postgres: no). Today that capability and the `Pg` tier name happen to
coincide 1:1, so the bug is latent — but the moment a `cold` rung is
ever backed by something live-queryable, or a `hot` rung by something
file-scannable, `pg_routed_kinds`'s literal `TierKind::Pg` filter
silently answers the wrong question.

This change is a FOUNDATIONAL revision of S2's tier model: it splits
the ROLE canon.yaml's `routing`/`aging` sections declare (a
`Rung`: `local` / `hot` / `cold`) from the BACKEND that currently
implements each rung (`Backend`: `git` / `postgres` / `s3`), and
re-derives every backend-conditioned decision (report inclusion,
CLI error naming, lenient-attach scoping) off the correct axis —
BACKEND CAPABILITY for report inclusion, RUNG for everything else.

## What Changes

- **New `Rung` enum** (`Local`, `Hot`, `Cold`) replaces `TierKind`
  as the vocabulary `canon.yaml`'s `routing`/`aging` sections and every
  `canon-store`/`canon-cli` routing decision speak in. Rungs are the
  capability ladder S2's own design already named in prose (§4's
  diagram) — this change makes it the literal enum, not a comment.
- **New `Backend` enum** (`Git`, `Postgres`, `S3`) is the vendor-name
  home `TierKind` used to be. `canon.yaml`'s `tiers.<rung>.backend`
  key selects it; `Backend::offline_file_readable()` is the ONE
  capability method every backend-conditioned decision in the
  codebase now reads instead of matching on a literal name.
- **`canon.yaml`'s `tiers:` section rekeys from backend name to rung
  name, and each entry gains an explicit `backend:` tag:**
  ```yaml
  tiers:
    local: { backend: git,      root: canon/ledger }
    hot:       { backend: postgres, dsn_env: CANON_PG_DSN, schema: canon_v1 }
    cold:      { backend: s3,       bucket_env: CANON_R2_BUCKET, prefix: "canon/" }
  routing: { task: hot, change: local, handoff: hot }
  aging:   { handoff: { after: 0s, to: cold } }
  ```
  `routing`/`aging.*.to` values become rung names (`local`/`hot`/
  `cold`), never a backend name. The per-backend config field sets
  (`GitTierConfig{root}`, `PgTierConfig{dsn_env,schema}`,
  `R2TierConfig{bucket_env,prefix}`) are UNCHANGED — they move under a
  rung key plus a `backend:` discriminant, they are not redesigned.
- **`canon report`'s tier-boundary computation rekeys from `TierKind
  ::Pg` to `!Backend::offline_file_readable()`.** The marts include
  every kind whose routed rung's CONFIGURED BACKEND is
  offline-file-readable (git, S3); the `## Tiers not reflected`
  section + stderr `WARN` name every kind whose routed rung's backend
  is NOT (Postgres today, but the derivation is backend-capability-
  keyed, not tier-identity-keyed, so it stays correct under ANY future
  rung↔backend assignment — a `cold` rung ever backed by Postgres
  would be excluded; a `hot` rung backed by S3 would be included).
- **`TierRegistry` rekeys from three named backend fields to three
  named RUNG fields** (`local`/`hot`/`cold`), each holding whichever
  backend adapter (`GitTier`/`PgTier`/`R2Tier`) `canon.yaml` configured
  for that rung. `canon query`'s tier-unavailable error names the RUNG
  plus, when known, the BACKEND (e.g. "hot tier (postgres) is not
  attached (no live DSN)") instead of a bare backend name.
- **HARD MIGRATION, no backward-compatibility alias.** `TierPolicy::
  from_yaml` accepts ONLY the new rung/backend shape. A legacy `git`/
  `pg`/`r2` value used where a rung is expected (`routing.<kind>`,
  `aging.<kind>.to`, or a `tiers:` top-level key) is a loud parse
  error naming the rung vocabulary — never silently accepted, never a
  deprecation-warning-then-still-works path. Every `canon.yaml` in the
  tree — every Rust test fixture and the `target/usage-review/loom/
  canon.yaml` live dogfood dummy — is rewritten to the new shape as
  part of this change's implementation phase.
- **The s22 `query-tier-degradation`/`uniform-lenient-tier-build`
  lenient-attach machinery (`crates/canon-cli/src/tiers.rs`) rekeys to
  `Rung`** — `build_lenient_tiers_for_kind`'s "attach only what this
  kind's read fan-out needs" and `build_lenient_tiers`'s "attempt
  every declared tier, each independently lenient" contracts are
  UNCHANGED in substance; only the key type (`Rung` instead of
  `TierKind`) and the concrete `canon.yaml` paths its scenarios cite
  change.

### Capabilities

- **ADDED** `tier-role-backend-split`: the `Rung`/`Backend` split
  itself — the new `canon.yaml` shape, the loud hard-migration parse
  behavior, and the cross-cutting non-regression guarantee (`canon
  gate check` byte-identical).
- **MODIFIED** `tier-policy` (s2 `tiered-storage`): `routing`/`aging`
  now declare rungs, not backends; `canon.yaml`'s `tiers:` section is
  keyed by rung with an explicit `backend:` tag.
- **MODIFIED** `query-tier-degradation` (s22): the tier-unavailable
  error names rung + backend; kind-scoped lenient attachment is
  computed over `Rung`, not `TierKind`.
- **MODIFIED** `uniform-lenient-tier-build` (s22): the shared per-
  backend attach-or-degrade core is unchanged in shape; its scenarios'
  concrete `canon.yaml` paths move from `tiers.pg.*`/`tiers.r2.*` to
  the rung-keyed equivalents (e.g. `tiers.hot.schema`).
- **MODIFIED** `report-pg-tier-boundary` (s25): the boundary
  derivation rekeys from tier-identity (`TierKind::Pg`) to backend
  capability (`!Backend::offline_file_readable()`) — the observable
  markdown section and stderr line keep their exact wording and
  placement; the difference is invisible for today's git/postgres/S3
  pairing and only diverges under a future non-default rung↔backend
  assignment.

### Explicit non-goals

- **No new backend.** Still exactly git/postgres/S3 — this change
  renames and re-homes the vocabulary, it does not add a fourth
  storage technology.
- **No tier-adapter read/write/age logic change.** `GitTier`/`PgTier`/
  `R2Tier`'s internal implementations are untouched; they become
  *backend implementations* selected by a rung's `backend:` tag,
  never rewritten.
- **No pg → report materialization.** The report still excludes
  live-database-backed rungs entirely — this change only names that
  boundary by the correct axis (backend capability); a future opt-in
  "materialize the hot rung for reporting" (a live snapshot/replica
  read) is explicitly out of scope.
- **No change to the closed 12-`RecordKind` set** (`canon-model`
  untouched).
- **connector-never-authority preserved: `canon gate check` is
  byte-identical.** `canon-gate` reads nothing from `canon-store`'s
  tier vocabulary or `canon-report`'s boundary derivation; this change
  touches no `canon-gate` source file.
- **`canon report` stays offline/deterministic/drift-checkable.** No
  live Postgres connection is added anywhere in `canon-report`'s or
  `canon-store`'s DuckDB view layer.
- **DuckLake is NOT introduced.** The cold rung remains plain
  object-store parquet, read via DuckDB's `read_parquet` — never a
  DuckLake catalog. This change does not touch how the cold rung's
  data is written or read, only what it is called.
- **No backward-compatibility shim.** Per operator directive: canon
  has never been pushed/released, so there is nothing external to
  preserve — no alias layer, no deprecation warning that still parses
  the old shape, no dual-vocabulary transition period.

## Impact

- **`crates/canon-store/src/policy.rs`**: `TierKind` → `Rung` +
  `Backend`; `GitTierConfigRaw`/`PgTierConfigRaw`/`R2TierConfigRaw` →
  one `BackendConfigRaw` enum tagged on `backend:`; `TiersRaw`
  (`HashMap<String,GitTierConfigRaw|…>` three-field struct) →
  `HashMap<String, serde_yaml::Value>` keyed by rung name, decoded in
  two validated steps (rung-key parse, then backend-tag parse) so a
  legacy `tiers.git`/`tiers.pg`/`tiers.r2` key fails loud with a rung-
  vocabulary hint rather than an opaque serde error; `TierPolicy`'s
  `git`/`pg`/`r2` fields → `tiers: HashMap<Rung, BackendConfig>`;
  `routing`/`aging.*.to` retype from `TierKind` to `Rung`.
- **`crates/canon-store/src/registry.rs`**: `TierRegistry`'s `git`/
  `pg`/`r2` fields → `local`/`hot`/`cold`; `handle(TierKind)` →
  `handle(Rung)`; `tiers_for_read`/`query`/`age_all` rekey
  correspondingly; `StoreError::TierUnavailable{tier: TierKind, ..}` →
  `{rung: Rung, backend: Option<Backend>, reason}`.
- **`crates/canon-cli/src/tiers.rs`**: `build_tiers`/
  `build_lenient_tiers`/`build_lenient_tiers_for_kind`/
  `tiers_needed_for`/`read_tier`/`LoadedTiers`/`LenientTiers` all
  rekey from `TierKind` to `Rung`; `attach_pg`/`attach_r2` are renamed
  to name the BACKEND they attach (`attach_postgres`/`attach_s3`),
  called once per rung whose configured backend matches.
- **`crates/canon-cli/src/tier.rs`, `src/query.rs`, `src/init.rs`,
  `src/context.rs`, `src/main.rs`, `src/plans.rs`, `src/ingest.rs`**:
  every `canon.yaml` literal/scaffold text and every `TierKind`
  reference updates to the rung vocabulary; `canon init`'s scaffolded
  `canon.yaml` template emits the new shape.
- **`crates/canon-report/src/tier_boundary.rs`** (s25): `pg_routed_kinds`
  → a backend-capability-keyed derivation (name TBD at implementation
  time, e.g. `non_offline_readable_kinds`); filters on
  `!Backend::offline_file_readable()` instead of `TierKind::Pg`.
  `crates/canon-store/sql/views.sql`'s `stg_records` doc comment
  reframes from "the `pg` tier is intentionally not staged" to
  "backends without `offline_file_readable()` are intentionally not
  staged".
- **Every canon.yaml fixture referencing the git/pg/r2 vocabulary
  (26 Rust source files under `crates/**`, enumerated below) is
  rewritten to the rung/backend shape** — this is the mechanical bulk
  of the implementation phase's diff, not a design risk (the mapping
  is a bijection today: `git → local`, `pg → hot`, `r2 → cold`).
- **`target/usage-review/loom/canon.yaml`** (the one live multi-tier
  dogfood dummy, git+postgres+S3) is rewritten to the new shape.
  `target/usage-review/eno-drift/canon.yaml` and `target/usage-review/
  najun-art-dummy/canon.yaml` are git-only scratch dummies; migrating
  them is optional/best-effort (their `tiers.git`/`routing: … : git`
  shape still fails to parse post-migration, but neither is exercised
  by any test — they are standalone dogfood checkouts).
- **No new crate.** `canon-model`/`canon-ingest`/`canon-vocab`/
  `canon-plugin`/`canon-learn`/`canon-gate` are unaffected — `canon-
  model` in particular has zero `TierKind`/`Rung`/`Backend` references
  today and gains none.

### canon.yaml fixtures to migrate (grepped `routing:`/`tiers:` under `crates/**` + `target/usage-review/*/canon.yaml`)

`crates/canon-cli/src/context.rs`, `src/ingest.rs`, `src/init.rs`,
`src/main.rs`, `src/plans.rs`, `src/tiers.rs`,
`crates/canon-cli/tests/artifact_ingest.rs`, `tests/gate.rs`,
`tests/init.rs`, `tests/plans_ingest.rs`, `tests/plugin_sync.rs`,
`tests/query.rs`, `tests/query_tier_degradation.rs`,
`tests/report_tier_boundary.rs`, `tests/scaffold.rs`,
`tests/selftest_fixture.rs`, `tests/support/mod.rs`,
`crates/canon-gate/src/ledger.rs`, `src/policy.rs`, `src/selftest.rs`,
`src/staleness.rs` (these four are `risk_routing:`/`policy.yaml`
matches — NOT `canon.yaml`'s tier `routing:`, false positives from the
grep pattern, confirmed by inspection; no change needed),
`crates/canon-report/src/tier_boundary.rs`, `tests/tier_boundary.rs`,
`crates/canon-store/src/policy.rs`, `src/registry.rs`,
`tests/e2e_write_age_query_duckdb.rs` — 26 files total; plus
`target/usage-review/loom/canon.yaml` (live multi-tier dummy, MUST
migrate) and, best-effort, `target/usage-review/eno-drift/canon.yaml`
+ `target/usage-review/najun-art-dummy/canon.yaml` (git-only scratch).
