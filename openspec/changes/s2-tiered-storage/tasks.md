## 1. Tier trait and adapter scaffolding

- [x] 1.1 Define `trait Tier { fn write(...); fn read(...); fn age(...); }`
      in `crates/canon-store`, replacing S0's marker-constant stub.
      **Evidence:** `crates/canon-store/src/tier.rs` — `pub trait Tier:
      Send + Sync { fn tier_kind(...); fn write(&self, record: &dyn
      StoredRecord) -> Result<WriteReceipt, StoreError>; fn read(&self,
      query: &TierQuery) -> Result<TierReadResult, StoreError>; fn
      age(&self, rule: &AgingRule) -> Result<AgeReport, StoreError>; }`
      (lines 222-241); `src/lib.rs` now exports
      `git_tier`/`pg_tier`/`r2_tier`/`policy`/`registry`/`tier` — S0's
      marker-constant stub fully replaced.
- [x] 1.2 Add `partition_template()` to each S1 record kind in
      `canon-model` (a pure path-template string per kind — `canon-model`
      stays storage-agnostic; only `GitTier` interprets the template
      against a filesystem).
      **Evidence:** `crates/canon-model/src/envelope.rs` —
      `RecordKind::partition_template()` returns `"kind={kind}/{id}.json"`
      (flat) or `"kind={kind}/area={area}/{id}.json"` (area-scoped, via
      new `is_area_scoped()` helper); pure `&'static str`, no
      filesystem/canon-store dependency added to `canon-model` — the
      workspace dependency graph still runs `canon-store` →
      `canon-model`, never the reverse.
- [x] 1.3 Implement `GitTier`: `write` places a record at its
      `partition_template()`-resolved path under `canon.yaml`'s
      `tiers.git.root`; `read` globs and parses records back into
      `RawRecord`s.
      **Evidence:** `crates/canon-store/src/git_tier.rs`
      `GitTier::write`/`read` (via `scan_kind_where`), resolving
      `expected_relative_path` under `self.root` (= `tiers.git.root`).
      Tested: `write_then_read_round_trips_a_flat_kind`,
      `write_then_read_round_trips_an_area_scoped_kind_area_from_scenario_id`
      (unit), plus
      `git_tier_all_kinds.rs::every_well_formed_fixture_round_trips_through_git_tier`
      — all 12 `RecordKind`s round-trip.
- [x] 1.4 Implement `PgTier` (sqlx): `write`/`read` against a
      `canon.yaml`-configured DSN (`tiers.pg.dsn_env`) and schema
      (`tiers.pg.schema`, default `canon_v1`).
      **Evidence:** `crates/canon-store/src/pg_tier.rs`
      `PgTier::connect(dsn, schema)`/`write`/`read` via `sqlx`;
      `canon.yaml`'s `tiers.pg.dsn_env`/`tiers.pg.schema` (default
      `canon_v1`) parsed by `policy.rs`. SQL-generation unit-tested
      offline (6 tests: `create_table_sql_...`, `upsert_sql_...`,
      `select_sql_...`, `select_older_than_sql_...`, `delete_sql_...`,
      `schema_identifier_is_validated_...`); the real-connection path is
      exercised by `tests/e2e_write_age_query_duckdb.rs` against a
      genuinely local, unix-socket-only ephemeral Postgres
      (`tests/support::LocalPg`, `initdb`/`pg_ctl`) — ran LIVE in this
      verification pass (1.12s, not the skip branch). A real-Postgres
      live path is separately gated (`tests/pg_tier_live.rs`, `live-pg`
      feature + `PgTier::connect_live`/`CANON_PG_DSN`) — now run LIVE
      against a local docker-compose Postgres, see task group 6.
- [x] 1.5 Implement `R2Tier` (arrow/parquet): `write`/`read` against a
      `canon.yaml`-configured bucket (`tiers.r2.bucket_env`) and prefix
      (`tiers.r2.prefix`, default `canon/`), using a DuckLake-compatible
      layout matching the prior session store's existing PG-catalog + R2-
      parquet shape so the prior session store's marts can eventually join.
      **Evidence:** `crates/canon-store/src/r2_tier.rs`
      `R2Tier::local`/`connect_live`/`write`/`read` via
      `arrow`/`parquet`/`object_store`; `canon.yaml`'s
      `tiers.r2.bucket_env`/`tiers.r2.prefix` (default `canon/`) parsed by
      `policy.rs`. Layout shares `partition::hive_object_key` with
      `GitTier` (same Hive coordinates, `.parquet` extension) —
      DuckLake-compatible per design D5's `stg_r2_records` view reading a
      `read_parquet` glob directly, no catalog process needed at write
      time. Unit-tested offline
      (`write_then_read_round_trips_via_local_object_store`,
      `parquet_bytes_actually_land_on_local_disk_in_hive_layout`,
      `duplicate_content_write_is_deduped_not_rewritten`,
      `connect_live_without_bucket_env_fails_loud_not_silently`,
      `connect_live_falls_back_to_docker_compose_defaults_when_s3_env_is_unset`);
      the live-R2 path is gated behind `tests/r2_tier_live.rs` (`live-r2`
      feature) — now run LIVE against a local docker-compose MinIO
      bucket, see task group 6.

## 2. Git-tier layout enforcement

- [x] 2.1 Implement the layout check generalizing
      `tools/parity.py::_ledger_layout_problem` (lines 1272-1294): given a
      record's kind and its file path, validate the path against that
      kind's `partition_template()`; return a `FailureClass::Malformed`
      `layout`-subclass violation on mismatch via
      `canon_model::validate_evidence` (S1 D6), never a silent reparse at
      an inferred path.
      **Evidence:** `crates/canon-store/src/partition.rs` —
      `validate_layout`/`hive_object_key`/`validate_kind_matches_content`/
      `validate_body`, generalizing `_ledger_layout_problem`'s
      flat/area-scoped split to all 12 kinds via `partition_template()`;
      violations are `EvidenceViolation::new(FailureClass::Malformed,
      "layout", ...)`, `canon_model`'s own violation type (S1 D6). Tested:
      `partition::tests::*` (7 tests: area resolution, digest/path
      identity, kind/content mismatch, six-mismatch-style area
      disagreement, layout expected/actual reporting, regime-key
      sanitization).
- [x] 2.2 Wire the layout check into `GitTier::write` (reject a write to a
      non-conforming path) and `GitTier::read` (exclude and report
      misfiled files encountered during a scan).
      **Evidence:** `GitTier::write` (`git_tier.rs:181`) calls
      `partition::validate_layout` on the path it derives before writing
      (write-time self-consistency — `GitTier::write` takes no
      caller-supplied path at all, so a non-conforming write is
      structurally unreachable, not merely rejected after the fact);
      `GitTier::read`'s `scan_kind_where` (`git_tier.rs:129-143`) computes
      each found file's expected path and excludes+reports a mismatch as
      a violation rather than accepting it. Tested:
      `a_preexisting_misfiled_file_is_flagged_on_read_and_excluded`,
      `area_mismatch_between_directory_and_content_is_a_layout_violation`,
      plus
      `git_tier_fixtures.rs::every_misfiled_fixture_is_excluded_and_reported_per_expected_violations`
      (5 pre-planted misfiled fixtures, 0 accepted).
- [x] 2.3 Enforce append-only semantics: `GitTier::write` rejects a write
      to a path that already holds a record; only a dedicated
      `GitTier::migrate_write` path (used exclusively by the future `canon
      migrate`, S11) may overwrite, and every such rewrite records a
      quarantine-report entry for anything it cannot safely upgrade.
      **Evidence:** `GitTier::write` rejects an existing path with
      `StoreError::DuplicatePath` (`git_tier.rs:171-173`; tested
      `duplicate_write_is_rejected_not_silently_overwritten`).
      `GitTier::migrate_write` (`git_tier.rs:72-89`) is the sole overwrite
      path, returning `MigrateOutcome::Quarantined(QuarantineEntry)` when
      content can't resolve a conforming path. Tested for the
      successful-relocate-and-overwrite branch
      (`migrate_write_relocates_and_allows_overwrite`) only — the
      `Quarantined` branch itself has no dedicated test feeding an
      actually-unresolvable record through `migrate_write`; the code path
      is real (constructs and returns a `QuarantineEntry`) but its own
      coverage is a minor gap, not fabricated.
- [x] 2.4 Add fixtures: one well-formed record per area-scoped and
      non-area-scoped kind; one misfiled variant per kind (wrong
      directory, wrong filename) with an EXPECTED-violations file; a
      duplicate-path write attempt.
      **Evidence:** `crates/canon-store/fixtures/git-tier/` —
      `well-formed/` (1 flat `kind=change`, 1 area-scoped
      `kind=scenario/area=world`), `misfiled/` (5 variants: missing
      `area=`, wrong filename, wrong area, wrong directory, malformed
      content) + `EXPECTED-violations.json` (5 entries, all
      `"malformed"`), asserted by `git_tier_fixtures.rs`. The
      duplicate-path write attempt is covered by the unit test named
      under 2.3 (`duplicate_write_is_rejected_not_silently_overwritten`)
      rather than a fixture file. `git_tier_all_kinds.rs` additionally
      reuses S1's own 12-kind fixture corpus for full-coverage
      round-tripping, beyond this task's literal "one kind per shape" ask.

## 3. Tier policy and aging

- [x] 3.1 Add `TierPolicy` parsing to `canon.yaml`: `tiers` (per-tier
      config), `routing` (kind → tier), `aging` (kind → `{after, to}`).
      **Evidence:** `crates/canon-store/src/policy.rs` —
      `TierPolicy::from_yaml` parses `tiers`/`routing`/`aging`; root
      `canon.yaml` gains all three sections (git/pg/r2 tiers, 12 routing
      entries covering all 12 kinds, 2 aging rules — `handoff`/`event`,
      D3's worked example verbatim). Tested: `policy::tests::*` (6 tests)
      +
      `real_canon_yaml.rs::every_record_kind_routes_somewhere_in_the_shipped_canon_yaml`
      (against THIS repo's real root `canon.yaml`, not a synthetic
      fixture) +
      `s1_handoff_templates_and_s2_tier_policy_coexist_in_one_canon_yaml`
      (S1's `handoff_templates:` key unaffected).
- [x] 3.2 Resolve every `Tier::write`/`Tier::read` call through
      `TierPolicy.routing` — no caller branches on a literal kind name to
      pick a tier.
      **Evidence:** `TierRegistry::persist`/`query`/`age_all`
      (`registry.rs`) resolve every call through
      `self.policy.tier_for(kind)` — no `match kind { ... }` branch
      anywhere in `registry.rs`, `git_tier.rs`, `pg_tier.rs`, or
      `r2_tier.rs`. Tested:
      `registry::tests::persist_routes_through_policy_never_a_literal_kind_branch`,
      `persist_of_an_unrouted_kind_is_a_loud_error` (an unrouted kind is
      `StoreError::UnroutedKind`, never a silent default tier).
- [x] 3.3 Implement `canon tier age`: for each `aging` entry, select
      records past `after` in their current tier, compute a canonical
      content digest (SHA-256 over deterministic-ordered serialization),
      write to `to`'s tier keyed by that digest, and delete from the
      source tier only after the destination write confirms.
      **Evidence:** `crates/canon-cli/src/tier.rs` — `canon tier age
      [--dry-run]`, wired in `src/main.rs`'s `TierCommand::Age`. A real
      run calls `canon_store::registry::TierRegistry::age_all()`
      directly (the digest-keyed write-then-delete mechanism itself
      stays exactly where 3.3's original evidence left it,
      `git_tier.rs`/`pg_tier.rs`'s `Tier::age`, never reimplemented
      here); `--dry-run` previews candidates via a read-only
      `Tier::read` + the same `after`-threshold predicate, performing
      no write/delete. `crates/canon-cli/src/tiers.rs` builds the live
      `GitTier`/`PgTier`/`R2Tier` handles a `canon.yaml` configures
      (`PgTier`/`R2Tier` only when their own `tiers.*` section is
      present — zero network otherwise), with a CLI-only
      `CANON_R2_LOCAL_ROOT` test seam (`R2Tier::local`, never used by
      `canon-store` itself) so integration tests exercise a genuine
      git→r2 move offline, with no credentials. Tested (offline,
      against the built `canon` binary):
      `crates/canon-cli/tests/tier_age.rs` —
      `real_run_moves_the_aged_record_and_leaves_the_fresh_one_and_is_idempotent_on_rerun`
      (a 30-day-old `trajectory` past a 1d threshold moves git→r2,
      `moved: 1`; a within-threshold sibling record is left in git; an
      immediate re-run reports `moved: 0, already_aged: 0`) and
      `dry_run_reports_candidates_and_performs_no_writes` (dry-run
      reports the same candidate count but leaves both tiers
      byte-for-file untouched).
- [x] 3.4 Add fixtures + tests: a record past its aging threshold moves
      tiers and is readable at its new location; a record within
      threshold is untouched; re-running `canon tier age` on an
      already-aged record performs zero duplicate writes (digest hit).
      **Evidence:**
      `registry::tests::age_all_moves_records_past_threshold_and_is_idempotent_on_rerun`
      — a 30-day-old `trajectory` past a 1d threshold moves git→r2
      (`moved: 1`), a second immediate run finds nothing left to move
      (`moved: 0, already_aged: 0`); `tests/e2e_write_age_query_duckdb.rs`
      reproduces the same pattern against real Postgres (60-day-old
      `handoff` past a 30d threshold). The within-threshold-untouched
      case IS now covered (previously an open gap in this evidence
      note):
      `crates/canon-cli/tests/tier_age.rs::real_run_moves_the_aged_record_and_leaves_the_fresh_one_and_is_idempotent_on_rerun`
      plants a within-threshold `trajectory` alongside an aged one and
      asserts the git tier's file count stays at 1 (the fresh record
      untouched) across both the first real run and an immediate
      re-run. One precise caveat remains: the specific "digest-hit"
      recovery case — a record still present in BOTH source and
      destination (e.g. after a prior write-succeeded/delete-failed
      partial run), making a re-run's `already_aged` counter increment
      — is never exercised (both idempotence tests instead prove
      "nothing left in source to re-select"). The underlying per-write
      content-digest dedup itself IS proven directly
      (`r2_tier::tests::duplicate_content_write_is_deduped_not_rewritten`,
      `pg_tier`'s `ON CONFLICT ... DO UPDATE` upsert SQL).

## 4. Unified query and DuckDB views

- [x] 4.1 Implement `canon query --kind <k> [--since <t>]`: resolve
      `<k>`'s tier(s) from `TierPolicy` (a kind may be split across pg and
      r2 post-aging), issue each tier's native read, merge by `at`, no
      cross-tier JOIN inside `canon query` itself.
      **Evidence:** `crates/canon-cli/src/query.rs` — `canon query
      --kind <k> [--since <t>] [--json]`, wired in `src/main.rs`'s
      `Command::Query`. Calls
      `canon_store::registry::TierRegistry::query()` directly (the
      fan-out/merge-by-`at` mechanism, no cross-tier JOIN, stays
      exactly where 4.1's original evidence left it, `registry.rs`'s
      `query()`); `--kind` parses a `RecordKind::as_str()` wire string,
      `--since` an RFC3339/ISO-8601 timestamp. Default output is a
      human table (`AT`/`ID`/`DIGEST`, `ID` via
      `canon_store::partition::resolve_partition`); `--json` emits the
      full merged record bodies. Tested (offline, against the built
      `canon` binary):
      `crates/canon-cli/tests/query.rs` —
      `merges_records_split_across_the_routed_tier_and_its_aging_destination`
      (a `trajectory` split across git and a local-filesystem-backed r2
      tier merges into 2 records, ordered by `at`, no duplicate/gap)
      and `since_filters_to_records_at_or_after_the_given_timestamp`
      (only the at-or-after-cutoff record is returned).
- [x] 4.2 Add a DuckDB view file (adapting the donor parity harness's SQL layering):
      `stg_*` views over the git tier's Hive files (`read_text`/glob) and
      the r2 tier's parquet exports (`read_parquet`); `int_*` views
      mirroring `canon-gate`'s derivation logic (stub against S2's own
      fixtures until S5 ships real gate logic to mirror); `mart_*`
      persona-facing views.
      **Evidence:** `crates/canon-store/sql/views.sql` —
      `stg_git_records`/`stg_r2_records`/`stg_records` (content-trusted
      `read_text`/`read_parquet` glob extraction, deliberately never
      `hive_partitioning=true` per the design doc's Risk-section
      donor-fidelity note), `int_evidence_verdicts` (explicit STUB — S5/
      `canon-gate` hasn't shipped yet, documented in-file as "replace
      wholesale, don't extend"), `mart_records_by_kind` (persona-facing
      rollup).
- [x] 4.3 Verify the view file opens (`duckdb -init <file>`) against a
      fixture corpus with both git-tier Hive files and r2-tier parquet,
      and every view returns rows matching the fixture's known content.
      **Evidence:** `tests/e2e_write_age_query_duckdb.rs` runs the real
      `duckdb -init sql/views.sql` binary against real fixture roots
      (git-tier Hive JSON files + r2-tier parquet from a real local
      Postgres/object-store round trip), which ran LIVE in this
      verification pass (`duckdb`/`initdb`/`pg_ctl` all present on PATH —
      not the skip branch). `-init` loading the whole file proves every
      `CREATE OR REPLACE VIEW` (all 5) parses and opens without error;
      the row-content assertion itself is against `mart_records_by_kind`
      only (`change,git,1` / `handoff,r2,1` post-aging /
      `trajectory,r2,1`, matching the fixture exactly) —
      `int_evidence_verdicts` isn't independently row-asserted since this
      fixture plants no `evidence_record`.

## 5. Companion skill and fixtures

- [x] 5.1 Author this spec's companion skill under
      `canon/skills/tiered-storage/SKILL.md`, covering: how to add a new
      record kind's tier routing/aging rule to `canon.yaml`, how to run
      `canon tier age` and `canon query`, and how to read the DuckDB
      views.
      **Evidence:** `canon/skills/tiered-storage/SKILL.md` — covers
      adding/changing a `routing:`/`aging:` entry,
      `TierRegistry::persist`/`query`/`age_all` (documented as `canon
      tier age`/`canon query`'s backing implementation), a dedicated
      "Running `canon tier age` / `canon query`" section with real
      `canon tier age [--dry-run]` / `canon query --kind <k> [--since
      <t>] [--json]` invocation examples now that CLI wiring has
      shipped (`crates/canon-cli/src/tier.rs`/`query.rs`/`tiers.rs`,
      commit `5aa36920` — see 3.3/4.1), and reading the DuckDB views
      incl. rebindable `CANON_GIT_ROOT`/`CANON_R2_ROOT`.
- [x] 5.2 Add an end-to-end fixture exercising the full write → age →
      query round-trip across all three tiers (a `canon
      selftest`-shaped fixture corpus with rebindable roots, matching the
      testing strategy's GateCtx-equivalent pattern): write records of at
      least one git-tier kind, one pg-tier kind, and one r2-tier kind;
      age the pg-tier kind past its threshold; run `canon query` and
      confirm merged results; open the DuckDB views against the same
      fixture and confirm `mart_*` output matches — this is the "write/
      read/age round-trip across all three tiers in fixtures; layout
      violations detected; DuckDB views open against a fixture corpus"
      acceptance bar from the design doc's S2 section.
      **Evidence:** `tests/e2e_write_age_query_duckdb.rs` — plants one
      git-tier (`change`), one pg-tier (`handoff`, 60d old), one r2-tier
      (`trajectory`) record; ages the pg-tier `handoff` past its 30d
      threshold (pg→r2); confirms `TierRegistry::query` sees it merged
      post-move; opens `sql/views.sql` via real `duckdb -init` against
      the same roots and confirms `mart_records_by_kind` matches. Ran
      LIVE in this verification pass (real local Postgres + real
      `duckdb` binary, not the skip branch). The design doc's quoted
      acceptance bar's third clause ("layout violations detected") is
      proven by a separate fixture corpus (task 2.4's
      `git_tier_fixtures.rs`), not inside this same file — together the
      full quoted bar is met across the two purpose-built fixtures.

## 6. Live tiers run against a local docker-compose stack (MinIO + Postgres)

- [x] 6.1 Add a repo-root `docker-compose.yml` provisioning both live-tier
      backing services entirely locally (operator directive 2026-07-10):
      `postgres:16-alpine` (`POSTGRES_USER=canon`/`POSTGRES_PASSWORD=canon`/
      `POSTGRES_DB=canon_v1`, `pg_isready` healthcheck, named volume, port
      `${CANON_PG_PORT:-55432}:5432`) and `minio` (`MINIO_ROOT_USER=canon`/
      `MINIO_ROOT_PASSWORD=canoncanon`, `/minio/health/ready` healthcheck,
      named volume, ports `${CANON_MINIO_PORT:-59000}:9000`/
      `${CANON_MINIO_CONSOLE_PORT:-59001}:9001`) plus a `minio-init`
      one-shot (`minio/mc`, `depends_on: minio: condition: service_healthy`)
      that aliases the compose `minio` and runs `mc mb --ignore-existing
      local/canon` so the bucket `R2Tier::connect_live`'s own
      docker-compose default targets always exists.
      **Evidence:** `docker-compose.yml`. Brought up LIVE in this
      verification pass (`docker compose up -d --wait postgres minio` —
      both report `Healthy`; `docker compose up minio-init` — `Bucket
      created successfully `local/canon``).
- [x] 6.2 Retrofit `R2Tier::connect_live` from Cloudflare-R2-specific
      credentials (`CANON_R2_ACCOUNT_ID`/`CANON_R2_ACCESS_KEY_ID`/
      `CANON_R2_SECRET_ACCESS_KEY`, a hardcoded
      `https://{account_id}.r2.cloudflarestorage.com` endpoint) to a
      generic S3-compatible `AmazonS3Builder` construction defaulting to
      the docker-compose MinIO endpoint — `CANON_S3_ENDPOINT` (default
      `http://127.0.0.1:59000`), `CANON_S3_ACCESS_KEY`/`CANON_S3_SECRET_KEY`
      (default the compose root creds `canon`/`canoncanon`),
      `CANON_S3_REGION` (default `us-east-1`), always
      `with_allow_http(true)`/`with_virtual_hosted_style_request(false)`
      (path-style). The `bucket_env`-named-var bucket resolution itself is
      UNCHANGED — still a hard, no-default failure when unset (load-bearing
      for `canon-cli`'s own release-build safety net,
      `crates/canon-cli/src/tiers.rs::release_build_ignores_local_r2_root_env_var_and_fails_loud`,
      out of this change's territory but verified NOT to regress: that
      test's bucket-lookup failure happens before any of the four
      newly-defaulted vars are ever read). Real Cloudflare R2 remains
      reachable by overriding `CANON_S3_ENDPOINT`/`CANON_S3_REGION=auto`/
      credentials — a superset, not a breaking change to the production
      contract.
      **Evidence:** `crates/canon-store/src/r2_tier.rs::R2Tier::connect_live`.
      Tested offline: `connect_live_without_bucket_env_fails_loud_not_silently`
      (bucket resolution unchanged),
      `connect_live_falls_back_to_docker_compose_defaults_when_s3_env_is_unset`
      (a real `object_store` client builds with ZERO `CANON_S3_*` env vars
      set — building never performs network I/O). No new Cargo
      dependency — `object_store`'s existing `features = ["aws"]` already
      covers a generic S3-compatible endpoint.
- [x] 6.3 Add `PgTier::connect_live(schema)`, the `PgTier` counterpart to
      `R2Tier::connect_live`: resolves `CANON_PG_DSN` with the
      docker-compose default
      (`postgres://canon:canon@127.0.0.1:55432/canon_v1`) and delegates to
      the unchanged `PgTier::connect(dsn, schema)`. `canon-cli`'s own
      `build_tiers` is untouched — it still resolves `canon.yaml`'s
      `tiers.pg.dsn_env`-named var itself and calls `PgTier::connect`
      directly (fail-loud, no default).
      **Evidence:** `crates/canon-store/src/pg_tier.rs::PgTier::connect_live`.
      No new Cargo dependency — `sqlx`'s existing `features = [...,
      "postgres"]` already covers this.
- [x] 6.4 Rewrite `tests/r2_tier_live.rs` and extend `tests/pg_tier_live.rs`
      (both `#![cfg(feature = "live-r2"/"live-pg")]`) to target the
      docker-compose stack instead of a real cloud R2 bucket / an
      arbitrary `CANON_PG_DSN_TEST`: a cheap hand-rolled TCP-reachability
      probe against the resolved endpoint/DSN host:port runs BEFORE any
      real tier I/O. Reachable: the full round-trip runs and any
      subsequent I/O error is a real test failure (never swallowed).
      Unreachable: `CANON_REQUIRE_LIVE=1` (the CI live-tier job's own env)
      turns this into a hard `panic!` — "the live-tier CI job MUST
      actually exercise MinIO/Postgres, never silently go green";
      otherwise a clean `eprintln!` skip + return, so a bare local `cargo
      test --features live-pg,live-r2` never fails on a machine without
      docker running. `r2_tier_live.rs`'s round trip additionally asserts
      the digest-dedup no-op (second identical write reports `deduped:
      true`, same object key) and the Hive object-key shape
      (`kind=trajectory/...parquet`); `pg_tier_live.rs`'s round trip
      additionally asserts a NEW digest at the same `(kind, id)` updates
      the SAME pg row in place (never a second row) and exercises a real
      `Tier::age` move (a 90-day-old session ages past a 30d threshold
      into a destination tier, deleted from pg, with a still-live sibling
      record left untouched).
      **Evidence:** `crates/canon-store/tests/r2_tier_live.rs`,
      `crates/canon-store/tests/pg_tier_live.rs`. Ran LIVE in this
      verification pass against the real docker-compose stack (6.1's
      containers) with `CANON_REQUIRE_LIVE=1` set (proving genuine
      exercise, not a skip) — both tests pass; also verified with zero
      `CANON_*` env vars exported (proving the code-level docker-compose
      defaults, not just this task's own env exports, are correct) and
      with docker down (proving the clean-skip and the
      `CANON_REQUIRE_LIVE=1` hard-fail branches both work).
- [x] 6.5 Add a CI job spinning the docker-compose stack and running the
      live-tier tests against it, alongside a (previously absent)
      baseline offline `cargo test --workspace` job.
      **Evidence:** `.github/workflows/test.yml` — `offline` job (`cargo
      test --workspace`, no docker); `live-tier` job (`docker compose up
      -d --wait postgres minio` → `docker compose up minio-init` →
      `cargo test -p canon-store --features live-pg,live-r2` with
      `CANON_REQUIRE_LIVE=1` and the `CANON_PG_DSN`/`CANON_S3_*` env
      matching 6.1's compose defaults → `docker compose down -v`
      always-cleanup). No pre-existing test workflow was found under
      `.github/workflows/` (only `publish.yml`/`build-native.yml`,
      neither runs `cargo test`) — this task establishes the offline
      baseline job fresh rather than leaving an existing one untouched.

## Open Questions

- ~~**§10 Q1** (verbatim from the design doc): "Reuse the donor's hosted Postgres instance +
  R2 bucket with `canon_*` prefixes, or provision dedicated ones?
  (Recommend: reuse with prefixes; revisit at team scale.)"~~ **RESOLVED
  (production evidence):** the donor's own Drizzle config `tablesFilter:
  ["!ducklake_*"]` already coexists two independently-owned table sets on
  ONE hosted Postgres instance, and the prior session store's CronJobs + the prior event store's app-server pods
  already share the same database-url Secret key — this
  change implements the now-confirmed default (`PgTier` schema `canon_v1`
  on the donor's existing hosted Postgres instance; `R2Tier` prefix `canon/` on the donor's
  existing R2 bucket, per task 1.4/1.5). `canon.yaml`'s `dsn_env`/
  `bucket_env` indirection still means a future dedicated-infrastructure
  migration is a config change, not a code change — that remains a
  non-blocking possibility, but the shared-vs-dedicated QUESTION itself is
  no longer open.
