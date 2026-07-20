# Tasks — s23 durable import diagnostics (watermark-on-failure)

Single-crate change (`canon-cli`), no phased sequencing (design.md).

## 1. Shared malformed-nonzero-zero-persisted predicate

- [x] 1.1 `canon-cli/src/plans.rs`'s per-source loop: introduce ONE local
      (e.g. `let malformed_zero_persisted = !malformed.is_empty() &&
      changes_persisted == 0 && tasks_persisted == 0;`) computed once, at
      the point `changes_persisted`/`tasks_persisted`/`malformed` are all
      already known (after the persist loop, before both the cursor-advance
      decision and the existing `non_clean_sources` push).
- [x] 1.2 Rewrite `plans.rs:400`'s existing `non_clean_sources` condition to
      read the new shared local instead of re-stating the expression inline
      — same runtime behavior, one fewer independently-maintained copy
      (design.md R2).

## 2. Cursor-advance exclusion

- [x] 2.1 `canon-cli/src/plans.rs:380`: change `fully_durable`'s definition
      from `changes_unwritten == 0 && tasks_unwritten == 0` to
      `changes_unwritten == 0 && tasks_unwritten == 0 &&
      !malformed_zero_persisted` — reusing task 1.1's shared local. No
      other line in the `if fully_durable { .. }` cursor-write block
      (`plans.rs:381-388`) changes.
- [x] 2.2 Confirm (by inspection, and by test 3.2 below) that a clean pass
      (`malformed.is_empty()`) and a partial-success pass
      (`malformed.len() > 0 && (changes_persisted > 0 ||
      tasks_persisted > 0)`) both still satisfy the rewritten
      `fully_durable` exactly as before this change — `!malformed_zero_persisted`
      is `true` (a no-op conjunct) for both.
- [x] 2.3 Update `plans.rs`'s module doc (the "Watermark cursor" section,
      `plans.rs:92-96`, "A source with ANY unwritten candidate this pass
      does NOT advance its cursor") to also name the malformed-zero-persisted
      exclusion this change adds, so the doc comment stays the accurate
      single source of truth for what blocks a cursor advance.

## 3. Tests

- [x] 3.1 `canon-cli/tests/plans_ingest.rs` (or wherever s18's
      root-near-miss/non-clean-source fixture already lives): extend the
      existing `root: openspec`-one-level-too-high fixture test to run
      `canon ingest plans` **three times in succession** against the SAME
      unfixed config and assert run #1, run #2, AND run #3 each: print the
      named `missing-proposal-md` malformed entry + root-near-miss hint,
      print the unconditional stderr WARN, and exit non-zero — proving the
      watermark never silences the diagnostic on any subsequent run (the
      exact regression Developer's live evidence reproduced).
- [x] 3.2 New/extended test: a source with `malformed > 0` but
      `changes_persisted > 0` (s18's existing partial-success fixture) run
      **twice** in succession — run #1 persists + is non-clean-flagged per
      s18 (unchanged from today); run #2 hits `skipped_unchanged: true`,
      `exit 0`, no WARN — proving this change's exclusion does NOT widen to
      "any malformed entry blocks the cursor."
- [x] 3.3 New/extended test: a genuinely empty, well-formed source
      (`malformed.is_empty()`, `changes_parsed == 0`) run twice — both runs
      exit `0`; run #2 specifically asserts `skipped_unchanged: true` —
      proving s18's "a legitimately empty source stays a clean silent
      no-op" scenario is unaffected by this change.
- [x] 3.4 New test: a source that fails malformed-zero-persisted on run #1,
      then is FIXED (a real `openspec/changes/<id>/proposal.md` added, or
      `root:` corrected) before run #2 — run #2 is a normal fully-durable
      pass (real records persist, `malformed` drops to its true remaining
      count), its cursor IS written, and run #3 (config unchanged again)
      hits `skipped_unchanged: true`, `exit 0` — proving the fix-then-quiet
      transition works with no manual cursor-reset step (design.md's
      "Preserving idempotence" section).
- [ ] 3.5 Unit test on the extracted local (task 1.1) -- N/A: `plans.rs`
      has no pre-existing `#[cfg(test)] mod tests` covering pure
      per-source-outcome construction (task's own stated precondition),
      so this conditional task does not apply; the local is exercised
      end-to-end by 3.1-3.4's integration tests instead.

## 4. Closure

- [ ] 4.1 `cargo build --workspace` + `cargo clippy --workspace --all-targets
      -- -D warnings` + `cargo test --workspace --no-fail-fast` (bare, no
      pipe masking) all green.
- [ ] 4.2 `bunx openspec validate --strict s23-durable-import-diagnostics`
      green.
- [x] 4.3 Re-run (unmodified expectations) s18's existing
      `loud-plan-import-diagnostics` acceptance tests — confirm run #1
      behavior (named malformed entry, hint, WARN, non-zero exit) is
      byte-identical to before this change; only run #2+ behavior against
      an UNCHANGED malformed source differs.
- [x] 4.4 Structural invariants re-asserted green: `RecordKind::ALL.len()
      == 12` at all three assertion sites; `canon gate check` verdicts
      byte-identical with/without a plan import (`canon-cli/tests/
      plans_ingest.rs`'s existing byte-identity test, unmodified) —
      connector-never-authority unaffected by this change.
- [ ] 4.5 `canon selftest` all suites green.
