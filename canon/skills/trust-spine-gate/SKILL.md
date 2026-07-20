---
name: trust-spine-gate
description: How to run canon gate check/task/promote/install-hooks/selftest — the trust-spine gate over an EvidenceRecord-shaped corpus (coverage, verdict-ledger, staleness, the trust ladder), evidence-gated openspec checkbox flips, staging→promote, hook-seam installation, and reading a FAILURE_CLASSES violation. Use when a task claims done, before flipping an openspec checkbox, when wiring a repo's pre-commit/hook gate, or when a canon gate command exits non-zero and you need to know what the failure class means.
---

# trust-spine-gate

`canon-gate` (S5, `crates/canon-gate`) is canon's trust spine: a
two-layer covered-vs-green evidence gate over any `EvidenceRecord`-shaped
corpus, generalizing the donor parity harness's `tools/parity.py`. It answers two
DIFFERENT questions per artifact — "does required evidence exist"
(coverage, D3a) and "did the evidence pass, by whom, how stale"
(verdict-ledger, D3b) — never collapsing them into one "done" boolean.
`canon-cli` (`crates/canon-cli/src/gate.rs`) is the CLI surface over it.

## The eight failure classes

Every violation `canon gate` reports carries one of these stable, grep-
able strings (`canon_gate::FAILURE_CLASSES`) — never renamed without a
coordinated fixtures+hooks migration:

| Class | Meaning |
|---|---|
| `uncovered-cell` | A policy-required evidence cell (role × artifact) has no matching record at all. Coverage is "a test exists", never "a test passed" — a `Divergent` verdict still satisfies coverage. |
| `unreviewed-promotion` | An artifact's `reviewed` lifecycle tag has no matching ledger review-record. |
| `trust-below-required` | The artifact's achieved trust level is below `policy.yaml`'s `trust_required` for its class — RELEASE-scoped only (`canon gate check --release`); never fires on an ordinary run. |
| `stale-evidence` | A passing record degraded to stale: its declared surface changed since its `evidence_sha`, or HEAD moved past `staleness.max_commits_behind`. Only ever degrades an ALREADY-green record. |
| `malformed-evidence` | A candidate record doesn't parse, is misfiled (wrong Hive path), or carries a present-but-unparseable interim companion tag (`trust_ladder`/`evidence_note`). Malformed evidence is no evidence. |
| `flagged` | The human-only `flagged` overlay is set — never green regardless of any passing evidence, even a `ratified` artifact. |
| `unevidenced-flip` | `canon gate task <task_id>` was asked to flip a checkbox with no matching, non-`Divergent` `EvidenceRecord`. |
| `fabricated-evidence` | The evidence note attached to a flip contains a blocklisted marker (`"would pass"`, `"TBD"`, `"n/a"`) or a bare `verified` claim with no attached command result. |

## `canon gate check [--repo <dir>] [--release]`

The DISPATCHER: assembles coverage + ledger + staleness + the always-on
trust-ladder check (`canon_gate::check_set`) over the resolved repo's
evidence corpus and runs them, printing every violation grouped by
failure class. `--release` additionally engages the release-scoped
`trust-below-required` check — the trust-ladder check is NEVER dropped
when `--release` is passed (this is a structural property of the
dispatcher, not caller discipline: a release profile that ran
`ReleaseTrustCheck` alone could miss `unreviewed-promotion`/`flagged` on
a record `ReleaseTrustCheck`'s own release-scoped path never inspects).

```bash
canon gate check --repo .            # ordinary evaluation
canon gate check --repo . --release  # + trust-below-required
```

Exit `0` clean, `1` gate-red (any violation), `2` usage/load failure
(unreadable ledger, corrupt `canon.yaml`). `--repo` (or its omission)
resolves through the same nearest-ancestor `canon.yaml` walk `canon
context`/`canon fmt` use — run it from any subdirectory of a repo and it
still reads the repo ROOT's `canon/policy.yaml` and `canon/ledger`.

## `canon gate task <task_id> [--repo <dir>]`

The evidence-gated openspec checkbox flip (`gated-task-completion`).
Resolves `<task_id>` (`<change_id>#<n>`) to
`<repo>/openspec/changes/<change_id>/tasks.md`, requires a matching,
non-`Divergent` `EvidenceRecord` in the ledger, and flips `- [ ]` →
`- [x] ` with an appended evidence note ONLY on a clean check. Every
other path — missing evidence, a `Divergent` verdict, a fabricated
evidence note — leaves the row byte-unchanged and exits `1` with the
blocking violation on stderr. An already-`[x]` row is an idempotent
no-op (exit `0`, no violation). An unknown `task_id` is reported, never
silently ignored (exit `1`).

```bash
canon gate task s5-trust-spine-gate#5.2 --repo .
```

This is canon's OWN format authority for the checkbox grammar — never
call a donor CLI's `flipTaskDone`/`task-flip.ts` from a canon-authored change;
always go through `canon gate task`.

## `canon gate promote [--repo <dir>] [--dry-run]`

Staging → committed (O13): every well-formed record under
`canon/ledger/_staging/` is re-validated with the SAME checks the gate
applies, assigned a monotonic per-(role, surface) `run_seq` (gap-free
within one invocation, continuing from the committed tier's own max), and
moved into the committed ledger. A malformed or unpartitionable candidate
is refused — exit `1`, no `run_seq` consumed, the staging file left in
place for a reviewer to fix. `--dry-run` prints the plan (target path +
assigned `run_seq` per candidate) without writing or deleting anything.

```bash
canon gate promote --repo .              # land every staged candidate
canon gate promote --repo . --dry-run    # preview only
```

## `canon gate install-hooks [--repo] [--event] [--matcher] [--command] [--timeout]`

Idempotent, diff-only hook-seam installation (design D8): merges one
`{matcher?, hooks: [{type: "command", command, timeout}]}` entry into
BOTH `<repo>/.claude/settings.json` and `<repo>/.codex/hooks.json` —
additive only, never touching an existing (e.g. a donor CLI's own
`hook run <kind>`) entry in the same matcher group. Running it twice in a
row with no manual edits between reports "no diff" and writes nothing.
When neither file already carries ANY `canon gate`-invoking command, it
ALSO emits the generic `<repo>/scripts/canon-gate-pre-commit.sh`
(advisory by default — set `CANON_GATE_ADVISORY=0` to make it block the
commit on a failing gate).

```bash
canon gate install-hooks --repo .
# non-default wiring:
canon gate install-hooks --repo . --event PreToolUse --matcher Edit --command "canon gate check" --timeout 30
```

Prefer this over hand-editing `settings.json`/`hooks.json` — it is the
mechanism that keeps two CLIs (canon + a donor CLI) from clobbering each
other's hook-seam entries over time.

## `canon gate selftest`

Runs the shipped fixture corpus (`crates/canon-gate/fixtures/<class>/`)
— one fixture per `FAILURE_CLASSES` entry, each a deliberately broken
corpus proving that class fires and ONLY that class (exact-set-match
against the fixture's own `expected_failures.txt`, both under-detection
and over-triggering fail the run). Takes no `--repo`; self-contained.
Run this after touching `crates/canon-gate/src/**` to prove the gate
itself is still trustworthy (DO-330 tool-qualification discipline, D17)
before trusting any OTHER `canon gate check` run's green.

```bash
canon gate selftest
```

## Reading a violation

Every printed line is `<failure-class> <subject> — <detail>` (`canon
gate check`'s own grouped-by-class output; `canon gate task`/`canon gate
promote` print the same `Violation::line()` shape on stderr for a
blocked action). `<subject>` is the artifact's own join-spine identity
(`task_id` preferred, then `scenario_id`, then `run_id`) — grep the
ledger or `tasks.md` for it directly, never guess.

## What this skill does NOT cover

- Authoring `policy.yaml`'s `risk_routing`/`trust_required`/`staleness`
  fields, including CEL predicates — see the `canon-policy` skill.
- The `canon-gate` library's own internals (`GateCtx`/`GateContext`/
  `GateCheck`, the trust-ladder classifier, the flag-clear ratchet) —
  read `crates/canon-gate/src/**`'s own module docs; this skill is the
  CLI-usage surface, not an API reference.
- Wiring a specific consumer repo's REAL `.claude/settings.json`
  (`canon gate install-hooks` ships the mechanism; actually running it
  against e.g. a consumer repo's own settings.json is that repo's own follow-up
  change, per design decision 7/8's migration-target boundary — this
  change never edits a consumer repo's hook config for you).
