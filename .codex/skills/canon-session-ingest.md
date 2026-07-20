# canon-session-ingest

> How to run canon ingest sessions [--watch] [--full] [--all-workspaces] — the S3 (session-ingest) pipeline, scoped by default to the current project (s31), that scans every registered agent-CLI transcript source (omp/pi, Hermes, Claude Code, Codex), normalizes each into canon-model Session/Run/Event records (including user_directive events captured from user turns), and persists them through canon-store's tiered write path, incrementally via a per-FILE watermark cursor. Use when ingesting agent session/cost/directive telemetry, running a --watch poll loop, widening the scan machine-wide, resetting the watermark, or reading the per-adapter run summary.

# canon-session-ingest

S3 (`s3-session-ingest`) turns every agent CLI's on-disk transcript store
into canon-model `Session`/`Run`/`Event` records on canon's join spine.
`canon ingest sessions` (`crates/canon-cli/src/ingest.rs`) scans the four
registered adapters, normalizes, and persists through canon-store — the
same tier resolution `canon query` / `canon tier age` use. s31
(`s31-scoped-session-ingest`) scoped the default corpus to the current
PROJECT, made the watermark gate per-FILE, and added user-directive
capture — see D1/D3/D4 below.

**Quick start needs zero docker/services.** `canon init` (s32
`sqlite-hot-backend`) scaffolds `hot` as a local sqlite file
(`canon/hot.db`) with `session`/`run`/`event` already routed to it —
a fresh repo's FIRST `canon ingest sessions` persists real records
with no `docker compose` stack, no `CANON_PG_DSN`, no live database
at all. Swap `hot` to postgres (the commented stanza `canon init`
ships right beside the live sqlite one) once team-scale multi-agent
concurrency outgrows sqlite's single-writer WAL mode.

## `canon ingest sessions [--watch] [--interval-secs N] [--home <dir>] [--canon-yaml <path>] [--full] [--all-workspaces]`

```bash
canon ingest sessions                 # one pass over this project's sources
canon ingest sessions --watch         # poll every 30s (see --interval-secs)
canon ingest sessions --full          # ignore the watermark; re-parse every in-scope file
canon ingest sessions --all-workspaces  # machine-wide scan (pre-s31 behavior)
canon ingest sessions --home /custom  # override the scan-root home ($HOME)
```

## The four adapters (declaration order)

`canon-ingest`'s static registry ships, in order: **`omp`** (omp/pi
sessions), **`hermes`**, **`claude`** (`~/.claude/projects/**/*.jsonl`),
**`codex`** (`${CODEX_HOME:-~/.codex}/sessions/` UNIONED with
`archived_sessions/` — a live-only scan under-counts rotated sessions).
Each resolves its documented default root and honors an env override
(`CANON_INGEST_OMP_SESSIONS_DIR`, `CANON_INGEST_CLAUDE_SESSIONS_DIR`,
`CODEX_HOME`, `HERMES_HOME`). An absent source root is a zero-record,
non-fatal skip (never a crash).

### Per-source root overrides (`canon.yaml`)

A `canon.yaml` MAY redirect any adapter's scan roots without touching the
frozen `SessionAdapter` trait (the override lives at the canon-cli layer;
the adapter still does the parsing):

```yaml
ingest:
  sources:
    omp:
      roots: [/data/omp-sessions]   # relative paths resolve against the canon.yaml dir
    codex:
      roots: []                      # explicit empty = scan ZERO (not the default)
```

A source with a **present** `roots` scans exactly those (an explicit
`roots: []` scans nothing); a source whose key or `roots` field is
**absent** keeps its own env-override + documented-default resolution.
Unlike artifacts (`artifacts.sources`, where unconfigured = no scan),
sessions default to their home roots when unconfigured. A PRESENT but
broken `ingest:` section fails **loud** (`IngestError::Config`) — a typo
(`root:` for `roots:`), an unknown source id (`claude` vs `claude-code`),
or a non-YAML canon.yaml — rather than silently scanning the wrong
corpus; a missing/unreadable canon.yaml stays a soft no-config.

## Project scope (s31 D3) — the default corpus

`canon ingest sessions` defaults to THIS PROJECT's own sessions, never
the whole machine: the repo's main `git worktree` root plus every linked
worktree (`git worktree list --porcelain`, resolved once per pass).
Outside a git repo (or with git absent), the scope fails soft to the
repo root alone — never an error.

- **omp/pi and Claude Code are cwd-PARTITIONED on disk** — one
  subdirectory per project, named by a lossy forward-encoding of the cwd
  (every `/` -> `-`). Out-of-scope subdirectories are PRUNED at
  enumerate time — never read, never digested, never counted.
- **Codex and Hermes are NOT partitioned** — every session lives in one
  shared transcript store. Rows are filtered POST-PARSE by
  `workspace_key` membership instead (ordinary filtering, not malformed);
  a row with no captured workspace is kept, fail-soft.
- **`--all-workspaces`** restores the pre-s31 machine-wide scan. The
  scope is still RESOLVED even with this flag (so `project_key` stamping
  below stays correct) — only the pruning/filtering are disabled.
- Every ingested `Session` whose workspace resolves into the active
  project set gets `project_key` stamped to the MAIN worktree's own
  normalized key — a session recorded in a linked worktree carries the
  SAME `project_key` as one recorded in the main worktree, so `canon
  query --kind session` aggregates a whole project's sessions regardless
  of which worktree recorded them.
- The run summary's first line always names the active scope:
  `scope: project /repo/root (2 roots)` or `scope: all workspaces`.

## The watermark (s31 D1) — per-FILE, incremental `--watch`

Each pass content-digests every present (post-scope-pruning) file and
diffs it against its source's persisted cursor
(`canon_store::cursor::SourceCursor`, under `<repo>/canon/ingest/cursors/`,
gitignored) via `SourceCursor::diff`, written AFTER a durable pass. A
file whose digest exactly matches the cursor is SKIPPED — never handed
to parse; a new or changed file is (re-)parsed. This supersedes S3 §3's
original SOURCE-granular all-or-nothing gate: every registered adapter
derives one session from exactly one file, so a single growing
transcript among thousands now re-parses ALONE instead of dragging the
whole source back through parse/normalize/persist.

- The gate is **sound**: a file is skipped only when its content digest
  matches the cursor, so a new, changed, copied, or restored file is
  always re-scanned (never false-skipped).
- The fresh cursor built each pass records EVERY readable present file
  (unchanged and changed alike), so next pass's diff stays sound.
- Correctness never depends on the cursor: a missing/corrupt cursor
  degrades to treating every present file as new, and the
  digest-idempotent write path (S3 4.2) keeps any rescan from
  double-writing.

**`--full`** ignores the cursors and re-parses every present in-scope
file (a full rescan / cursor reset) — safe because the digest-idempotent
write path means a byte-identical resubmission is a no-op (no
duplicates); cursors re-advance afterward.

## User directives (s31 D4)

Every adapter (except Hermes, whose format carries no user-turn text —
documented in its own module doc, always emits zero directives) emits
one `Event` per USER-role message: `label: "user_directive"`,
`detail: { text, workspace_key, workspace_label }`, the FULL verbatim
text (no truncation). These interleave with `token_usage` events in one
deterministic `seq` order (chronological, directive-before-token on a
timestamp tie — the human turn precedes the reply it triggered), so
re-parsing a grown transcript re-emits byte-identical earlier events
(digest-deduped downstream). Query them with:

```bash
canon query --kind event   # then filter client-side on detail.label == "user_directive"
```

Distilling directives into S6 strategy memory or S8 retrieval guidance
is explicitly OUT OF SCOPE for s31 — a named follow-up.

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

- **scope** — the active s31 D3 project scope (`--all-workspaces` names
  it explicitly).
- **file(s) scanned** — every present file this source matched, after
  D3 pruning, before the D1 per-file gate.
- **reparsed** — the subset actually (re-)parsed this pass (`0` on a
  fully steady-state pass).
- **skipped unchanged (watermark)** — files skipped because THAT FILE's
  digest was byte-identical to its cursor entry.
- **malformed records** — corrupt lines / dbs an adapter hit but could
  not extract a row from (counted as a violation, never crashing).
- **store tiers unreachable** — if `canon.yaml`'s tiers aren't reachable
  (e.g. `tiers.pg` set but `CANON_PG_DSN` unset) or `session`/`run`/
  `event` aren't routed, the pass prints the normalized bundle as JSON
  instead of persisting (the documented seam) — never a partial write.

## What this skill does NOT cover

- Artifact/verdict ingestion (`canon ingest artifacts`) — a DIFFERENT
  pipeline (S4); see the `canon-artifact-ingest` skill.
- Intra-file byte-offset resume (S3 3.3) — a deferred further
  optimization on top of the cursor; the current watermark still reads
  each file to digest it, only skipping the parse/normalize/persist above.
- Distilling `user_directive` events into S6 strategy memory or S8
  retrieval guidance (s31 D4) — a named follow-up, not this pipeline.
- Cost-parity computation — omp/pi's own cost is `0.0`/`Unknown` per the
  ported `pi.rs` behavior.
