# canon-session-ingest

> How to run canon ingest sessions [--watch] [--full] [--all-workspaces] — the session-ingest pipeline, scoped by default to the current project, that scans every registered agent-CLI transcript source (omp/pi, Hermes, Claude Code, Codex), normalizes each into Session/Run/Event records (including user_directive events from user turns), and persists them incrementally via a per-file watermark cursor. Use when ingesting agent session/cost/directive telemetry, running a --watch poll loop, widening the scan machine-wide, resetting the watermark, or reading the per-adapter run summary.

# canon-session-ingest

`canon ingest sessions` turns every agent CLI's on-disk transcript store
into `Session`/`Run`/`Event` records on canon's join spine, scanning four
adapters, normalizing, and persisting through the same tier resolution
`canon query` / `canon tier age` use. The default corpus is scoped to the
current PROJECT; the watermark gate is per-file; user directives are
captured.

**Quick start needs zero docker/services.** `canon init` scaffolds `hot`
as a local sqlite file (`.canon/hot.db`) with `session`/`run`/`event`
already routed to it — a fresh repo's FIRST `canon ingest sessions`
persists real records with no `docker compose` stack, no `CANON_PG_DSN`,
no live database. Swap `hot` to postgres (the commented stanza `canon
init` ships beside the sqlite one) once team-scale multi-agent
concurrency outgrows sqlite's single-writer mode.

## `canon ingest sessions [--watch] [--interval-secs N] [--home <dir>] [--canon-yaml <path>] [--full] [--all-workspaces]`

```bash
canon ingest sessions                   # one pass over this project's sources
canon ingest sessions --watch           # poll every 30s (see --interval-secs)
canon ingest sessions --full            # ignore the watermark; re-parse every in-scope file
canon ingest sessions --all-workspaces  # machine-wide scan
canon ingest sessions --home /custom    # override the scan-root home ($HOME)
```

## The four adapters

Registered in order: **`omp`** (omp/pi sessions), **`hermes`**,
**`claude`** (`~/.claude/projects/**/*.jsonl`), **`codex`**
(`${CODEX_HOME:-~/.codex}/sessions/` unioned with `archived_sessions/`).
Each resolves its default root and honors an env override
(`CANON_INGEST_OMP_SESSIONS_DIR`, `CANON_INGEST_CLAUDE_SESSIONS_DIR`,
`CODEX_HOME`, `HERMES_HOME`). An absent source root is a zero-record,
non-fatal skip.

### Per-source root overrides (`canon.yaml`)

```yaml
ingest:
  sources:
    omp:
      roots: [/data/omp-sessions]   # relative paths resolve against the canon.yaml dir
    codex:
      roots: []                      # explicit empty = scan ZERO (not the default)
```

A source with a **present** `roots` scans exactly those (explicit
`roots: []` scans nothing); a source whose key or `roots` is **absent**
keeps its env-override + default resolution. Unlike artifacts (where
unconfigured = no scan), sessions default to their home roots when
unconfigured. A PRESENT but broken `ingest:` section fails **loud** (a
typo like `root:` for `roots:`, an unknown source id, or a non-YAML
canon.yaml) rather than silently scanning the wrong corpus; a
missing/unreadable canon.yaml stays a soft no-config.

## Project scope — the default corpus

`canon ingest sessions` defaults to THIS PROJECT's sessions, never the
whole machine: the repo's main `git worktree` root plus every linked
worktree (`git worktree list --porcelain`). Outside a git repo, the scope
fails soft to the repo root alone.

- **omp/pi and Claude Code are cwd-partitioned on disk** — one
  subdirectory per project. Out-of-scope subdirectories are pruned at
  enumerate time — never read or counted.
- **Codex and Hermes are not partitioned** — rows are filtered
  post-parse by workspace membership; a row with no captured workspace is
  kept, fail-soft.
- **`--all-workspaces`** restores the machine-wide scan. Scope is still
  resolved (so `project_key` stamping stays correct); only the
  pruning/filtering are disabled.
- Every ingested session whose workspace resolves into the active project
  gets `project_key` stamped to the MAIN worktree's key, so `canon query
  --kind session` aggregates a whole project's sessions regardless of
  which worktree recorded them.
- The run summary's first line always names the active scope:
  `scope: project /repo/root (2 roots)` or `scope: all workspaces`.

## The watermark — per-file, incremental `--watch`

Each pass content-digests every present (post-pruning) file and diffs it
against its source's persisted cursor (under
`<repo>/.canon/ingest/cursors/`, gitignored), written after a durable
pass. A file whose digest matches its cursor is SKIPPED — never parsed; a
new or changed file is (re-)parsed. One session derives from exactly one
file, so a single growing transcript re-parses alone instead of dragging
the whole source back through parse/persist.

- The gate is sound: a file is skipped only when its digest matches the
  cursor, so a new, changed, copied, or restored file is always
  re-scanned.
- Correctness never depends on the cursor: a missing/corrupt cursor
  treats every present file as new, and the digest-idempotent write path
  keeps any rescan from double-writing.

**`--full`** ignores the cursors and re-parses every present in-scope
file (a full rescan / cursor reset) — safe because a byte-identical
resubmission is a no-op; cursors re-advance afterward.

## User directives

Every adapter (except Hermes, whose format carries no user-turn text)
emits one event per USER-role message: `label: "user_directive"`,
`detail: { text, workspace_key, workspace_label }`, the FULL verbatim
text (no truncation). These interleave with `token_usage` events in one
deterministic order (directive-before-token on a timestamp tie). Query
them with:

```bash
canon query --kind event   # then filter client-side on detail.label == "user_directive"
```

## Reading the run summary

Each pass prints the active scope, one line per adapter, then totals:

```
scope: project /Users/me/Workspace/canon (2 roots)
omp: 3 file(s) scanned, 1 reparsed, 2 skipped unchanged (watermark), 4 row(s) parsed, 0 malformed record(s)
claude-code: 1 file(s) scanned, 0 reparsed, 1 skipped unchanged (watermark), 0 row(s) parsed, 0 malformed record(s)
sessions normalized: 4
malformed records (corrupt line/db, counted as violations): 0
rows skipped (malformed session_id): 0
runs written: 4
events written: 37
```

- **scope** — the active project scope (`--all-workspaces` names it).
- **file(s) scanned** — every present file this source matched, after
  pruning, before the per-file gate.
- **reparsed** — the subset actually (re-)parsed this pass (`0` on a
  steady-state pass).
- **skipped unchanged (watermark)** — files skipped because that file's
  digest was byte-identical to its cursor entry.
- **malformed records** — corrupt lines / dbs an adapter hit but could
  not extract a row from (counted as a violation, never crashing).
- **store tiers unreachable** — if `canon.yaml`'s tiers aren't reachable
  (e.g. `tiers.pg` set but `CANON_PG_DSN` unset) or `session`/`run`/
  `event` aren't routed, the pass prints the normalized bundle as JSON
  instead of persisting — never a partial write.

## What this skill does NOT cover

- Artifact/verdict ingestion (`canon ingest artifacts`) — a different
  pipeline; see the `canon-artifact-ingest` skill.
- Cost-parity computation — omp/pi's own cost is `0.0`/`Unknown` per the
  ported behavior.
