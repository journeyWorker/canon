# canon-session-ingest

`canon ingest sessions` turns every agent CLI's on-disk transcript store
into `Session`/`Run`/`Event` records on canon's join spine, scanning four
adapters, normalizing, and persisting through the same tier resolution
`canon query` / `canon tier age` use. The default corpus is scoped to the
current PROJECT; the watermark gate is per-file; user directives are
metadata-only unless explicitly opted in by bounded privacy config.

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

### Provenance, digests, and parser generations

The parser semantics are compared against Tokscale upstream at
https://github.com/junhoyeo/tokscale (revision
`fe72e1f9b5a2b1a927108b353a9730a5c631a196`); the cache-accounting comparison
reference is commit
[`2cd6c648df026fb43d37e492efa9c344e2d681ee`](https://github.com/junhoyeo/tokscale/commit/2cd6c648df026fb43d37e492efa9c344e2d681ee).
Canon ports the selected parser semantics, including Claude's nested
cache-write buckets, but does **not** vendor Tokscale's pricing or cache
service.

Three identities are intentionally distinct:

* the raw file SHA-256 watermark identifies unchanged transcript bytes;
* the normalized content digest is canonical JSON over the complete
  normalized event and changes when normalized token detail changes; and
* the parse-generation cursor identity combines adapter id and parse version,
  forcing a re-read when parser semantics change even if raw bytes do not.

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
- A cursor's identity carries its adapter's PARSE VERSION as well as its
  files, so changing an adapter's normalization re-reads that adapter's
  transcripts instead of reporting them unchanged — no `--full`, no
  cursor deletion. Version `1` IS the bare `client_id` (`omp.json`), so a
  cursor already on disk is never invalidated just by installing the
  mechanism; a bump appends the suffix (`claude-code-v3.json`) and that
  adapter re-reads once. `omp`, `hermes`, and `codex` are at `1`;
  `claude-code` is at `3`, because s37 changed its sidechain parse output
  (it now carries `agent_id`/`parent_agent_id`) and cache-accounting parity
  now recognizes nested cache-write buckets, so it re-reads once to backfill
  both normalization changes that the old watermark would otherwise hide.

**`--full`** ignores the cursors and re-parses every present in-scope
file (a full rescan / cursor reset) — safe because a byte-identical
resubmission is a no-op; cursors re-advance afterward.

## User directives

Every adapter (except Hermes, whose format carries no user-turn text)
emits one parsed `DirectiveRow` per USER-role message. Parsing retains
source text only in memory; the shared post-adapter privacy boundary
controls whether it becomes a durable event. The default when
`ingest.sessions.privacy` is absent is metadata-only: no
`user_directive` event reaches normalized output.

Repositories that explicitly opt in MUST set a positive bound:

```yaml
ingest:
  sessions:
    privacy:
      capture_user_directives: true
      max_directive_chars: 4096
```

Captured text is classified and redacted BEFORE the Unicode-scalar capture
bound is applied. The conservative classifier replaces known API
key/token/password assignments, bearer credentials, PEM private keys, and
email addresses with deterministic `[REDACTED_SECRET_N]` /
`[REDACTED_PII_N]` placeholders and includes `redaction_counts`. The
detail's `redacted` is true only when a known value was replaced;
`truncated` is independent and may be true for otherwise unredacted text.
This classifier is not a guarantee against every kind of secret or PII.
`workspace_key`/`workspace_label` remain metadata. The source transcript
remains the source of truth and is not rewritten, purged, or replaced by
Canon's normalized event.

The source cursor identity includes the privacy mode and positive bound, so
toggling capture or changing its bound re-scans unchanged source bytes.
Re-scans retain stable event sequence/key ordinals; they do not purge
previously stored normalized records. Operators owning pre-existing raw
data MUST handle purge/migration separately.

These events interleave with `token_usage` events in one deterministic
order (directive-before-token on a timestamp tie). Query them with:

```bash
canon query --kind event   # filter detail.label == "user_directive"
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
  `event` aren't routed, the pass prints a metadata-only JSON fallback
  instead of persisting — never a partial write. The fallback contains
  scope, adapter counts, record counts, IDs/digests, and a stable
  routing/tier failure class/reason; it never contains directive text,
  task/context prose, or raw event detail.

## What this skill does NOT cover

- Artifact/verdict ingestion (`canon ingest artifacts`) — a different
  pipeline; see the `canon-artifact-ingest` skill.
- Cost-parity computation — omp/pi's own cost is `0.0`/`Unknown` per the
  ported behavior.