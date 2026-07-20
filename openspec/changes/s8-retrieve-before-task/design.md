## Context

The donor tuning project's `optimize.ts` / `sweep-manifest.ts` already prove the full
pattern end-to-end for ONE role-shaped domain (`sim` sweeps): a NEW run
calls `evaluatePreSweepLookup` (live retrieval, regime-scoped, fail-soft —
its own doc notes it adds `Effect.catchAllDefect` alongside `Effect.
catchAll` so it "always resolves, never fails the enclosing sweep"); the
guidance is recorded into a `SweepManifest.injectedGuidance` field; a REPLAY
calls `manifestGuidanceForReplay(manifest)` instead, which returns
`injectedGuidance` UNCHANGED — no live call. The donor harness's own
`pre-edit-pattern-lookup.ts` hook independently proves the same fail-soft,
advisory-only retrieval contract at the tool-call level (PreToolUse hook).
S8 generalizes both into one role-agnostic command + manifest contract any
dispatch surface can call — not just sim sweeps, not just PreToolUse hooks.

## Goals / Non-Goals

**Goals:**
- Ship `canon retrieve --role <r> --regime <k> [--k <n>]`: top-k
  `StrategyItem` search scoped by role+regime (reusing S6's `regime_key`
  grammar and search surface), returning strategies + guardrails — a
  guardrail is simply a `StrategyItem` distilled from a failure-polarity
  verdict per S4's table, not a separate type.
- Guarantee retrieval NEVER blocks and NEVER gates reproducibility: a
  store outage, timeout, or malformed row returns empty guidance, never an
  error the caller must handle as fatal — mirrors
  `evaluatePreSweepLookup`'s "always resolves" contract and the donor monorepo's
  PreToolUse hook fail-soft convention.
- Guarantee retrieved guidance is recorded VERBATIM into the run manifest
  at dispatch time; a later replay of that manifest re-injects the SAME
  guidance, never a fresh lookup — byte-identical reproducibility
  regardless of live memory state at replay time.
- Exclude demoted strategies (`status: demoted`) from retrieval —
  restates S7's demotion contract as a hard requirement on the read side.
- Ship the generic pre-dispatch hook script + the donor CLI's wiring for the donor monorepo,
  mirroring S5's hook-seam wiring shape.

**Non-Goals:**
- Strategy ranking/relevance improvements beyond S6's existing
  cosine-similarity search — S8 is a thin retrieval + recording layer, not
  a new ranking algorithm.
- Building the manifest schema from scratch — the manifest IS S1's
  `run_id`-keyed record; S8 only adds the `injected_guidance`-equivalent
  field and the read/write functions around it.
- Any write path back into S6's store — S8 is read-only against
  strategies; promotion/demotion writes are S6/S7's job.

## Decisions

1. **`canon retrieve` is a thin, fail-soft wrapper over S6's search, not a
   new store.** `canon retrieve --role <r> --regime <k>` calls S6's
   `search_similar_strategies` scoped to `role`, queried with `regime_key
   (role, repo, area, hash)` — the SAME canonical serialization S6
   defines, no second key derivation — wrapped so EVERY error path (store
   timeout, malformed row per §7 "malformed evidence is no evidence")
   returns an empty guidance list rather than propagating. *Alternative
   rejected:* retrieval as its own service with its own cache/index —
   would duplicate S6's regime-key/search surface and risk the two
   drifting, the exact join-spine failure design doc §1 exists to
   prevent.

2. **The manifest guidance field mirrors `SweepManifest.injectedGuidance`
   exactly.** S1's `Run`/manifest record gains an
   `injected_guidance: Vec<StrategyRef>` field, where `StrategyRef =
   {strategy_id, title, content}` is a full SNAPSHOT, not a live pointer —
   so even if the strategy is later demoted or its content edited, the
   manifest keeps what was ACTUALLY shown to the agent at dispatch time.
   Written once at dispatch (`canon retrieve` → dispatch hook → manifest
   write); read via `manifest_guidance_for_replay(manifest) ->
   Vec<StrategyRef>`, which returns the field unchanged — the named
   replay/live-retrieval boundary the donor monorepo's own module doc calls out
   ("giving the replay path its own name keeps that call-site distinction
   from being silently blurred into scattered reads"). *Alternative
   rejected:* storing only `strategy_id`s and re-resolving content at
   replay time — this is EXACTLY the bug S8's acceptance criterion exists
   to prevent ("replay with a changed store produces identical run
   inputs"); re-resolving IDs against a changed store changes the content.

3. **Fail-soft is enforced at the call boundary, not hoped for by
   convention.** `canon retrieve`'s public signature returns
   `Vec<StrategyRef>` — never a `Result`/error type — at the CLI/library
   boundary; internally, errors are caught and logged, never surfaced as a
   typed failure the caller must handle, mirroring
   `evaluatePreSweepLookup`'s no-error-channel shape and its explicit
   rationale. *Alternative rejected:* `Result<Vec<StrategyRef>,
   RetrieveError>` with callers required to `.unwrap_or_default()` —
   leaves a footgun where a future caller could propagate the error and
   accidentally make retrieval blocking, exactly the failure mode
   "advisory and fail-soft" exists to forbid at the type level.

4. **The pre-dispatch hook reuses S5's wiring shape.** The generic
   pre-dispatch hook script + the donor CLI's wiring for the donor monorepo follows the SAME
   `.claude/settings.json` / `.codex/hooks.json` entry shape S5
   establishes (`{matcher, hooks: [{type: "command", command: "canon
   retrieve ...", timeout}]}`), invoked at a PreToolUse-equivalent point
   analogous to the donor monorepo's existing `pre-edit-pattern-lookup.ts` hook,
   generalized to any `task`-shaped dispatch. *Alternative rejected:* a
   bespoke dispatch-hook mechanism independent of S5's wiring — two
   different wiring conventions in the same settings.json is exactly the
   drift risk S5's design.md risk section calls out; S8 reuses S5's
   `canon gate install-hooks`-equivalent installer rather than inventing
   a second one.

5. **Retrieval scoping filters MAY be CEL via S13, once S13 lands.** Beyond
   the `role`/`regime_key` scoping decision 1 already fixes, a future
   `canon retrieve` refinement (e.g. an area/tag match predicate narrower
   than the regime key) is an S13 `canon-policy` CEL predicate over
   `StrategyItem`/`Trajectory` facts (design doc §5 S13: "S8 retrieval
   scoping filters — regime/area match predicates"), not a bespoke filter
   DSL invented in this crate. This change does not itself add CEL
   evaluation — S13 owns `canon-policy`; this is a forward-compatibility
   pointer only.

## Risks / Trade-offs

- [Risk] Snapshotting strategy content into the manifest (decision 2)
  bloats manifest size for high-guidance-count runs → [Mitigation] cap
  `--k` (default matches S6's search default, e.g. 5) and truncate
  `content` length in DISPLAY/report surfaces only — the snapshot used
  for the byte-identical replay input stays complete.
- [Risk] A demoted strategy already embedded in an OLD manifest (before
  decision 2's exclusion applied at a later write) still replays with the
  demoted content → [Mitigation] this is CORRECT per the reproducibility
  guarantee — a replay reproduces what happened, not what should have
  happened; `canon report` (S9) flags "this historical run used
  since-demoted guidance," it never rewrites the manifest (§7
  append-only).
- [Risk] Fail-soft-by-type (decision 3) makes a genuine retrieval outage
  invisible to the operator → [Mitigation] internal errors are still
  logged (structured log line) even though never surfaced as a typed
  failure — observability without blocking.
- [Risk] The pre-dispatch hook auto-derives `--regime` in shell, so a
  local segment-normalizer that drifted from `canon_model::ids::
  regime_key` would write a strategy under `dev/my_repo/...` yet query
  `dev/my-repo/...` — `retrieve_guidance` fail-softs to empty and the
  guidance is SILENTLY missed (whole-branch-review finding) →
  [Mitigation] the hook does NOT re-derive the key: it assembles the
  repo/area segments through `canon regime-key`, a thin CLI over the
  SAME `regime_key` serializer S4/S6/S14's write path calls, so there
  is exactly one canonicalizer at both ends (decision 1's "no second
  key derivation," restated at the shell boundary). Only the kebab
  `RoleId` is shell-slugged, and a kebab string is a fixed point of
  that canonicalizer, so `--role` still equals the assembled key's
  leading segment. Regression-locked by `pre_dispatch_hook_regime_
  preserves_underscores_matching_the_rust_write_path`.

## Migration Plan

- Step 1: `canon retrieve` ships standalone against S6's store — works
  with zero manifest integration (a caller can just print retrieved
  guidance).
- Step 2: the manifest field + dispatch-hook wiring ship together — the
  hook is what actually populates `injected_guidance`.
- Step 3: the donor monorepo's wiring is additive alongside `pre-edit-pattern-lookup.ts`
  (not a replacement) — the donor monorepo's existing hook keeps working until a
  follow-up donor-side cutover, mirroring S5's/S6's non-destructive
  migration posture.
- Rollback: removing the pre-dispatch hook entry reverts to no injected
  guidance; existing manifests with `injected_guidance` already recorded
  remain replayable — the field is additive, not required by older
  manifest schema versions.

## Open Questions

- Whether `--k` should default per-role (mirroring S7's per-role reward
  defaults) or stay a single global default — deferred to implementation
  time once S7's per-role tuning data exists.
- The exact PreToolUse-equivalent dispatch point for non-Claude/Codex
  consumer repos — decided per S0's runtime-target scope (Claude Code +
  Codex only, decision 11), so this is really "which lifecycle event,"
  pinned when the generic script is authored.
