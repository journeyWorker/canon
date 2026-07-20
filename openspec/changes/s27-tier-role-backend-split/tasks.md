# Tasks — s27 tier role/backend split

Sequencing follows design.md: **P1 (`Rung`/`Backend` enums + `TierPolicy`
rewrite) before everything else** — every other phase depends on the new
vocabulary existing. P2 (`TierRegistry` rekey) depends on P1. P3
(`canon-cli` tier builders) depends on P1+P2. P4 (`canon-report` boundary
rekey) depends on P1 only, independent of P2/P3. P5 (CLI surface text —
`init`/`context`/`main`) depends on P1. P6 (the mechanical canon.yaml
fixture rewrite, 26 files + the loom dummy) depends on P1-P5 landing
first (so fixtures are rewritten against the FINAL shape, not an interim
one). P7 (tests) depends on P1-P6. P8 (closure) depends on all.

## 1. `Rung`/`Backend` enums + `TierPolicy` rewrite (P1)

- [x] 1.1 `crates/canon-store/src/policy.rs`: replace `TierKind` with
  `Rung { Local, Hot, Cold }` (`as_str`/`parse`, the latter
  rejecting `git`/`pg`/`r2` with the rung-vocabulary hint, design D1/D3)
  and a new `Backend { Git, Postgres, S3 }` (`as_str`/`parse`/
  `offline_file_readable`, design D1/D2).
- [x] 1.2 Same file: replace `GitTierConfigRaw`/`PgTierConfigRaw`/
  `R2TierConfigRaw`/`TiersRaw` with `BackendConfigRaw` (internally
  tagged on `backend:`, design D1) and a `tiers: HashMap<String,
  serde_yaml::Value>` raw field — decoded in `from_yaml` via the
  two-step rung-key-then-backend-value validation (design D1/D3
  mechanism) so a legacy `tiers.git`/`tiers.pg`/`tiers.r2` key and a
  `backend:`-less rung block both fail loud with the correct hint
  class.
- [x] 1.3 Same file: `TierPolicy.git`/`.pg`/`.r2` fields → `pub tiers:
  HashMap<Rung, BackendConfig>` (`BackendConfig` enum wrapping
  `GitTierConfig`/`PgTierConfig`/`R2TierConfig`, unchanged field sets);
  `routing`/`aging.*.to` retype from `TierKind` to `Rung`; `tier_for`
  returns `Result<Rung, StoreError>`.
- [x] 1.4 Same file: rewrite `from_yaml` to parse `routing`/`aging`
  values via `Rung::parse` (not `TierKind::parse`) and the `tiers` map
  per 1.2's two-step decode; every hint-carrying error case (routing
  value, aging `.to` value, top-level `tiers` key, missing `backend:`
  tag) covered by a unit test in this file's own `#[cfg(test)] mod
  tests`.
- [x] 1.5 Module doc comment update: name the D1 role/backend split,
  the D3 hard-migration posture (no alias), and cross-reference
  `openspec/changes/s27-tier-role-backend-split/design.md`.

## 2. `TierRegistry` rekey (P2 — after 1)

- [x] 2.1 `crates/canon-store/src/registry.rs`: `git`/`pg`/`r2` fields
  → `local`/`hot`/`cold: Option<Arc<dyn Tier>>` (design D5); keep a
  separate concrete `git()`-equivalent accessor for callers needing
  the git-adapter specifically (design D5's Trade-off mitigation, e.g.
  `--plugin`'s git-tree resolution) — resolved via whatever backend
  `Rung::Local` is configured with, downcast or tracked
  separately as the implementation phase's own judgment call.
- [x] 2.2 Same file: `handle(rung: Rung) -> Result<Arc<dyn Tier>,
  StoreError>` rekeys `TierKind`→`Rung`; `StoreError::TierUnavailable`
  grows `{ rung: Rung, backend: Option<Backend>, reason: String }`
  (design D5) — `None` backend when the rung has no `tiers.<rung>`
  block at all, `Some(backend)` when configured but unattached.
- [x] 2.3 Same file: `tiers_for_read`, `query`, `age_all` rekey their
  `TierKind` parameter/local types to `Rung` with no other logic
  change (routed-rung-plus-aging-destination read fan-out; aging-
  source-to-aging-destination move — both unchanged in substance).
- [x] 2.4 `crates/canon-store/src/tier.rs`: `StoreError::TierUnavailable`
  variant + `Display` impl update to the new `{rung, backend, reason}`
  shape, producing text like `"hot tier (postgres) is not attached (no
  live DSN)"` / `"hot tier is not configured (no \`tiers.hot\` in
  canon.yaml)"` (design D5).

## 3. `canon-cli` tier builders rekey (P3 — after 1, 2)

- [x] 3.1 `crates/canon-cli/src/tiers.rs`: `build_tiers`/
  `build_lenient_tiers`/`build_lenient_tiers_for_kind`/
  `tiers_needed_for`/`read_tier`/`LoadedTiers`/`LenientTiers` rekey
  `TierKind`→`Rung` throughout.
- [x] 3.2 Same file: rename `attach_pg`→`attach_postgres`,
  `attach_r2`→`attach_s3` (design D4) — same attach-or-degrade
  contract, doc comments updated to cite the rung-keyed `canon.yaml`
  paths (`tiers.hot.*`/`tiers.cold.*` in their worked examples,
  instead of `tiers.pg.*`/`tiers.r2.*`).
- [x] 3.3 `crates/canon-cli/src/tier.rs`: `canon tier age`'s CLI
  surface rekeys any `TierKind` reference to `Rung`; error/output text
  naming a tier by name now names the rung (and backend, where the
  existing text already named one).
- [x] 3.4 `crates/canon-cli/src/query.rs`: `fold_pg_routed_kind`'s own
  MAINTAINER NOTE (s21) list stays kind-gated (unaffected by this
  change per that note's own design) — confirm compiles unmodified
  against the `Rung`-keyed `TierPolicy`/`TierRegistry` APIs; update
  only the doc comment's literal `pg` mentions to `hot`/`postgres`
  where they describe `canon.yaml`'s routing vocabulary specifically
  (not the historical s21 rationale, which stays as written).

## 4. `canon-report` boundary rekey (P4 — after 1, independent of 2/3)

- [x] 4.1 `crates/canon-report/src/tier_boundary.rs`: rename
  `pg_routed_kinds` to a backend-capability-named function (e.g.
  `non_offline_readable_kinds`); filter `policy.routing` entries whose
  rung resolves (via `policy.tiers`) to a backend where
  `!Backend::offline_file_readable()`, INCLUDING a routed rung with NO
  `tiers.<rung>` entry at all (unconfigured — never assume it's
  offline-readable; treat unconfigured identically to "backend
  unknown, exclude conservatively" — design D2's own robustness bar,
  captured as a new unit test).
- [x] 4.2 Same file: `render_note`/`warn_line` unchanged in shape
  (still take `&[RecordKind]`, still produce byte-identical prose to
  today for the identical kind set) — only their caller's derivation
  function changed name/logic upstream.
- [x] 4.3 Same file: module doc rewrite — name the D2 backend-capability
  reframing explicitly, cross-reference `s27-tier-role-backend-split`
  alongside the existing `s25-report-pg-tier-boundary` reference (never
  delete the s25 provenance note; this is a documented supersession,
  not an erased history).
- [x] 4.4 `crates/canon-store/sql/views.sql`: `stg_records`'s doc
  comment (currently naming "the `pg` tier is intentionally not staged
  here") rewrites to name "backends without `offline_file_readable()`
  are intentionally not staged" — no `CREATE VIEW` statement touched.

## 5. CLI surface text rekey (P5 — after 1)

- [x] 5.1 `crates/canon-cli/src/init.rs`: `canon init`'s scaffolded
  `canon.yaml` template emits the new `tiers.<rung>: {backend: ...}`
  shape, `routing`/`aging` values as rung names, and updates its own
  in-template comment (currently "flip a `routing:` line to `pg`/`r2`")
  to describe rungs/backends correctly.
- [x] 5.2 `crates/canon-cli/src/context.rs`, `src/main.rs`: any
  `TierKind`/literal `git`/`pg`/`r2` routing-vocabulary reference in
  doc comments or `--check-config` surface text updates to the rung/
  backend vocabulary.
- [x] 5.3 `crates/canon-cli/src/plans.rs`, `src/ingest.rs`: the
  `TierPolicy { git: None, pg: None, r2: None, ... }` empty-policy
  construction (`plans.rs:293`) rekeys to `TierPolicy { tiers:
  HashMap::new(), ... }`; any inline test-fixture `canon.yaml` text in
  these two files migrates per phase 6 below.

## 6. Mechanical canon.yaml fixture rewrite (P6 — after 1-5)

- [x] 6.1 Rewrite every inline `canon.yaml` literal across the 26
  files enumerated in proposal.md's Impact section
  (`crates/canon-cli/src/{context,ingest,init,main,plans,tiers}.rs`;
  `crates/canon-cli/tests/{artifact_ingest,gate,init,plans_ingest,
  plugin_sync,query,query_tier_degradation,report_tier_boundary,
  scaffold,selftest_fixture}.rs` + `tests/support/mod.rs`;
  `crates/canon-report/src/tier_boundary.rs` + `tests/tier_boundary.rs`;
  `crates/canon-store/src/{policy,registry}.rs` + `tests/
  e2e_write_age_query_duckdb.rs`) using the mechanical bijection
  `git→local`, `pg→hot`, `r2→cold` on every routing/aging VALUE,
  and `tiers.git→tiers.local`/`tiers.pg→tiers.hot`/`tiers.r2→
  tiers.cold` PLUS an added `backend: git|postgres|s3` tag on each
  `tiers:` block key. `crates/canon-gate/src/{ledger,policy,selftest,
  staleness}.rs`'s `risk_routing:`/`policy.yaml` matches are OUT OF
  SCOPE (confirmed false positives, proposal.md Impact) — do not
  touch.
- [x] 6.2 Every literal STRING ASSERTION in these files that embeds
  the old vocabulary (e.g. `"tiers.pg not attached"`,
  `assert_eq!(kinds.iter()... vec!["event","session","task"])`'s
  surrounding routing setup, `TierKind::Pg` in a test's own
  construction) updates in lockstep with its fixture — tracked
  per-file so no assertion silently starts asserting against stale
  text.
- [x] 6.3 `target/usage-review/loom/canon.yaml` (the one live
  multi-tier dogfood dummy) rewritten to the new shape, preserving its
  exact routing/aging semantics (git→local, pg→hot, r2→cold,
  `handoff` ages hot→cold at `0d`).
- [~] 6.4 (skipped: best-effort, non-blocking per task text) (Best-effort, non-blocking) `target/usage-review/
  eno-drift/canon.yaml` and `target/usage-review/najun-art-dummy/
  canon.yaml` — git-only scratch dummies, exercised by no test;
  migrate if convenient, skip otherwise without blocking closure.

## 7. Tests (P7 — after 1-6)

- [x] 7.1 `crates/canon-store/src/policy.rs`'s own `#[cfg(test)] mod
  tests`: new-shape parse success (rung routing + backend tags, any
  rung↔backend pairing); each of the four legacy-shape failure modes
  (routing value, aging `.to` value, top-level `tiers` key, missing
  `backend:` tag) — spec `tier-role-backend-split`'s four scenarios.
- [x] 7.2 `crates/canon-store/src/registry.rs`'s own tests: `handle`/
  `tiers_for_read`/`query`/`age_all` rekeyed to `Rung`, including a
  `TierUnavailable` naming both rung and backend, and a separate case
  naming rung alone when unconfigured — spec `query-tier-degradation`'s
  rung/backend-naming scenarios (exercised at the `canon-cli` binary
  layer per 7.3, and at this crate's own unit-test layer for the
  `StoreError` shape itself).
- [x] 7.3 `crates/canon-cli/tests/query_tier_degradation.rs`: rekey
  every existing fixture per phase 6; add the "rung with no
  `tiers.<rung>` block fails naming rung alone" case (new scenario,
  spec `query-tier-degradation`).
- [x] 7.4 `crates/canon-report/tests/tier_boundary.rs`: rekey existing
  fixtures; add the "cold rung backed by postgres is excluded" case
  proving capability-keying (spec `report-pg-tier-boundary`'s new
  scenario) and a "hot rung backed by s3 is INCLUDED" companion case
  (the flip side of the same robustness claim, design D2).
- [x] 7.5 `crates/canon-cli/tests/report_tier_boundary.rs`: rekey
  existing fixtures to the rung/backend shape; stderr `WARN` text
  assertions updated only where they embed the old vocabulary.
- [x] 7.6 `crates/canon-cli/tests/{artifact_ingest,gate,init,
  plans_ingest,plugin_sync,query,scaffold,selftest_fixture}.rs`: rekey
  fixtures per phase 6; re-run and confirm every PRE-EXISTING
  assertion (persisted counts, exit codes, output shape) passes
  unmodified in semantics.
- [x] 7.7 New test proving `canon gate check` byte-identity across the
  migration (spec `tier-role-backend-split`'s gate-invariance scenario)
  — run `canon gate check` against an unchanged evidence corpus before
  and after a `canon.yaml` rung/backend migration, assert identical
  verdicts.

## 8. Closure (P8 — after all)

- [x] 8.1 `cargo build --workspace` + `cargo clippy --workspace
  --all-targets -- -D warnings` + `cargo test --workspace
  --no-fail-fast` all green.
- [x] 8.2 `bunx openspec validate --strict
  s27-tier-role-backend-split` green.
- [x] 8.3 Manually run `canon init` on a scratch repo, confirm the
  scaffolded `canon.yaml` parses via the new `TierPolicy::from_yaml`
  unmodified, and confirm a hand-edited legacy `routing: { task: pg }`
  line fails loud with the rung-vocabulary hint.
- [x] 8.4 Manually run `canon query --kind <hot-routed kind>` against
  a `canon.yaml` with an unset hot-rung DSN and confirm the error text
  names both the rung and the backend.
- [x] 8.5 Manually run `canon report` against `target/usage-review/
  loom`'s migrated `canon.yaml` and confirm the `## Tiers not
  reflected` section + stderr `WARN` name the same hot-routed kinds as
  before migration (byte-identical observable output across the
  migration, proving D2's "today's pairing selects the identical kind
  set" claim end to end).
- [x] 8.6 Structural invariants re-asserted green: `RecordKind::
  ALL.len() == 12` at all assertion sites; `canon gate check`
  byte-identity acceptance tests unmodified and still green — this
  change touches no `canon-gate` source file.
