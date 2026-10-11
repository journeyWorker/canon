# canon-storage

canon stores every record in one of three capability **rungs**:

- `local` — diffable files in the repo (git-backed), zero network.
- `hot` — live-queryable state (sqlite or postgres).
- `cold` — bulk archive (s3).

A rung is a *role*; which backend implements it is a separate
declaration. `routing:`/`aging:` name a rung (`local`/`hot`/`cold`),
never a backend name. `tiers.<rung>.backend:` names the backend.

## Backends and their config

| backend    | rung role | `tiers.<rung>` config              | credentials / network |
| ---------- | --------- | ---------------------------------- | --------------------- |
| `git`      | local     | `root:` (relative to canon.yaml)   | none — zero network   |
| `sqlite`   | hot       | `path:` (relative to canon.yaml)   | none                  |
| `postgres` | hot       | `dsn_env:` + `schema:`             | live network + creds  |
| `s3`       | cold      | `bucket_env:` + `prefix:`          | live network + creds  |

`canon init` scaffolds `sqlite` as the zero-dependency `hot` default
(WAL journal + busy timeout, fine for one operator's batch ingest).
sqlite is a **single-writer store**: for heavy multi-agent concurrent
writers, swap the `hot` rung to `postgres` — same hot role, a one-block
canon.yaml swap (the scaffold ships the postgres stanza commented right
beside the sqlite one). A postgres/s3 rung that can't attach fails loud
at startup — never a silent fallback to local files.

```yaml
tiers:
  local: { backend: git, root: .canon/ledger }
  hot:   { backend: sqlite, path: .canon/hot.db }
  # hot: { backend: postgres, dsn_env: CANON_PG_DSN, schema: canon }
  cold:  { backend: s3, bucket_env: CANON_BUCKET, prefix: canon/ }
```

Using a backend name (`git`/`pg`/`s3`) where a rung is expected is
rejected with a hint:

```
canon.yaml TierPolicy: `git` is a BACKEND name, not a rung — canon.yaml's
`routing`/`aging`/`tiers` keys now name a capability rung
(local/hot/cold); declare the backend separately via
`tiers.<rung>.backend: git`
```

## Routing a record kind

`routing:` is the only place a kind's rung is decided. Keys are the
stable snake_case kind wire strings (`change`, `evidence_record`,
`strategy_item`, …) — the same string the record's own `kind` field
serializes to. Values are `local`/`hot`/`cold`.

```yaml
routing:
  scenario: local   # change this to move FUTURE `scenario` writes
```

Changing a routing value moves future writes only; re-run reads
resolve the policy live. An unknown kind or rung name is a loud config
parse error. Every record kind needs a `routing:` entry — an unrouted
kind fails hard at its first write/query, never silently dropped.

## Aging a kind to another rung

`aging:` moves records from their routed rung to a `to:` rung once
their age exceeds `after:`:

```yaml
aging:
  handoff: { after: 30d, to: cold }   # <n>d/h/m/s — one integer, one unit
```

`canon tier age` applies every `aging:` rule once. It is
content-digest idempotent: re-running finds nothing left to move
(`moved == 0`); a partial run (destination written, source not deleted)
re-selects the record but the destination write is a no-op. A kind
split across two rungs (some aged, some not) still reads correctly —
`canon query` fans out.

## Running `canon tier age` / `canon query`

Both take `--canon-yaml <path>` (default `canon.yaml` in the current
directory).

```bash
# Preview what would move (read-only; writes/deletes nothing).
canon tier age --dry-run

# Apply every `aging:` rule once (the destructive move+delete).
canon tier age

# Read a kind, fanning out across its routed rung AND its aging
# destination, merged by time. --kind is the kind wire string — the
# same vocabulary as routing:/aging: (`change`, `handoff`, …).
canon query --kind handoff --since 2026-06-01T00:00:00Z

# Machine-readable output instead of the human table.
canon query --kind trajectory --json
```

## What `canon report` does and doesn't read

canon exposes layered `stg_`/`int_`/`mart_` query views; `canon report`
builds its numbers from the `mart_` views. Those read ONLY local roots
— the git ledger and a LOCAL `.canon/r2` parquet mirror. `canon report`
never opens a live DB connection or the live bucket, so:

- Kinds routed to `hot` (sqlite/postgres) do NOT appear in `canon
  report`.
- Kinds routed to `cold` (s3) appear only if a local `.canon/r2` mirror
  has been separately materialized — there is no automatic sync.

These kinds are not lost: read them with `canon query --kind <kind>`.
The gap is named loud, not under-counted — when any routed kind isn't
report-readable, `canon report` renders a `## Kinds not read directly`
section listing them and emits a matching stderr `WARN`.

## Retention, export, and access audit

`canon purge --kind <kind> --before <RFC3339> [--repo <dir>] [--dry-run] [--json]`
is the canonical data-lifecycle boundary. The only purgeable kinds are
`session`, `run`, `event`, and `trajectory`; the cutoff applies to every
routed rung and every configured aging destination, including retained
historical versions. For `trajectory`, the command also purges the
operator-local Parquet learn store under `learn.root` in `canon.yaml`;
an unavailable configured learn root is an explicit error, never a
successful zero-row purge. The operation is idempotent. Apply mode
preflights all attached destinations before the first delete, but storage
backends are not one transaction: a failure after a prior delete returns
an explicit `partial` result with matched/deleted counts and a nonzero
status. Purge never rewrites Git history or deletes source transcript
files.

`canon export --kind <kind> [--before <RFC3339>] [--after <RFC3339>]
[--out <file>] [--json]` emits a versioned manifest containing source scope,
counts, digests, and redacted metadata. It does not authorize backend access;
credentials remain an operator prerequisite. New output files are mode
`0600`. Raw event detail text is excluded unless `--include-sensitive` is
passed and `.canon/policy.yaml` explicitly contains:

```yaml
query:
  allow_sensitive: true
```

Every query, export, and purge appends an actor-stamped record to
`.canon/audit/access.jsonl` (mode `0600` on creation) in the repository
whose `canon.yaml` it read (through `--repo` or `--canon-yaml`), never in
the current directory; with no `canon.yaml` there is no audit and nothing
is written. The log is local operator state: `canon init` keeps
`.canon/audit/` out of git (below). `CANON_ACTOR` takes
precedence over the OS username. Audit rows contain operation, kind, scope,
sensitive flag, counts, and timestamp only—never query text, secrets, or
record bodies.

## Staged writes and promotion

Four writes stage their record instead of committing it: `canon evidence
add`, `canon finding add` and `canon finding close` (under
`.canon/ledger/_staging/`, committed by `canon gate promote`) and `canon
divergence stage` (committed by `canon divergence promote`). `canon.yaml`'s
`write:` section decides whether they promote in the same run:

```yaml
write:
  promote: auto   # or manual
```

- `auto` — the command stages, then promotes that one record through the
  same validation and assignment `canon gate promote` (or `canon
  divergence promote`) applies, and prints its promote line. Records
  other runs staged stay staged; only `canon gate promote` / `canon
  divergence promote` commit everything staged. The success line ends
  `— staged and promoted in this run; nothing to promote`. `canon init`
  writes `auto`.
- `manual` — the record stays staged and the line ends ``— run `canon
  gate promote` to commit it``. An absent `write:` key (or `promote:`
  field) is `manual`, so a repo that never set it is unchanged.

`--promote` / `--no-promote` decide for one write and win over the config
without reading it; both at once is a usage error (exit `2`). If the
promotion refuses the command's own record, the record stays staged, the
line ends ``— run `canon gate promote` to commit it`` (`canon divergence
promote` for a divergence), and the command exits `1`. With neither flag,
a malformed `write:` section (anything but `promote: auto | manual`)
refuses the write, exit `2`, before anything is staged. `canon init
--check-config` reports `[PASS] write (promote: auto)`, `[not configured]
write (promote: manual)`, or `[FAIL] write: …`. `canon gate promote` still
works under `auto` and is idempotent: with nothing staged it prints
`nothing staged` and exits `0`.

## The scenario index

`canon inventory sync` materializes one `Scenario` record per
`(project_id, scenario_id)` under `.canon/ledger/kind=scenario/`. The
index is derived — a pure function of the `.feature` files — so the
commands that read it rebuild it first: `canon gate check`, `canon
status`, `canon subject status`, `canon review add` and `canon evidence
add`. The index is stale when a `.feature` file holds a scenario with no
live index record, or one whose recorded source digest differs from the
file's current bytes, or when the index holds a scenario of a spec root
that no `.feature` file of it declares any more. The reader then runs
`canon inventory sync`'s code on the changed files (validation still
covers the whole root) and prints one stderr line:

```
canon status: refreshed the scenario index from 1 changed feature file(s) (specs/features/kind=feature/area=cart/add.feature): 1 record(s) written
```

A deleted scenario is not erased: `canon inventory sync` — and a reader's
refresh, identically — appends a tombstone, a new generation of its
record with `removed: true`. The gate, `canon status`, `canon query` and
the report panels then treat it as absent; re-adding the scenario writes
a live generation that supersedes the tombstone. A root that fails to
sync writes no tombstone either.

```
canon status: refreshed the scenario index from 1 scenario(s) no longer in any feature file (cart.pay.01): 1 record(s) written
```

`canon inventory sync` and every refresh hold an OS file lock on
`.canon/scenario-index.lock` while they write, so concurrent runs never
index one change twice; a refresh that waited checks staleness again and
is silent if another run already did the work. The lock is released when
its holder exits, however it exits, so it never goes stale; a writer
gives up after 30 s with a message naming the file (`canon inventory
sync` exits `2`; a reader skips the refresh and still runs).

A fresh index prints nothing. A root that fails to sync (format
violations, a duplicate id) is named and left as it was, and the command
still runs:

```
canon gate check: found the scenario index stale for 1 changed feature file(s) (…): 0 record(s) written; NOT refreshed: root `root` (2 format violation(s), 0 sync error(s)) — `canon inventory sync` lists them
```

Nothing is written in a directory without `canon.yaml`. `canon inventory
sync` stays the explicit command, and the one that lists violations and
tag diagnostics.

## What `canon init` keeps out of git

A fresh `canon init` writes these lines to `.gitignore` (each under a
comment, only the missing ones):

- `.canon/hot.db*` — the sqlite hot tier and its WAL/SHM sidecars.
- `.canon/audit/` — the query/export/purge access log.
- `.canon/ledger/kind=scenario/` — the scenario index; every reader
  rebuilds it.
- `.canon/scenario-index.lock` — the lock serializing index rebuilds.

A rerun on a repo that already has `canon.yaml` only refreshes
`AGENTS.md`, so an existing repo's `.gitignore` is unchanged; add the
lines by hand to adopt them.
