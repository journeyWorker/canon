## Why

s27 (`tier-role-backend-split`) split canon's tier vocabulary into a
capability `Rung` (`local`/`hot`/`cold`) and an implementing `Backend`
(`git`/`postgres`/`s3`), and — as an EXPLICIT design choice at the
time (D1's own "any rung MAY be tagged with any backend" claim) — left
`TierPolicy::from_yaml` accepting ANY rung/backend pairing. That
robustness claim is now the wrong default: nothing about a `local`
(diffable-file) rung backed by a live database, or a `hot`
(live-queryable) rung backed by git, is a coherent "swap the vendor,
keep the role" story — it is an incoherent config that happens to
parse. `canon.yaml`'s `tiers.local: { backend: postgres, ... }` parses
today; it should not.

**Second, independent bug in the same neighborhood:** s27's
`Backend::offline_file_readable()` — the ONE method `canon report`'s
tier-boundary derivation (`crates/canon-report/src/tier_boundary.rs`)
reads to decide which record kinds its marts can see — returns `true`
for `S3`. This is WRONG. `canon report`'s marts are built exhaustively
on `canon-store`'s DuckDB views, which read ONLY two LOCAL roots: the
git ledger (`stg_git_records`, via `read_text`) and a local `canon/r2`
parquet directory (`stg_r2_records`, via `read_parquet` over
`CANON_R2_ROOT`, defaulted to `<repo>/canon/r2`). Neither ever opens a
live connection. But `canon tier age` writes cold/S3 records to the
LIVE S3 BUCKET, never to the local `canon/r2` directory — so for a
repo whose `cold` rung is S3-backed (today's default pairing), the
report's local `canon/r2` mirror stays empty unless an operator
separately, manually materializes one; canon has no automatic sync
for this. `offline_file_readable()` returning `true` for S3 therefore
silently OVERCLAIMS: it tells `canon report` (and its readers) that
an S3-routed kind's data is safely reflected in the marts, when in the
common case it is not. This is exactly the round-3 F2 bug class s25
(`report-pg-tier-boundary`) was created to eliminate — reintroduced by
s27 onto the S3 backend specifically.

Both bugs are corrected in one change because they share the same
root file (`crates/canon-store/src/policy.rs`) and the same
`canon.yaml`-shape surface, but they are DISTINCT concerns: D1 below
is a parse-time COMPATIBILITY constraint (can this backend implement
this rung at all); D2 below is `canon-report`'s own report-INCLUSION
signal (does the report open this backend's own store directly). They
must never be collapsed into one method.

## What Changes

- **New `BackendClass` enum** (`LocalFile`/`LiveDb`/`ObjectStore`) —
  the I/O capability class a `Backend` belongs to. `Backend::class()`
  (`Git`→`LocalFile`, `Postgres`→`LiveDb`, `S3`→`ObjectStore`) and
  `Rung::expected_backend_class()` (`Local`→`LocalFile`,
  `Hot`→`LiveDb`, `Cold`→`ObjectStore`) are the two halves of a new
  compatibility check.
- **`TierPolicy::from_yaml` now validates every configured
  `tiers.<rung>` entry's backend class against that rung's expected
  class**, rejecting a mismatch with a loud, hint-carrying
  `PolicyError` (e.g. `` canon.yaml `tiers.local`: backend `postgres`
  is a live-database backend, but the `local` rung expects a
  local-file backend (`git`) ``). `local`/`hot`/`cold` are no longer
  "any backend accepted" — they are coherent capability roles again.
  The `backend:` field itself stays explicit (a future same-class
  backend swap, e.g. a second live-database vendor for `hot`, remains
  expressible) — this is a compatibility CLASS check, not a pin to one
  literal backend name.
- **`Backend::offline_file_readable()` renames to
  `Backend::read_directly_by_report()`, and `S3` flips from `true` to
  `false`.** `Git` stays `true` (the git ledger IS one of
  `canon-report`'s local read roots); `Postgres` stays `false` (a live
  server, never opened); `S3` is now `false` too (a live bucket, never
  opened directly — only a separately-materialized local `canon/r2`
  mirror would surface its data, and canon has no automatic sync for
  that mirror today).
- **`crates/canon-report/src/tier_boundary.rs` renames
  `non_offline_readable_kinds` to `kinds_not_read_directly`**, filters
  on `!Backend::read_directly_by_report()`, and renders `## Kinds not
  read directly` instead of `## Tiers not reflected` — an S3-routed
  (`cold`, by today's convention) kind now appears in this section,
  correcting the previous silent omission.
- **Truthful, non-absolute boundary wording (design D3).** The shared
  sentence both the rendered note and the stderr `WARN` read no longer
  claims data is unconditionally "not reflected" (false once a local
  `canon/r2` mirror exists) — it states that `canon report` reads its
  local roots directly and that a listed kind's backend's own store is
  not one of them, so its data appears only if separately materialized
  into those local roots, which may be incomplete or stale.
- **No new backend, no new `RecordKind`, no live connection added
  anywhere.** This change is a validation tightening (D1) plus a
  one-bit correction and a wording correction (D2/D3) — no new I/O
  path.

### Capabilities

- **ADDED** `rung-backend-capability`: the `BackendClass`
  compatibility check itself — `Backend::class()`,
  `Rung::expected_backend_class()`, and `TierPolicy::from_yaml`'s new
  loud rejection of a class-mismatched `tiers.<rung>` entry.
- **MODIFIED** `report-pg-tier-boundary` (s25, rekeyed by s27): the
  report-inclusion signal renames to `read_directly_by_report`, S3
  flips to excluded, the rendered heading/sentence and stderr wording
  update to the truthful, non-absolute framing (D2/D3).
- **MODIFIED** `tier-role-backend-split` (s27): retracts the "any rung
  may be tagged with any backend" scenario — s28's D1 supersedes it
  with the class-compatibility constraint.

### Explicit non-goals

- **No live read added to `canon-report`.** No `PgTier::connect`, no
  S3 client, no row count, no `stg_pg_records`/`stg_s3_records`.
- **No automatic `canon/r2` materialization / sync feature.** An
  operator wanting an S3-backed rung's data in the report must still
  manually mirror it into local `canon/r2` today — a future opt-in
  materialization feature is explicitly out of scope.
- **No change to the closed 12-`RecordKind` set.**
- **connector-never-authority preserved: `canon gate check` is
  byte-identical.** No `canon-gate` source file is touched.
- **`canon report` stays offline/deterministic/drift-checkable.**
- **No backend-class relaxation escape hatch.** A class mismatch is
  ALWAYS a hard parse error — no `--allow-any-backend` flag, no
  per-rung override.
- **No fixture migration needed for today's default pairing.** Every
  existing `canon.yaml` fixture in the tree already uses
  `local`→git/`hot`→postgres/`cold`→s3 — the class-correct combo — so
  D1's validation introduces no fixture churn EXCEPT for the small
  number of tests that deliberately constructed an incompatible
  combo (`cold`→postgres, `hot`→s3) to exercise s27's own "any rung,
  any backend" claim; those move to direct `Backend`-method unit
  tests (D1/D2's counter-cases), since the combo they exercised is no
  longer constructible via `TierPolicy::from_yaml`.

## Impact

- **`crates/canon-store/src/policy.rs`**: new `BackendClass` enum +
  `Backend::class()` + `Rung::expected_backend_class()`;
  `TierPolicy::from_yaml`'s `tiers` loop gains a class-compatibility
  check; `Backend::offline_file_readable()` renames to
  `Backend::read_directly_by_report()` with `S3` flipped to `false`.
- **`crates/canon-report/src/tier_boundary.rs`**:
  `non_offline_readable_kinds` → `kinds_not_read_directly`, filtering
  on `read_directly_by_report()`; `## Tiers not reflected` → `##
  Kinds not read directly`; the shared sentence rewords per D3; module
  doc updates to match.
- **`crates/canon-report/src/render.rs`**: `render()`'s
  `non_offline_readable_kinds` parameter renames to
  `kinds_not_read_directly` (pure rename, no shape change).
- **`crates/canon-report/src/lib.rs`**: `report()`'s local variable
  renames to match.
- **`crates/canon-cli/src/main.rs`**: `run_report`'s local variable
  and doc comment rename to match.
- **`crates/canon-store/sql/views.sql`**: `stg_records`'s doc comment
  corrects to describe the local-mirror-vs-live-bucket distinction
  (D2/D3) — no `CREATE VIEW` statement changes.
- **`canon/skills/tiered-storage/SKILL.md`**: the "What canon report
  reflects" section updates to the corrected boundary.
- **Tests**: `crates/canon-store/src/policy.rs`'s
  `any_rung_may_be_tagged_with_any_backend` test (a `cold`→postgres
  fixture, now class-mismatched) becomes
  `class_mismatched_backend_fails_loud_with_a_hint`, asserting the new
  rejection; new `each_class_correct_rung_backend_pairing_parses`,
  `backend_class_matches_the_designed_pairing`,
  `read_directly_by_report_is_true_only_for_git` tests.
  `crates/canon-report/src/tier_boundary.rs`'s
  `a_cold_rung_backed_by_postgres_is_excluded_…` and
  `a_hot_rung_backed_by_s3_is_included_…` tests (both now
  class-mismatched fixtures) are replaced by
  `a_cold_rung_backed_by_s3_now_appears_in_kinds_not_read_directly`
  (the class-correct combo, exercising the corrected D2 signal).
  `crates/canon-report/tests/tier_boundary.rs` and
  `crates/canon-cli/tests/report_tier_boundary.rs` update their
  heading-text assertions and add the S3-now-included scenario.
- **No new crate.** `canon-model`/`canon-ingest`/`canon-vocab`/
  `canon-plugin`/`canon-learn`/`canon-gate` are unaffected.
