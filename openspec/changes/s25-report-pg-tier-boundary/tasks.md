# Tasks — s25 report pg-tier boundary

Sequencing follows design.md: **P1 (tier_boundary.rs) before P2 (render.rs) before P3 (lib.rs)**; P4 (main.rs stderr WARN) needs only P1 and may land in parallel with P2/P3; P5 (views.sql doc comment) is fully independent; P6 (tests) depends on P1-P4; P7 (closure) depends on all.

## 1. `tier_boundary` module (P1)

- [x] 1.1 `crates/canon-report/src/tier_boundary.rs` (new file): `pub fn pg_routed_kinds(repo_root: &Path) -> Vec<RecordKind>` — reads `<repo_root>/canon.yaml`, parses via `canon_store::policy::TierPolicy::from_yaml`, filters `policy.routing` entries where the tier is `TierKind::Pg`, returns the kinds sorted ascending by `RecordKind::as_str()`. Fail-soft to an empty `Vec` (never a panic/`Err`) on a missing file or a parse failure (design D1/R1) — mirrors `crate::digest::DigestHeader::compute`'s existing `<repo_root>/canon/policy.yaml` precedent.
- [x] 1.2 Same file: `pub fn render_note(kinds: &[RecordKind]) -> Option<String>` — `None` when `kinds` is empty; otherwise a `## Tiers not reflected` markdown fragment (design D2's exact fixed text: heading, one blockquote sentence naming git+r2 as the marts' sources and `canon query --kind <kind>` as the escape hatch, one `` - `<kind>` `` bullet per entry in `kinds`' given order — callers pass the already-sorted `pg_routed_kinds` result, this function never re-sorts).
- [x] 1.3 Same file: `pub fn warn_line(kinds: &[RecordKind]) -> Option<String>` — `None` when `kinds` is empty; otherwise one line naming git+r2 as the marts' sources, `canon query --kind <kind>` as the escape hatch, and a comma-joined list of `kinds` (in the given order) — no `canon report: WARN ` prefix (the CLI caller adds that, matching every other stderr line's own prefix convention in `main.rs::run_report`).
- [x] 1.4 Module-doc `tier_boundary.rs` per this crate's existing convention (every module in this crate module-docs its own provenance/invariant — see `crate::digest`, `crate::check`, `crate::marts`): name the F2 finding, the config-derived/never-live-pg invariant (design D1), and the "note and warn_line always agree because both are `pg_routed_kinds`'s single derivation" invariant (design D3/R2).
- [x] 1.5 Unit tests in `tier_boundary.rs`'s own `#[cfg(test)] mod tests`: no-`canon.yaml` → empty; malformed `canon.yaml` → empty (never a panic); all-git `routing` → empty; a multi-tier `routing` → the correct sorted kind set; `render_note`/`warn_line` name the identical sorted kinds; two `pg_routed_kinds` calls over an unchanged file are equal.

## 2. Render the boundary section (P2 — after 1)

- [x] 2.1 `crates/canon-report/src/render.rs::render`: add a `pg_routed_kinds: &[RecordKind]` parameter (last positional parameter, after `marts`) and import `canon_model::envelope::RecordKind` + `crate::tier_boundary`.
- [x] 2.2 Same fn: immediately after the existing `## Inputs (digest)` block (the `digest.ledger_hash` line + its trailing blank line) and before the existing `## Trust matrix` block, insert `if let Some(note) = tier_boundary::render_note(pg_routed_kinds) { out.push_str(&note); }` — no other panel's rendering code is touched (design D2: purely additive, byte-identical output for an empty `pg_routed_kinds`).
- [x] 2.3 Update `render`'s own doc comment to name the new parameter and its config-derived-not-live provenance (matching this crate's per-fn doc-comment convention).

## 3. Wire into `report()` (P3 — after 1, 2)

- [x] 3.1 `crates/canon-report/src/lib.rs::report`: compute `let pg_routed_kinds = tier_boundary::pg_routed_kinds(&inputs.repo_root);` after the existing `marts` construction, and pass it as `render::render`'s third argument. Add `pub mod tier_boundary;` to the crate's module list alongside the existing `pub mod` declarations.
- [x] 3.2 No `ReportInputs` field change (design D3) — confirm `crates/canon-report/src/bin/canon-report.rs` and any other `ReportInputs::new` call site compiles unmodified.

## 4. CLI stderr WARN (P4 — after 1, independent of 2/3)

- [x] 4.1 `crates/canon-cli/src/main.rs::run_report`: immediately after `canon_cli::report::resolve_inputs(repo)` resolves `(repo, inputs)`, and BEFORE dispatching to any of `--snapshot`/`--check`/the flagless write, call `canon_report::tier_boundary::{pg_routed_kinds, warn_line}` over the resolved `repo` and, when `Some(msg)`, `eprintln!("canon report: WARN {msg}")`.
- [x] 4.2 Update `run_report`'s doc comment to name this new stderr behavior and the "same `pg_routed_kinds` derivation as the markdown note, so they can never disagree" invariant (design D3).

## 5. `views.sql` doc comment (P5 — independent of 1-4)

- [x] 5.1 `crates/canon-store/sql/views.sql`: extend `stg_records`'s existing doc comment (immediately above `CREATE OR REPLACE VIEW stg_records AS`) to state explicitly that the `pg` tier is intentionally NOT staged here — name `canon query --kind <k>` as the live-tier read path, name the new `crates/canon-report/src/tier_boundary.rs::pg_routed_kinds` as the mechanism that surfaces this gap LOUD in `canon report`'s own output, and cross-reference this change's own capability id (`report-pg-tier-boundary`) — matching the file's own established "name every stub/proxy/gap it contains" convention (`int_evidence_verdicts`'s STUB note is the direct precedent). No `CREATE VIEW`/`CREATE OR REPLACE VIEW` statement is touched.

## 6. Tests (P6 — after 1-4)

- [x] 6.1 New `crates/canon-report/tests/tier_boundary.rs`: (a) a multi-tier `canon.yaml` (routing at least one kind to `pg`, others to `git`) renders `## Tiers not reflected` naming exactly the `pg`-routed kinds, sorted, and never names a git-routed kind; (b) two `report()` calls over the identical multi-tier fixture are byte-identical; (c) a repo with no `canon.yaml` at all (the existing fixture-corpus default every other test already uses) renders NO `## Tiers not reflected` section; (d) a `canon.yaml` whose `routing` never touches `pg` also renders no section.
- [x] 6.2 New `crates/canon-cli/tests/report_tier_boundary.rs` (invokes the built `canon` binary, matching `tests/report.rs`'s own discipline): (a) a multi-tier repo's `canon report --repo .` exits 0 and its stderr contains `canon report: WARN` naming every `pg`-routed kind, and the written `canon/REPORT.md` names the SAME kinds in its `## Tiers not reflected` section; (b) a git-only repo (no `canon.yaml`) emits no `WARN` line at all.
- [x] 6.3 Confirm every PRE-EXISTING test in `crates/canon-report/tests/` and `crates/canon-cli/tests/` (`byte_stability.rs`, `fresh_repo.rs`, `marts.rs`, `snapshot.rs`, `check_gate.rs`, `gate_independence.rs`, `tests/report.rs`, …) stays green UNMODIFIED — none of their fixture repos write a `canon.yaml`, so `pg_routed_kinds` returns empty for every one of them and the new section/warning never fires (design.md Impact/P6).

## 7. Closure

- [ ] 7.1 `cargo build --workspace` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo test --workspace --no-fail-fast` (bare, no pipe masking) all green. **DEVIATION: out of this wave's scoped-verification mandate (crate-scoped `-p canon-report -p canon-cli` only, per operator instruction); not run by this implementer. `cargo build -p canon-report -p canon-cli` + `cargo test -p canon-report -p canon-cli` + `cargo clippy -p canon-report -p canon-cli --all-targets -- -D warnings` are all green.**
- [ ] 7.2 `bunx openspec validate --strict s25-report-pg-tier-boundary` green. **DEVIATION: not run by this implementer (openspec CLI validation is a closure-phase/orchestrator step, out of this wave's crate-scoped verification mandate); spec/design/tasks docs were followed to the letter above.**
- [x] 7.3 Manually run `canon report --repo <a multi-tier fixture repo whose canon.yaml routes ≥1 kind to pg>` and confirm the `## Tiers not reflected` section renders naming that kind plus a `canon report: WARN` stderr line; run the identical command a second time and confirm byte-identical `canon/REPORT.md` (i.e. `canon report --check` reports no drift) — the exact end-to-end proof the acceptance scenarios ask for.
- [x] 7.4 Structural invariants re-asserted green: `RecordKind::ALL.len() == 12` at all three assertion sites; `canon gate check` byte-identity acceptance tests (`crates/canon-report/tests/gate_independence.rs`, `crates/canon-cli` gate tests) unmodified and still green — this change touches no `canon-gate` source file, confirming "read-only reporting, never a gate input" holds for the new boundary note exactly as it already holds for every existing panel.
