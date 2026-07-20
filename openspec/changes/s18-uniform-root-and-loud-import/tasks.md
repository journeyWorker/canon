# Tasks — s18 uniform root resolution + loud plan-import diagnostics

Two independent surface fixes, each closing one SYNTHESIS blocker; neither
depends on the other, so P1 (B2, `canon query`) and P2 (B1, `canon ingest
plans`) may land in either order or in parallel. P3 closes both.

## 1. `canon query` accepts `--repo` via the shared ancestor walk (B2)

- [x] 1.1 `canon-cli/src/main.rs`: change `Command::Query`'s `canon_yaml`
      field to `Option<PathBuf>` (`#[arg(long)]`, no default) and add
      `repo: PathBuf` (`#[arg(long, default_value = ".")]`), doc comment
      cross-referencing `resolve_repo_root` the same way `Command::Ingest`'s
      `Plans::repo` already does.
- [x] 1.2 `canon-cli/src/query.rs`: `run`'s signature resolves the
      `canon.yaml` path from `(repo, canon_yaml)` before calling
      `tiers::build_tiers` — `canon_yaml.is_some()` uses that literal path
      AS-IS (no walk); `None` resolves
      `context::resolve_repo_root(&repo).join("canon.yaml")`. `run_with_
      plugin` takes the identical resolved path (no plugin-path divergence).
- [x] 1.3 `main.rs::run_query`/`run_query_with_plugin` thread the new
      `repo`/`canon_yaml` pair through to `canon_cli::query::run`/
      `run_with_plugin` unchanged otherwise (same `--json`/`--plugin`
      branching, same output functions).
- [x] 1.4 Tests: `canon query` from a subdirectory of a `canon.yaml`-rooted
      repo (no flags) resolves and succeeds; explicit `--canon-yaml <path>`
      still resolves that literal path AS-IS even when cwd is elsewhere
      (back-compat, no ancestor walk applied to it); explicit `--repo <dir>`
      (non-`.`) is used as-is, no walk (mirrors `resolve_repo_root`'s own
      `repo != "."` short-circuit, pinned by that function's existing
      tests); `--repo` and `--canon-yaml` both supplied →
      `--canon-yaml` wins, asserted by a fixture where the two would
      resolve to DIFFERENT files.
- [x] 1.5 `canon query --help` reviewed: `--canon-yaml`'s help text updated
      to state it is an explicit override that bypasses `--repo`'s
      ancestor walk (no longer implies it is the only/default resolution
      path).

## 2. `canon ingest plans` names malformed constructs and stops exiting 0 on a wholly-unproductive pass (B1)

- [x] 2.1 `canon-ingest`: `plan_adapters/openspec.rs`'s `parse_change_dir`
      (and any other malformed-incrementing site in the module) records a
      path + reason per malformed construct — `unreadable-directory`,
      `invalid-change-id-grammar`, `missing-proposal-md` — on
      `PlanParseOutcome`, alongside (or replacing, if `usize` becomes
      derived from `.len()`) the existing bare `malformed: usize` count;
      `PlanAdapter`'s shared outcome shape carries this for every dialect,
      not an openspec-only field.
- [x] 2.2 Root-near-miss hint: a `missing-proposal-md` entry whose
      directory basename is exactly `changes` carries an additional
      actionable hint noting the configured `root:` may be pointing at the
      changes directory's PARENT rather than at (or above)
      `openspec/changes` itself — a targeted heuristic on the one concrete
      near-miss signature the SYNTHESIS reproduces, not a fuzzy general
      guess.
- [x] 2.3 `canon-cli/src/plans.rs`: `PlanSourceSummary` carries the named
      malformed entries (path + reason [+ hint]); `format_human` prints
      each one under its source's summary line (mirroring the existing
      `dropped (<construct>): <count>` per-line convention); `format_json`
      serializes the full list, not just the count.
- [x] 2.4 `plans.rs::run`: after building each source's `PlanSourceSummary`,
      detect `malformed > 0 && changes_persisted == 0 && tasks_persisted ==
      0` for that source; surface the condition on `PlansOutcome` (e.g. a
      `non_clean_sources: Vec<…>` naming dialect + root + malformed count)
      for the CLI layer to act on. A legitimately empty/fresh source
      (`malformed == 0`) is NEVER flagged by this check.
- [x] 2.5 `main.rs::run_ingest_plans`: when `PlansOutcome` carries any
      non-clean source, print an unconditional stderr WARN per flagged
      source (dialect + root + malformed count + a `root:` hint pointer),
      regardless of `--json` — mirroring the function's existing
      always-print-`unwritten` precedent — and return a non-zero
      `ExitCode` instead of the current unconditional `ExitCode::SUCCESS`
      on `Ok(_)`. A pass with zero flagged sources keeps exiting `0`
      exactly as today.
- [x] 2.6 Tests: the live-repro shape (`root:` one level above
      `openspec/changes`, one real change dir beneath it) now exits
      non-zero, prints the stderr WARN naming the source + malformed count
      + the `changes`-basename hint, and the malformed entry in `--json`
      output carries the path + `missing-proposal-md` reason; a
      legitimately empty source (`malformed == 0`, zero changes found)
      still exits `0` with a clean no-op summary, unchanged from today; a
      source with SOME malformed dirs but at least one persisted record
      (`persisted > 0`) still exits `0` (partial success is not the
      targeted near-miss); a multi-source pass where only ONE source is
      flagged still exits non-zero overall while the other source's
      records persist normally (the flag is per-source, the exit is
      pass-wide).

## 3. Verification

- [x] 3.1 `cargo build --workspace` + `cargo clippy --workspace
      --all-targets -- -D warnings` + `cargo test --workspace
      --no-fail-fast` (bare, no pipe masking) all green. DONE (Wave-1
      re-verification): all three commands re-run clean after the
      Wave-1 review fixes landed (relative `MalformedEntry.path` +
      task_id-scoped `malformed-scenario-ref` diagnostic).
- [x] 3.2 `bunx openspec validate --strict
      s18-uniform-root-and-loud-import` green. DONE (Wave-1
      re-verification).
- [x] 3.3 `canon selftest` all suites green (no new suite added — this
      change touches two existing CLI surfaces, not a new fixture family).
      DONE (Wave-1 re-verification): `plan-import (3 check(s))` green
      alongside every other registered suite.
- [x] 3.4 Manual re-run of both SYNTHESIS repros against the rebuilt
      binary: `canon query --kind change` from a subdirectory now
      succeeds; `canon ingest plans` against the `root: openspec`
      misconfiguration now exits non-zero with the named diagnostic on
      stderr.
- [x] 3.5 s17's own test suites
      (`crates/canon-ingest`/`crates/canon-cli/tests/plans_ingest.rs`)
      re-run green unchanged — the malformed-detail enrichment must not
      alter any EXISTING assertion about malformed COUNTS, only add named
      detail alongside them.
