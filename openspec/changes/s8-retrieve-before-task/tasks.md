## 1. Retrieval command

- [x] 1.1 Implement `canon retrieve --role <r> --regime <k> [--k <n>]`:
      calls S6's `search_similar_strategies` scoped by role, queried via
      `regime_key(role, repo, area, hash)`.
      (`crates/canon-cli/src/retrieve.rs::run` + `main.rs`'s `Retrieve`
      arm — the CLI surface over `canon_learn::guidance::
      retrieve_guidance`, which wraps S6's ACTUAL shipped exact-
      `regime_key`-match `crate::retrieve::retrieve`, not the design
      doc's aspirational `search_similar_strategies` (reconciled per
      `guidance.rs`'s own module doc — no second reconciliation here).
      `--role`/`--regime`/`--k`/`--repo`/`--json` all wired; a `--role`/
      `--regime` disagreement is a clean usage error (exit `2`), never
      reachable through `retrieve_guidance` itself. Proven by 7 unit
      tests (`src/retrieve.rs::tests`) + 6 real-binary integration
      tests (`tests/retrieve.rs`: seeded store returns the snapshot,
      nonexistent store prints empty + exits `0`, `--k` cap, role/
      regime mismatch, malformed `--regime` via clap, `--help`).
      `cargo build/test/clippy -p canon-cli` all green; `canon
      retrieve --help` and a real-binary smoke run verified by hand.)
- [x] 1.2 Enforce fail-soft at the type boundary: the public signature
      returns `Vec<StrategyRef>` with no error channel; internal errors
      are caught + logged, never propagated.
      (`crates/canon-learn/src/guidance.rs::retrieve_guidance` — returns
      `Vec<StrategyRef>`, no `Result`; every `crate::retrieve::retrieve`
      error (store outage, malformed row) is caught and logged via
      `eprintln!` (this crate's established non-`tracing` diagnostic
      convention), never propagated. Proven by
      `guidance::tests::retrieve_guidance_on_a_store_outage_returns_
      an_empty_vec_never_panics_or_errors` against a synthetic
      always-failing `StrategyStore`.)
- [x] 1.3 Exclude `status: demoted` strategies from retrieval results
      (restates S7's demotion contract on the read side).
      (`crates/canon-learn/src/guidance.rs::retrieve_guidance` filters
      out every item with `demotion.is_some()` before capping at `k`.
      Proven by
      `guidance::tests::retrieve_guidance_excludes_a_demoted_strategy`.
      The `canon retrieve` CLI now calls this directly — see task 1.1 —
      so the exclusion is exercised end-to-end through the real binary
      too, not library-only anymore.)

## 2. Manifest guidance recording

- [x] 2.1 Add `injected_guidance: Vec<StrategyRef>` to S1's `Run`/manifest
      record (`StrategyRef = {strategy_id, title, content}`, a full
      snapshot, not a live pointer).
      (`crates/canon-model/src/records.rs::{StrategyRef, Run::
      injected_guidance}` — `#[serde(default, skip_serializing_if =
      "Vec::is_empty")]`, additive/backward-compatible: `Run::new`'s
      signature is UNCHANGED (defaults to empty), a new
      `Run::with_injected_guidance` builder sets it. Proven by
      `records::tests::{run_with_injected_guidance_round_trips,
      run_without_injected_guidance_key_deserializes_empty_and_
      reserializes_without_the_key}` — an old manifest with no key
      deserializes empty AND reserializes without introducing the key.
      Schemas regenerated via `cargo xtask write`
      (`schemas/run.schema.json`); `cargo build/test -p canon-cli
      -p canon-vocab -p canon-ingest` verified green — no Run
      construction site outside canon-model breaks.)
- [x] 2.2 Implement `manifest_guidance_for_replay(manifest) ->
      Vec<StrategyRef>`: returns the recorded field unchanged — the named
      replay boundary, never a fresh `canon retrieve` call.
      (`crates/canon-learn/src/guidance.rs::manifest_guidance_for_
      replay` — takes `&Run` only (no store handle), so a live lookup
      is impossible by construction, not just by convention. Proven by
      `guidance::tests::manifest_guidance_for_replay_returns_the_
      recorded_snapshot_verbatim_even_after_the_source_is_demoted`
      (record guidance, demote the source, assert replay still yields
      the byte-identical original snapshot while a fresh retrieval
      excludes it) and
      `an_old_manifest_without_injected_guidance_replays_empty`.)
- [x] 2.3 Wire the dispatch-time write: `canon retrieve` output is written
      into `injected_guidance` exactly once per run, at dispatch.
      — ✅ `crates/canon-cli/src/dispatch.rs` (`canon dispatch begin
      --role <r> --regime <k> [--repo] [--agent-id]`): `begin()` mints a
      `Run` (status Running), retrieves the role+regime guidance ONCE via
      `canon_learn::retrieve_guidance`, records it verbatim into
      `Run::injected_guidance`, and persists the manifest to the private
      side-channel `<repo>/.canon/dispatch/<run_id>.json` (NOT the git
      tier — a live dispatch Run and the post-hoc ingest Run for the same
      session would collide on the Hive path; the module doc explains the
      reconciliation seam). Fail-soft retrieval, fail-loud write. Test
      `tests/dispatch.rs::dispatch_begin_records_retrieved_guidance_into_the_manifest`.

## 3. Pre-dispatch hook wiring

- [x] 3.1 Implement the generic pre-dispatch hook script invoking `canon
      retrieve`, reusing S5's `.claude/settings.json` /
      `.codex/hooks.json` entry shape.
      (`canon/skills/canon-retrieve/pre-dispatch.sh` — POSIX `sh`,
      fail-soft/advisory-only, mirrors `pre-edit-pattern-lookup.ts`'s
      contract: fires on a `Task`-shaped PreToolUse dispatch, derives
      `--role` from the tool's `subagent_type` and `--regime` from a
      documented conservative default (overridable via
      `CANON_RETRIEVE_ROLE`/`_AREA`/`_HASH`/`_REGIME`), calls `canon
      retrieve --json`, emits a `PreToolUse`
      `hookSpecificOutput.additionalContext` JSON envelope (Claude
      Code's own documented convention, matching the donor CLI's own
      hooks' identical envelope shape) — never blocks, always exits
      `0`. WIRING reuses the ALREADY-GENERIC `canon gate install-hooks`
      (S5) completely unmodified — zero new installer code, zero
      second wiring convention (design decision 4):
      `canon gate install-hooks --repo . --event PreToolUse --matcher
      Task --command "sh <path-to-script>" --timeout 15`. Proven by 4
      real-subprocess integration tests
      (`crates/canon-cli/tests/pre_dispatch_hook.rs`: seeded guidance
      surfaces as `additionalContext`, no guidance stored is silent, a
      non-`Task` tool call is silent, `canon`/`jq` missing from `PATH`
      is silent — all exit `0`).)
- [ ] 3.2 Wire the donor monorepo: add the pre-dispatch hook entry alongside the
      existing `pre-edit-pattern-lookup.ts` hook (additive, not a
      replacement).
      (**Deferred, out of this task's territory**: S8-part2's scope is
      `crates/canon-cli/src/**` + `canon/skills/**` inside the canon
      repo itself; the donor monorepo's own `.claude/settings.json`/`.codex/
      hooks.json` and the donor harness are a DIFFERENT
      repo tree, not reachable from this worktree — the exact same
      migration-target boundary `canon-gate`'s own `hooks.rs` module
      doc already establishes for S5's task 4.3: "wiring the donor monorepo's own
      `.claude/settings.json`/`.codex/hooks.json` ... is a documented,
      separate follow-up". `canon gate install-hooks`/the pre-dispatch
      script (task 3.1) ship the mechanism; actually running it
      against the donor monorepo's real settings is the donor monorepo's own follow-up change.)
- [x] 3.3 Route the hook's auto-derived `--regime` through canon's OWN
      `regime_key` serializer, never a second shell derivation
      (whole-branch-review finding, `s8-retrieve-before-task`).
      (Adds `canon regime-key --role/--repo/--area/--hash`
      (`crates/canon-cli/src/main.rs::run_regime_key`) — the shell-
      facing counterpart to `canon_model::ids::regime_key`, the ONE
      serializer S4/S6/S14's Rust write path already calls — and
      rewrites `pre-dispatch.sh` to assemble `--regime` through it
      (`canon/skills/canon-retrieve/pre-dispatch.sh`). The prior local
      `slugify` collapsed the repo segment with `tr -c 'a-z0-9' '-'`,
      mapping a WRITTEN `dev/my_repo/...` to a QUERIED `dev/my-repo/...`
      — a silent `retrieve_guidance` miss on any repo/area segment
      containing `_`, `.`, or non-ASCII, since the Rust canonicalizer
      lowercases + collapses only whitespace/`/` and preserves every
      other character. `slugify` now derives ONLY the kebab `RoleId`
      (a fixed point of the Rust canonicalizer, so `--role` still
      equals the assembled key's leading segment). `canon regime-key`
      validates via `RegimeKey::parse` and exits `2` on a malformed
      result, which the hook's `|| true` degrades to its standard
      silent no-op. Proven by `crates/canon-cli/tests/pre_dispatch_
      hook.rs::pre_dispatch_hook_regime_preserves_underscores_matching_
      the_rust_write_path` (a `git init`ed `my_repo_v2` subdir:
      guidance seeded under the Rust write-path key surfaces through
      the hook — the pre-fix slugify would have queried `my-repo-v2`
      and missed), plus the two existing seed-and-surface tests updated
      to seed under the raw basename the write path uses, never a slug.)

## 4. Fixtures + selftest

- [x] 4.1 Build a fixture run whose manifest embeds retrieved guidance
      end-to-end (retrieve → dispatch → manifest write).
      — ✅ `crates/canon-cli/tests/dispatch.rs::dispatch_begin_records_retrieved_guidance_into_the_manifest`
      seeds a strategy, runs the real `canon dispatch begin` end-to-end
      (retrieve → mint Run → write manifest), and asserts the written
      `.canon/dispatch/<run_id>.json` embeds the retrieved guidance in
      `injected_guidance` (2.3's write seam now exists).
- [x] 4.2 Build a replay fixture: change the store's content after the
      original run, replay the manifest, assert the run's inputs
      (including `injected_guidance`) are byte-identical to the original.
      — ✅ `crates/canon-cli/tests/dispatch.rs::a_recorded_manifest_replays_verbatim_even_after_the_source_is_demoted`
      dispatches, demotes the source strategy, then replays the written
      manifest and asserts `injected_guidance` is byte-identical — the
      end-to-end complement to the library-level
      `guidance::tests::manifest_guidance_for_replay_returns_the_recorded_snapshot_verbatim_even_after_the_source_is_demoted`.
- [x] 4.3 Build a fixture proving a demoted strategy is excluded from a
      NEW retrieval but still present verbatim in an OLD manifest's
      replay (both halves of the reproducibility guarantee).
      — ✅ Both halves proven end-to-end: the OLD-manifest half by
      `tests/dispatch.rs::a_recorded_manifest_replays_verbatim_even_after_the_source_is_demoted`
      (the demoted strategy stays verbatim in the already-written
      manifest); the NEW-retrieval-excludes-demoted half through the real
      `canon retrieve` binary (task 1.3 / `retrieve::tests`).

## 5. Companion skill

- [x] 5.1 Author the `canon-retrieve` companion skill under
      `canon/skills/` — `canon retrieve` usage, reading
      `injected_guidance` in a manifest, the replay-vs-live-retrieval
      boundary.
      (`canon/skills/canon-retrieve/SKILL.md` — covers `canon
      retrieve`'s full flag surface, the pre-dispatch hook + its
      `canon gate install-hooks` wiring recipe, the replay-vs-live-
      retrieval boundary (`Run.injected_guidance`/
      `manifest_guidance_for_replay`), and an explicit "Deferred"
      section naming the task-2.3 gap honestly. Materialized via
      `canon skills install --source canon/skills --target .` —
      `.claude/skills/canon-retrieve/SKILL.md` (byte-verbatim),
      `.codex/skills/canon-retrieve.md` (flattened), and `canon/
      skills/.install-lock.json` bumped to `canon-retrieve` v1;
      re-running the install a second time reports "unchanged"
      (idempotent, no drift).)
