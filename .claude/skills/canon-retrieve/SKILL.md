---
name: canon-retrieve
description: How to run canon retrieve --role/--regime/--k/--repo/--json — the role+regime-scoped strategy-guidance query over promoted strategy memory, wire the companion pre-dispatch hook script via canon gate install-hooks, and understand the replay-vs-live boundary for a run manifest's injected_guidance. Use when a dispatched agent needs role-scoped strategy memory surfaced before it starts, wiring a repo's PreToolUse hook seam, or reading/replaying a run manifest that carries injected_guidance.
---

# canon-retrieve

`canon retrieve` surfaces a role's promoted strategy memory (see
`canon-learn`) at task-dispatch time. It is a fail-soft, advisory-only
read: it NEVER blocks a dispatch and NEVER gates reproducibility.

## `canon retrieve --role <r> --regime <k> [--k <n>] [--repo <dir>] [--json]`

Prints the top-`k` non-demoted strategies stored for `regime_key = <k>`
(strategies + guardrails; a guardrail is just a strategy distilled from a
failure-polarity verdict), newest-distilled first, capped at `--k`
(default 5).

```bash
canon retrieve --role dev --regime dev/canon/join-spine/9c93d024b1a2
canon retrieve --role dev --regime dev/canon/join-spine/9c93d024b1a2 --k 3 --json
```

- `--regime` is the FULL `regime_key` string
  (`<role>/<repo>/<area>/<hash>`) — the same serialization the write side
  produces.
- Assembling `--regime` in a script or hook? Use `canon regime-key
  --role <r> --repo <r> --area <a> --hash <h>`: it runs canon's ONE
  canonical `regime_key` serializer and prints the validated key, so a
  caller routes through the exact normalizer the write path uses instead
  of a drifting shell slug. Exits `2` with empty stdout on a malformed
  result (empty segment / bad `<hash>`) — a clean fail-soft branch for
  `|| exit 0` callers.
- `--role` MUST equal `--regime`'s leading segment — a mismatch is a
  clean usage error, exit `2` (mirroring `canon gate check`'s
  0-clean/1-red/2-usage convention), never a panic or a silently-ignored
  mismatch.
- `--repo` resolves through the same nearest-`canon.yaml`-ancestor walk
  `canon context`/`canon fmt`/`canon gate` use. The resolved
  `canon.yaml` `learn:` section names the store root (`canon/learn` by
  default); a repo with no `canon.yaml` still works, defaulting cleanly.

**FAIL-SOFT, always exit `0`** once `--role`/`--regime` agree: a store
outage, an empty/nonexistent store, or a malformed on-disk row all
degrade to an empty guidance list — never a nonzero exit or an error a
caller must handle.

## The pre-dispatch hook (`canon/skills/canon-retrieve/pre-dispatch.sh`)

A generic, fail-soft, advisory-only shell script that fires at a
PreToolUse-equivalent point for a `task`-shaped dispatch (Claude Code's
`Task` tool / Codex's equivalent). It reads the tool's `subagent_type`
as `--role`, assembles `--regime` by routing its repo/area segments
through `canon regime-key` (so it never re-derives the join key with a
drifting shell slug), calls `canon retrieve --json`, and emits the
result as a `PreToolUse` `hookSpecificOutput.additionalContext` JSON
envelope on stdout — Claude Code's convention for surfacing advisory
context without a permission decision. It NEVER blocks: a missing
`canon`/`jq`, a non-`Task` tool call, or empty guidance are all silent
no-ops, exit `0`.

Segment sources and override env vars
(`CANON_RETRIEVE_ROLE`/`CANON_RETRIEVE_AREA`/`CANON_RETRIEVE_HASH`/
`CANON_RETRIEVE_REGIME`) are documented in the script's own header.

### Wiring it

Place the script wherever your repo keeps materialized hook scripts
(e.g. `.claude/hooks/canon-retrieve-pre-dispatch.sh`), then register the
hook-seam entry with `canon gate install-hooks`:

```bash
canon gate install-hooks --repo . --event PreToolUse \
  --matcher Task --command "sh .claude/hooks/canon-retrieve-pre-dispatch.sh" \
  --timeout 15
```

This merges the standard `{matcher, hooks: [{type: "command", command,
timeout}]}` entry into BOTH `<repo>/.claude/settings.json` and
`<repo>/.codex/hooks.json`, additive and idempotent — see the
`canon-gate` skill for `install-hooks`'s full contract.

## Replay vs. live retrieval

A run manifest records `injected_guidance` — a full SNAPSHOT (each entry
`{strategy_id, title, content}`) of what `canon retrieve` returned at
dispatch time, visible in `--json` output. It is never a live pointer, so
a later edit or demotion of the source strategy can never retroactively
change what a manifest already recorded.

- **New run:** dispatch calls `canon retrieve` (live), then records the
  result into the run manifest's `injected_guidance`.
- **Replay:** reuse the manifest's recorded `injected_guidance` UNCHANGED
  — never a fresh `canon retrieve` call. A replay MUST read the stored
  guidance, so a live store change after the original run (a strategy
  added, edited, demoted, or removed) can never perturb the replay.

`canon retrieve` and the pre-dispatch script work standalone today — a
caller can print retrieved guidance with zero manifest integration.

## What this skill does NOT cover

- Strategy distillation/promotion/demotion — see the `canon-learn`
  skill.
- `canon gate install-hooks`'s full contract — see the
  `canon-gate` skill; this skill documents only the one entry it
  installs for `canon retrieve`.
