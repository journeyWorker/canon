# Tasks — s17 plan import (integration layer)

Sequencing follows design.md: **P1 (connector foundation + shared grammar)
lands strictly before P2 (openspec dialect), which lands before P3 (CLI +
persistence)** — the normalization contract must exist before a dialect
targets it, and a dialect must parse correctly before anything persists its
output; P4 (closure) depends on P1-P3. Mirrors s15/s16's
identity-before-producers discipline.

## 1. plan-connector foundation (P1)

- [x] 1.1 `canon-ingest`: `plan_adapter.rs` — `PlanAdapter` trait
      (`dialect_id()`, `resolve_source`, `parse`) + `PlanParseOutcome`
      (`Change`/`Task` candidates, per-construct NAMED unmapped-drop
      counts, malformed count) — mirrors `artifact_adapter.rs`'s frozen
      trait + shared-outcome shape; no canon-store dependency. — ✅ crates/canon-ingest/src/plan_adapter.rs
- [x] 1.2 `plan_registry.rs` — static, declaration-ordered registry +
      `find(dialect_id)` lookup, mirroring `artifact_registry.rs` (never
      HashMap-iteration-order dependent); ships with exactly the `openspec`
      entry. — ✅ crates/canon-ingest/src/plan_registry.rs + plan_adapters/openspec.rs stub
- [x] 1.3 Extract `openspec_task.rs`'s local checkbox-row grammar mirror
      (`parse_row`, annotation + evidence handling, `is_task_number`) into
      a shared `openspec_rows.rs` module; re-point the S4 verdict adapter
      at it — code motion only, canon-gate remains format authority, still
      no canon-gate dependency. — ✅ crates/canon-ingest/src/openspec_rows.rs
- [x] 1.4 Tests: the S4 verdict adapter's full existing suite passes
      unchanged against the shared module (zero behavior change); registry
      order deterministic; `find` misses return `None` (the CLI layer owns
      the loud error). — ✅ cargo test -p canon-ingest: 201 passed, 0 failed (unittests) + 12 integration

## 2. openspec dialect adapter (P2 — after P1)

- [x] 2.1 `plan_adapters/openspec.rs`: change-dir discovery mirroring
      `discover_task_files`'s root-shape tolerance (a repo root containing
      `openspec/changes/`, a direct changes dir, or a fixture tree), with
      `changes/archive/<basename>/` dirs included and flagged archived. — ✅ `discover_change_dirs`/`list_subdirs`/`is_archived`
- [x] 2.2 Change mapping: `change_id` = dir basename VERBATIM via
      `ChangeId::parse` (grammar failure skips the whole dir, counted,
      siblings unaffected); `title` = slug; `summary` = first paragraph
      under proposal.md `## Why`, whitespace-normalized (absent heading →
      empty summary + diagnostic); missing/unreadable proposal.md → dir
      skipped + counted; proposal-only dir (no tasks.md) → `Change` with
      `status: proposed`, zero tasks, zero diagnostics. — ✅ `parse_change_dir`/`why_summary`
- [x] 2.3 Status derivation (design D6, pure function of the snapshot):
      archive location → `archived` unconditionally; else zero rows →
      `proposed`, all done → `completed`, mixed → `in_progress`, none done
      → `proposed`; DEFERRED/DROPPED rows count by checkbox state alone. — ✅ `derive_status`
- [x] 2.4 Task mapping: each parseable row → `Task` with `task_id` =
      `<change_id>#<n>` via `TaskId::parse` — byte-identical to the S4
      verdict adapter's derivation; `status` from the checkbox verbatim;
      `evidence_note` = ` — ✅ ` suffix, else annotation text, else absent;
      non-checkbox lines ignored as prose; a bad `<n>` skipped + counted. — ✅ `parse_tasks_file` reusing `openspec_rows::task_id_for`
- [x] 2.5 Drop diagnostics (design D3): `specs/**/spec.md`
      `#### Scenario:` blocks → `spec-delta-scenario` count; `design.md` →
      `design-doc` count; zero `Scenario` records emitted anywhere in the
      adapter. — ✅ `count_drop_diagnostics`
- [x] 2.6 Determinism plumbing (design D7): `Task.at` = tasks.md mtime;
      `Change.at` = max mtime over the files read (proposal.md +
      tasks.md); actor fixed `Actor::new_unattributed(
      "canon-plan-import-openspec")`; no `Utc::now()` anywhere in body
      derivation. — ✅ `file_modified_at`/`actor()`
- [x] 2.7 Tests: fixture change tree (live/archive/malformed/
      proposal-only dirs) round-trips per the mapping table; task_id
      DERIVATION parity asserted byte-exact against `openspec_task.rs`
      for every row the verdict adapter emits (its emitted `task_id` set
      a SUBSET of the plan adapter's — the plan side also emits
      untouched-open rows the verdict adapter skips via `NotApplicable`);
      two parses of one snapshot produce byte-identical bodies; drop
      counts named and exact. — ✅ cargo test -p canon-ingest: 213 passed (unittests) + 12 integration

## 3. CLI wiring + persistence (P3 — after P2)

- [x] 3.1 `canon-cli`: `IngestCommand::Plans` beside `Sessions`/
      `Artifacts`; `plans.rs` driver — the one place the plan family meets
      canon-store, mirroring `ingest.rs`/`artifact_ingest.rs`. — ✅ crates/canon-cli/src/plans.rs + main.rs `IngestCommand::Plans`/`run_ingest_plans` + lib.rs registration
- [x] 3.2 `canon.yaml` `plans:` section (`sources: [{dialect, root}]`,
      roots resolved against the `canon.yaml` dir): ABSENT → zero sources,
      clean no-op; PRESENT → strict parse (`deny_unknown_fields`), loud on
      a typo'd key, an unregistered dialect id, or a nonexistent root. — ✅ `plans.rs::load_plan_sources_from_config`/`validate_source_roots`
- [x] 3.3 One-shot override: `--dialect <id> --source <path>` bypasses
      config; either flag alone fails loud; unknown dialect fails loud
      naming registered ids. — ✅ `plans.rs::resolve_sources`/`ensure_dialect_registered`
- [x] 3.4 Watermark gate: one `SourceCursor` per configured (dialect,
      root) source under `canon/ingest/cursors/` (reuses
      `canon_store::cursor`, source-granular content-digest predicate);
      cursor advances ONLY after a fully-persisted pass. — ✅ `plans.rs::run`/`plan_source_cursor_id`
- [x] 3.5 Persistence: every candidate through `TierRegistry::persist`
      with `persist_idempotent`'s DuplicatePath-tolerant discipline;
      unreachable routed tier → the documented `unwritten` seam (printed,
      non-fatal, cursor not advanced); human + `--json` output with
      per-source counts, drop diagnostics, and malformed tallies. — ✅ `plans.rs::persist_or_unwritten`/`build_lenient_tiers`/`format_human`/`format_json`
- [x] 3.6 Tests: absent section no-op; strict-parse failures loud;
      unchanged-source re-run writes zero records (cursor skip); mtime
      churn without byte churn skipped; checkbox flip → exactly the
      refreshed records, winning fold-latest; pg-unreachable → Change
      persisted, Tasks unwritten, cursor not advanced; `canon gate check`
      verdicts byte-identical with/without a prior plan import. — ✅ crates/canon-cli/tests/plans_ingest.rs: cargo test -p canon-cli -p canon-ingest all green
- [x] 3.7 Cross-source `change_id` collision (design D8): the driver
      detects two configured sources yielding the same `change_id` in one
      pass, imports ONLY the first-configured source's records, and skips
      each later occurrence with a NAMED `duplicate-change-id` diagnostic
      count. Test: two `plans:` sources whose trees both carry an
      `add-widget` change dir -> only the first's `Change`/`Task` import,
      the second counted under `duplicate-change-id`, never two competing
      histories in one pass. — ✅ `plans_ingest.rs::cross_source_change_id_collision_first_configured_source_wins`

## 4. closure (P4 — after P1-P3)

- [x] 4.1 Selftest fixture corpus (synthetic openspec change tree:
      live/archive/malformed/proposal-only dirs, rebindable root —
      mirrors s15/s16's rebindable-roots pattern) registered in
      `canon selftest`. — ✅ crates/canon-ingest/src/plan_selftest.rs (`ScratchDir`, two-sided exact-set fact oracle over Change/Task ids+statuses+named drop counts+malformed) registered as the 11th suite in crates/canon-cli/src/selftest.rs (`suites.len() == 11` bumped)
- [x] 4.2 Fixture SECOND dialect adapter registered in tests, proving the
      one-registry-entry extension seam (design D9's structural proof —
      the diff touches one registry entry + one module, nothing else). — ✅ crates/canon-ingest/tests/plan_fixture_dialect_seam.rs (`FixtureLineDialectAdapter`, test-local registry beside the untouched production `openspec` entry)
- [x] 4.3 Companion skill `canon/skills/canon-plan-import/SKILL.md`
      (configure a `plans:` source → import → query Change/Task rows →
      join against S4 verdict trajectories on `task_id`, openspec worked
      example; the deferred superpowers/donor-JSON waves named);
      materialize via `canon skills install` + install-lock bump. — ✅ canon/skills/canon-plan-import/SKILL.md + `.claude/skills/canon-plan-import/SKILL.md` + `.codex/skills/canon-plan-import.md` + `.install-lock.json` bumped (v1); reinstall verified byte-identical
- [x] 4.4 Doc reconciliation: s15's "S4 donor-JSON adapters are
      reclassified as s17 connectors" pointer and s16's "s17 is the
      remaining sibling" pointers read true against what actually got
      built; module docs in `openspec_task.rs`/`plan_adapters/openspec.rs`
      cross-reference the two-readers-one-join relationship (design R5). — ✅ verified via `git log 53451521..06b34323` (zero commits touch ledger/divergence/handoff/review/native_divergence.rs, canon-plugin/, or s15/s16's own openspec docs); reciprocal R5 cross-reference added to openspec_task.rs's module doc (plan_adapters/openspec.rs already carried its half)

## 5. Verification

- [x] 5.1 `cargo build --workspace` + `cargo clippy --workspace
      --all-targets -- -D warnings` + `cargo test --workspace
      --no-fail-fast` (bare, no pipe masking) all green. — ✅ all three commands run bare, zero warnings/failures across every crate
- [x] 5.2 `bunx openspec validate --strict s17-plan-import` green. — ✅ "Change 's17-plan-import' is valid"
- [x] 5.3 `canon selftest` all suites green, including the new
      plan-import fixture corpus. — ✅ `canon selftest`: 11/11 suites `ok`, plan-import 3 check(s)
- [x] 5.4 Structural invariants re-asserted green: `RecordKind::ALL.len()
      == 12` at all three assertion sites; no canon-gate/canon-learn
      source reference to the plan family. — ✅ `all_twelve_kinds_present_exactly_once`, `context::` surface-kinds test, `git_tier_all_kinds` all green; grep confirms zero `plan_registry`/`PlanAdapter` reference in canon-gate/canon-learn src

