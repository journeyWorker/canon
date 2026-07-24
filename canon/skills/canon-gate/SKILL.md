---
name: canon-gate
description: How to run canon gate check/task/promote/install-hooks/selftest — the trust-spine evidence gate over a repo's coverage, verdict ledger, staleness, and trust ladder, plus evidence-gated task checkbox flips, staging→promote, hook installation, and reading a failure-class violation. Use when a task claims done, before flipping a task checkbox, when wiring a repo's pre-commit/hook gate, or when a canon gate command exits non-zero and you need to know what the failure class means.
---

# canon-gate

`canon gate` is canon's trust spine: a two-layer evidence gate over a
repo's artifact corpus. It answers two DIFFERENT questions per artifact —
"does required evidence exist" (coverage) and "did the evidence pass, by
whom, how stale" (verdict-ledger) — never collapsing them into one
"done" boolean.

## The trust ladder

Each artifact climbs a trust ladder as evidence accrues: an unreviewed
record is weaker than a reviewed one, which is weaker than a ratified
one. `policy.yaml` sets a `trust_required` level per artifact class; the
release-scoped check (below) enforces it. A human-only `flagged` overlay
overrides the ladder — a flagged artifact is never green regardless of
evidence. (Author `policy.yaml`'s trust/staleness fields via `canon-policy`.)

## The eight failure classes

Every violation carries one of these stable, grep-able strings:

| Class | Meaning |
|---|---|
| `uncovered-cell` | A policy-required evidence cell (role × artifact) has no matching record. Coverage means "a test exists", not "a test passed" — even a `Divergent` verdict satisfies coverage. |
| `unreviewed-promotion` | An artifact tagged `reviewed` has no matching ledger review record. |
| `trust-below-required` | Achieved trust level is below `policy.yaml`'s `trust_required` for its class — RELEASE-scoped only (`canon gate check --release`); never fires on an ordinary run. |
| `stale-evidence` | A passing record degraded to stale: its declared surface changed since its `evidence_sha`, or HEAD moved past `staleness.max_commits_behind`. Only degrades an already-green record. |
| `malformed-evidence` | A candidate record doesn't parse, is misfiled, or carries an unparseable interim tag. Malformed evidence is no evidence. |
| `flagged` | The human-only `flagged` overlay is set — never green regardless of passing evidence. |
| `unevidenced-flip` | `canon gate task <task_id>` was asked to flip a checkbox with no matching, non-`Divergent` evidence record. |
| `fabricated-evidence` | An evidence note contains a blocklisted marker (`"would pass"`, `"TBD"`, `"n/a"`) or a bare `verified` claim with no attached command result. |

## `canon gate check [--repo <dir>] [--release]`

Assembles coverage + ledger + staleness + the always-on trust-ladder
check over the resolved repo's corpus and runs them, printing every
violation grouped by failure class. `--release` additionally engages the
release-scoped `trust-below-required` check; the trust-ladder check is
never dropped when `--release` is passed.

```bash
canon gate check --repo .            # ordinary evaluation
canon gate check --repo . --release  # + trust-below-required
```

Exit `0` clean, `1` gate-red (any violation), `2` usage/load failure
(unreadable ledger, corrupt `canon.yaml`). `--repo` (or its omission)
resolves through the nearest-ancestor `canon.yaml` walk, so it reads the
repo ROOT's `.canon/policy.yaml` and `.canon/ledger` from any subdirectory.

## `canon gate task <task_id> [--repo <dir>]`

The evidence-gated task checkbox flip. Resolves `<task_id>`
(`<change_id>#<n>`) to `openspec/changes/<change_id>/tasks.md`, requires
a matching non-`Divergent` evidence record, and flips `- [ ]` → `- [x]`
with an appended evidence note ONLY on a clean check. Every other path —
missing evidence, a `Divergent` verdict, a fabricated note — leaves the
row byte-unchanged and exits `1` with the blocking violation on stderr.
An already-`[x]` row is an idempotent no-op (exit `0`). An unknown
`task_id` is reported (exit `1`).

```bash
canon gate task my-change#5.2 --repo .
```

This is canon's own authority for the checkbox grammar — always flip
through `canon gate task`, never by hand.

## `canon gate promote [--repo <dir>] [--dry-run]`

Staging → committed: every well-formed record under
`.canon/ledger/_staging/` is re-validated with the SAME checks the gate
applies, assigned a monotonic per-(role, surface) `run_seq` (gap-free
within one invocation, continuing from the committed max), and moved into
the committed ledger. A malformed or unpartitionable candidate is refused
— exit `1`, no `run_seq` consumed, the file left in place. `--dry-run`
prints the plan (target path + assigned `run_seq` per candidate) without
writing or deleting.

```bash
canon gate promote --repo .            # land every staged candidate
canon gate promote --repo . --dry-run  # preview only
```

## `canon gate install-hooks [--repo] [--event] [--matcher] [--command] [--timeout]`

Idempotent, diff-only hook-seam installation: merges one
`{matcher?, hooks: [{type: "command", command, timeout}]}` entry into
BOTH `<repo>/.claude/settings.json` and `<repo>/.codex/hooks.json` —
additive only, never touching an existing entry in the same matcher
group. Running it twice with no manual edits between reports "no diff"
and writes nothing. When neither file already carries a `canon
gate`-invoking command, it ALSO emits
`<repo>/.canon/scripts/canon-gate-pre-commit.sh` (advisory by default — set
`CANON_GATE_ADVISORY=0` to make it block the commit on a failing gate).

```bash
canon gate install-hooks --repo .
# non-default wiring:
canon gate install-hooks --repo . --event PreToolUse --matcher Edit --command "canon gate check" --timeout 30
```

Prefer this over hand-editing `settings.json`/`hooks.json`.

## `canon gate selftest`

Runs the shipped fixture corpus — one fixture per failure class, each a
deliberately broken corpus proving that class fires and ONLY that class
(both under-detection and over-triggering fail the run). Takes no
`--repo`; self-contained. Run it before trusting any other `canon gate
check` run's green.

```bash
canon gate selftest
```

## Reading a violation

Every printed line is `<failure-class> <subject> — <detail>`. `<subject>`
is the artifact's own join identity (`task_id` preferred, then
`scenario_id`, then `run_id`) — grep the ledger or `tasks.md` for it
directly, never guess.
