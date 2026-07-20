# Tasks — s28 rung/backend capability

Sequencing follows design.md: **P1 (`policy.rs`: `BackendClass` +
validation + rename) before everything else** — every other phase
depends on `read_directly_by_report`/`BackendClass` existing. P2
(`tier_boundary.rs` rename/reword) depends on P1. P3 (pure
parameter/variable renames in `render.rs`/`lib.rs`/`main.rs`) depends
on P2. P4 (`views.sql`/`SKILL.md` doc corrections) depends on P1,
independent of P2/P3. P5 (tests) depends on P1-P3. P6 (closure)
depends on all.

## 1. `BackendClass` + validation + rename (P1)

- [x] 1.1 `crates/canon-store/src/policy.rs`: new `pub enum
  BackendClass { LocalFile, LiveDb, ObjectStore }` with private
  `describe()`/`example_backend()` helpers (design D1).
- [x] 1.2 Same file: `Backend::class(self) -> BackendClass` (`Git`→
  `LocalFile`, `Postgres`→`LiveDb`, `S3`→`ObjectStore`) and
  `Rung::expected_backend_class(self) -> BackendClass` (`Local`→
  `LocalFile`, `Hot`→`LiveDb`, `Cold`→`ObjectStore`), design D1.
- [x] 1.3 Same file: `TierPolicy::from_yaml`'s `tiers` decode loop
  validates `cfg.backend().class() == rung.expected_backend_class()`
  for every entry, returning a loud, hint-carrying `PolicyError` on
  mismatch (design D1) — naming the rung key, the configured backend,
  its actual class, the rung's expected class, and one example
  backend of that class.
- [x] 1.4 Same file: rename `Backend::offline_file_readable` →
  `Backend::read_directly_by_report`; flip `S3`'s return value from
  `true` to `false` (design D2). Update the module doc (new "Backend
  capability class" + "Correcting the report-inclusion signal"
  sections) and `from_yaml`'s own doc comment to name the new failure
  mode.
- [x] 1.5 Same file, `#[cfg(test)] mod tests`: replace the
  `any_rung_may_be_tagged_with_any_backend` test (a `cold`→postgres
  fixture, now class-mismatched) with
  `class_mismatched_backend_fails_loud_with_a_hint` asserting the new
  rejection and its message content; add
  `each_class_correct_rung_backend_pairing_parses` (local/git,
  hot/postgres, cold/s3 each parse independently); add
  `backend_class_matches_the_designed_pairing` and
  `read_directly_by_report_is_true_only_for_git` as the D1/D2
  counter-case unit tests (the direct-`Backend`-method migration of
  what would otherwise be an unconstructible production-config
  fixture).

## 2. `tier_boundary` rename + reword (P2 — after 1)

- [x] 2.1 `crates/canon-report/src/tier_boundary.rs`: rename
  `non_offline_readable_kinds` → `kinds_not_read_directly`; filter on
  `!cfg.backend().read_directly_by_report()` (design D2).
- [x] 2.2 Same file: `render_note` emits `## Kinds not read directly`
  (was `## Tiers not reflected`); `ESCAPE_HATCH_SENTENCE` rewords to
  the truthful, non-absolute framing (design D3); `warn_line`'s
  kind-list suffix renames `Not reflected:` → `Not read directly:`.
- [x] 2.3 Same file: rewrite the module doc (F2 provenance, the s27
  backend-capability reframing, the NEW s28 "read_directly_by_report,
  not offline_file_readable" section, and the "conservative lower
  bound, not an exact list" section explaining why a local `canon/r2`
  mirror can make the set imprecise in the safe direction only).
- [x] 2.4 Same file, `#[cfg(test)] mod tests`: rename calls to
  `kinds_not_read_directly`; replace
  `a_cold_rung_backed_by_postgres_is_excluded_…` and
  `a_hot_rung_backed_by_s3_is_included_…` (both now class-mismatched,
  unconstructible fixtures) with
  `a_cold_rung_backed_by_s3_now_appears_in_kinds_not_read_directly`
  (the class-correct combo, exercising the corrected D2 signal).

## 3. Pure rename propagation (P3 — after 2)

- [x] 3.1 `crates/canon-report/src/render.rs`: `render()`'s
  `non_offline_readable_kinds` parameter renames to
  `kinds_not_read_directly`; doc comment updates to match.
- [x] 3.2 `crates/canon-report/src/lib.rs`: `report()`'s local
  variable renames to match; doc comment updates.
- [x] 3.3 `crates/canon-cli/src/main.rs`: `run_report`'s local
  variable and doc comment rename to match.

## 4. Doc corrections (P4 — after 1, independent of 2/3)

- [x] 4.1 `crates/canon-store/sql/views.sql`: `stg_records`'s doc
  comment corrects to distinguish "Postgres has zero SQL view here"
  from "S3's local `canon/r2` mirror is scanned if present, but is not
  automatically kept in sync with the live bucket" (design D2/D3) —
  no `CREATE VIEW` statement changes.
- [x] 4.2 `canon/skills/tiered-storage/SKILL.md`: the "What canon
  report reflects" section rewrites to name
  `read_directly_by_report()`/`## Kinds not read directly` and the
  corrected S3/local-mirror distinction.

## 5. Tests (P5 — after 1-3)

- [x] 5.1 `crates/canon-report/tests/tier_boundary.rs`: rename heading
  assertions to `## Kinds not read directly`; flip the existing
  multi-tier test's `scenario`(cold/s3)-routed-kind assertion from
  absent to present; replace the cold+s3 "renders no section" fixture
  with a genuine git-only one; add
  `a_cold_rung_backed_by_s3_now_appears_in_the_boundary_section`
  (integration-level proof of the D2 correction).
- [x] 5.2 `crates/canon-cli/tests/report_tier_boundary.rs`: rename
  heading assertion to `## Kinds not read directly`.
- [x] 5.3 Confirm every PRE-EXISTING test elsewhere in the workspace
  stays green UNMODIFIED — every other `canon.yaml` fixture in the
  tree already uses the class-correct default pairing
  (local→git/hot→postgres/cold→s3), so D1's validation introduces no
  further fixture churn.

## 6. Closure

- [x] 6.1 `cargo build --workspace` green.
- [x] 6.2 `cargo test --workspace --no-fail-fast` green.
- [x] 6.3 `cargo clippy --workspace --all-targets -- -D warnings`
  green.
- [x] 6.4 `bunx openspec validate --strict s28-rung-backend-capability`
  green.
- [x] 6.5 `canon gate check` byte-identity re-confirmed — this change
  touches no `canon-gate` source file; `crates/canon-report/tests/
  gate_independence.rs` and `canon-cli`'s gate tests are unmodified
  and still green.
