# canon-retrieve

> How to run canon retrieve --role/--regime/--k/--repo/--json — the S8 (retrieve-before-task) role+regime-scoped strategy-guidance query over canon-learn's StrategyStore, wiring the generic pre-dispatch hook script (canon/skills/canon-retrieve/pre-dispatch.sh) via canon gate install-hooks, and the replay-vs-live-retrieval boundary (Run.injected_guidance / manifest_guidance_for_replay). Use when a dispatched agent needs role-scoped strategy memory surfaced before it starts, when wiring a repo's PreToolUse hook seam for task dispatch, or when reading/replaying a Run manifest that carries injected_guidance.

# canon-retrieve

S8 (`s8-retrieve-before-task`) generalizes a donor tuning project's
`evaluatePreSweepLookup`/`manifestGuidanceForReplay` pair and
a donor harness's fail-soft `pre-edit-pattern-lookup.ts` PreToolUse
hook into one role-agnostic retrieval command any dispatch surface can
call. `canon-learn` (S6/S7, `crates/canon-learn`) owns the strategy
store and the fail-soft library core
(`canon_learn::guidance::retrieve_guidance`/
`manifest_guidance_for_replay`); `canon-cli`
(`crates/canon-cli/src/retrieve.rs`) is the CLI surface over it.

## `canon retrieve --role <r> --regime <k> [--k <n>] [--repo <dir>] [--json]`

Prints the top-`k` non-demoted `StrategyItem`s stored for `regime_key
= <k>` (strategies + guardrails — a guardrail is just a `StrategyItem`
distilled from a failure-polarity verdict, never a separate type),
newest-distilled first, capped at `--k` (default 5).

```bash
canon retrieve --role dev --regime dev/canon/join-spine/9c93d024b1a2
canon retrieve --role dev --regime dev/canon/join-spine/9c93d024b1a2 --k 3 --json
```

- `--regime` is the FULL `regime_key` string
  (`<role>/<repo>/<area>/<hash>`) — the SAME canonical serialization
  S6's write side produces (`canon_model::ids::regime_key`); never a
  second key derivation.
- Assembling `--regime` in a script or hook? Use `canon regime-key
  --role <r> --repo <r> --area <a> --hash <h>`: it runs the ONE
  canonical `regime_key` serializer (`canon_model::ids::regime_key`)
  and prints the validated key, so a caller routes through the EXACT
  normalizer the S4/S6/S14 write path uses instead of a second
  derivation that could drift (e.g. a shell slug mapping a written
  `my_repo` to a queried `my-repo` and silently missing the
  namespace). Exits `2` with nothing on stdout on a malformed result
  (empty segment / bad `<hash>`) — a clean fail-soft branch for
  `|| exit 0` callers.
- `--role` MUST equal `--regime`'s own leading segment
  (`regime_key.role()`) — `regime_key` is the actual scoping key,
  `--role` is a caller-contract check on it. A mismatch is a clean
  usage error, exit `2` (mirrors `canon gate check`'s own
  0-clean/1-red/2-usage convention) — never a panic, and never a
  silently-ignored mismatch.
- `--repo` resolves through the same nearest-`canon.yaml`-ancestor
  walk `canon context`/`canon fmt`/`canon gate` already use (design
  D7). The resolved repo's `canon.yaml` `learn:` section
  (`canon_learn::LearnConfig`) names the operator-local store root
  (`canon/learn` by default) — a repo with no `canon.yaml` at all still
  works, defaulting cleanly.

**FAIL-SOFT, always exit `0`** once `--role`/`--regime` agree: a store
outage, an empty/nonexistent store, or a malformed on-disk row all
degrade to an empty guidance list — logged internally by
`retrieve_guidance`, never surfaced as a nonzero exit or an error a
caller must handle. `canon retrieve` NEVER blocks and NEVER gates
reproducibility.

## The pre-dispatch hook (`canon/skills/canon-retrieve/pre-dispatch.sh`)

A generic, fail-soft, advisory-only shell script that fires at a
PreToolUse-equivalent point for a `task`-shaped dispatch (Claude
Code's `Task` tool / Codex's equivalent), mirroring a donor harness's
fail-soft PreToolUse-hook contract: it reads the tool's
`subagent_type` as `--role`, and assembles `--regime` by routing its
repo/area segments through `canon regime-key` — canon's OWN
`regime_key` serializer (`canon_model::ids::regime_key`), the identical
normalizer the S4/S6/S14 write path uses, so the hook NEVER re-derives
the join key with a second, drifting shell slug (see the script's own
header comment for the segment sources + override env vars —
`CANON_RETRIEVE_ROLE`/`CANON_RETRIEVE_AREA`/`CANON_RETRIEVE_HASH`/
`CANON_RETRIEVE_REGIME`), calls `canon retrieve --json`, and emits the
result as a `PreToolUse` `hookSpecificOutput.additionalContext` JSON
envelope on stdout — Claude Code's own documented convention for
surfacing advisory context without a permission decision (the same
envelope shape the donor CLI's own hooks, e.g. `handoff-session-start`,
already emit). It NEVER blocks the dispatch: a missing `canon`/`jq`, a
non-`Task` tool call, or empty guidance are all silent no-ops, exit
`0`.

### Wiring it (reuses S5's installer — no second convention)

Place the script wherever your repo keeps materialized hook scripts
(e.g. `.claude/hooks/canon-retrieve-pre-dispatch.sh`, mirroring the
manual-placement convention for materialized hook scripts — the same
manual-placement discipline `canon-gate`'s own `PRE_COMMIT_SCRIPT`
documents: "install it directly ..., or invoke it from a lefthook/
husky `run:` line"), then wire the hook-seam entry with the ALREADY-
GENERIC `canon gate install-hooks` (S5, `s5-trust-spine-gate`) — this
skill adds NO second hook-installer:

```bash
canon gate install-hooks --repo . --event PreToolUse \
  --matcher Task --command "sh .claude/hooks/canon-retrieve-pre-dispatch.sh" \
  --timeout 15
```

This merges the standard `{matcher, hooks: [{type: "command", command,
timeout}]}` entry into BOTH `<repo>/.claude/settings.json` and
`<repo>/.codex/hooks.json`, additive and idempotent — see the
`trust-spine-gate` skill for `install-hooks`'s own full contract.

## The replay-vs-live-retrieval boundary

`Run.injected_guidance: Vec<StrategyRef>` (`canon_model::records::Run`)
is a full SNAPSHOT of what `canon retrieve` returned at dispatch time
— `StrategyRef = {strategy_id, title, content}` — never a live
pointer, so a later edit or demotion of the source strategy can never
retroactively change what a manifest already recorded.

- **New run:** dispatch calls `canon retrieve` (live), then records
  the result into `Run::injected_guidance` (via
  `Run::with_injected_guidance`) at the run-manifest write.
- **Replay:** call `canon_learn::guidance::manifest_guidance_for_replay(&run)`
  — returns `run.injected_guidance` UNCHANGED, NEVER a fresh `canon
  retrieve` call (the function takes `&Run` only, no store handle —
  there is no live lookup it could even perform). A replay of a run's
  manifest MUST go through this function, never
  `retrieve_guidance` directly, so a live store change after the
  original run (a strategy added, edited, demoted, or removed) can
  never perturb the replay.

**Deferred (tracked, not silently invented):** the dispatch-time WRITE
into `Run::injected_guidance` (`s8-retrieve-before-task` task 2.3)
depends on a live run-manifest write seam this codebase does not yet
have — `canon-ingest`'s `Run` construction
(`crates/canon-ingest/src/normalize.rs`) is a POST-HOC reconstruction
from parsed session transcripts, not a live "a run is starting now,
here is its manifest" write path a dispatch hook could call into. This
skill (and `canon retrieve` + the pre-dispatch script) work standalone
today (design.md Migration Plan Step 1: "works with zero manifest
integration — a caller can just print retrieved guidance"); wiring the
actual `injected_guidance` populate is Migration Step 2, blocked on
that write seam landing first.

## What this skill does NOT cover

- `canon-learn`'s own strategy distillation/promotion/demotion — see
  the `canon-reward` skill.
- `canon gate install-hooks`'s own full contract (merge rules,
  idempotency, the generic pre-commit script) — see the
  `trust-spine-gate` skill; this skill only documents the ONE entry it
  installs for `canon retrieve`'s own hook.
- Materializing `pre-dispatch.sh` itself into a consumer repo's
  `.claude/`/`.codex/` tree automatically (the way `canon gate
  install-hooks` auto-writes `canon-gate-pre-commit.sh`) — that is a
  natural follow-up (extending either `canon gate install-hooks`'s
  auto-write condition or `canon skills install`'s materializer to
  copy companion files, not just `SKILL.md`), not yet built; for now
  the script is placed manually, same as `PRE_COMMIT_SCRIPT`'s own
  documented manual-install path.
