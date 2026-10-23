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

**Done when:** after `canon tier age --commit --plan-digest <digest from the
preview>`, a bare `canon tier age` previews no effect (`moved` is `0`).

## Running `canon tier age` / `canon query`

Both read the `canon.yaml` of `--repo <dir>` (default: the nearest ancestor
directory holding one).

```bash
# Preview what would move (the bare run is read-only; writes/deletes nothing).
canon tier age

# Apply every `aging:` rule once (the destructive move+delete), with the
# `planDigest` the preview printed.
canon tier age --commit --plan-digest "$PLAN_DIGEST"

# Read a kind, fanning out across its routed rung AND its aging
# destination, merged by time. --kind is the kind wire string — the
# same vocabulary as routing:/aging: (`change`, `handoff`, …).
canon query --kind handoff --since 2026-06-01T00:00:00Z

# Machine-readable output instead of the human table.
canon query --kind trajectory --json

# Page a long result and keep only the fields you need; `meta` carries
# returned, total, hasMore and nextCursor (`--all` returns everything).
canon query --kind trajectory --json --limit 50 --fields at,kind
canon query --kind trajectory --json --limit 50 --cursor <nextCursor>
```

## The ledger is latest-only; the archive holds history

The `local` rung's directory (`.canon/ledger`) holds the current record of
each key. Appending a newer version (evidence for a scenario and role, a
closed finding, a rewritten scenario, a subject's next status) moves the
versions it supersedes to `.canon/archive`: same `kind=<kind>/…` layout and
file names, tracked in git. `status`, the gate, the guards and `canon query`
read the ledger only.

```bash
canon query --kind evidence --history   # every version, archive included
canon ledger compact                           # preview; writes nothing
canon ledger compact --commit --plan-digest "$PLAN_DIGEST"   # pre-0.23 ledger or interrupted move; idempotent; `canon migrate` runs it
```

A crash between the write and the move leaves a readable ledger that the next
append or `canon ledger compact` finishes. The archive is local, not routed
through `tiers:`; a `tiers.local.root` whose directory is not named `ledger`
has no archive and keeps every version.

**Done when:** after `canon ledger compact --commit --plan-digest <digest
from the preview>`, a bare `canon ledger compact` previews no effect.

## Where artifact bytes live (`artifact_store:`)

```yaml
artifact_store: { backend: local }                  # default
# artifact_store: { backend: cold, prefix: artifacts/ }   # an `s3` tier of `tiers:`
```

`local` is `.canon/artifacts/sha256/`, git-tracked and credential-free. An
object-store tier gets `evidence add`, `review scenario` and `artifacts store`
uploads under `<prefix>sha256/<digest>` and keeps nothing locally. The gate
and `canon artifacts verify [--full]` ask the store and refuse, never pass,
when it cannot be reached. `canon artifacts move --tier <tier>` (a preview; `--commit --plan-digest <digest>` acts)
uploads, reads back and verifies every local blob, then writes the
`artifact_store:` block, removes the local copies and ignores
`.canon/artifacts/`; commit the removals, `.gitignore` and `canon.yaml`. It is
resumable and a second run finds nothing to do.

## What `canon report build` does and doesn't read

canon exposes layered `stg_`/`int_`/`mart_` query views; `canon report build`
builds its numbers from the `mart_` views. Those read ONLY local roots
— the git ledger, the sqlite `hot` db file (local, no credential; read
since 0.18) and a LOCAL `.canon/r2` parquet mirror. `canon report build`
never opens a live DB connection or the live bucket, so:

- Kinds routed to a postgres `hot` rung do NOT appear in `canon report
  build`; kinds routed to `canon init`'s default sqlite `hot` rung do.
- Kinds routed to `cold` (s3) appear only if a local `.canon/r2` mirror
  has been separately materialized — there is no automatic sync.

These kinds are not lost: read them with `canon query --kind <kind>`.
The gap is named loud, not under-counted — when any routed kind isn't
report-readable, `canon report build` renders a `## Kinds not read directly`
section listing them and emits a matching stderr `WARN`.

## Retention, export, and access audit

`canon purge --kind <kind> --before <RFC3339> [--commit --plan-digest <planDigest>] [--repo <dir>] [--json]`
is the canonical data-lifecycle boundary: the bare run previews: it prints the removal set and
its `planDigest` and deletes nothing; `--commit --plan-digest <planDigest>` deletes only if
the plan recomputed now has the same digest (`PLAN_CHANGED`, exit 2, when the
data moved). The only purgeable kinds are
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

**Done when:** after `canon purge --kind <kind> --before <time> --commit
--plan-digest <digest from the preview>`, a bare `canon purge` with the same
arguments previews an empty removal set.

With `--json`, `purge` and `export` answer `{ "data", "meta" }` like every other
command: `data` is the plan or manifest, `meta.next` the step after (a plan's is the
human's review and `--commit --plan-digest`), and an export's page state (`total`, `returned`,
`nextCursor`) sits in `meta`. `export --json --output <file>` writes the manifest to the
file and answers with `data: { output, kind, recordCount }`; `--dry-run`
answers with what it would write and writes nothing.

`canon export --kind <kind> [--before <RFC3339>] [--after <RFC3339>]
[--output <file>] [--json] [--limit <n> | --all] [--cursor <c>]` emits a versioned manifest containing source scope,
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
add`, `canon finding add`, `canon finding close` and `canon divergence add`
(under `.canon/ledger/_staging/`, one staging shape for every kind,
committed by `canon gate promote`; `canon gate promote --kind divergence` commits only
the staged divergences). `canon divergence resolve|defer|accept` stage and
promote their own record in the same run. `canon.yaml`'s
`write:` section decides whether they promote in the same run:

```yaml
write:
  promote: auto   # or manual
```

- `auto` — the command stages, then promotes that one record through the
  same validation and assignment `canon gate promote` applies. Records
  other runs staged stay staged; only `canon gate promote` commits
  everything staged. The one success line names the committed record
  (`wrote kind=…/….json`; promotion renames it, so the staged file is not
  named) and ends `— staged and promoted in this run; nothing to
  promote`. `canon init` writes `auto`.
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

## Harnesses: the `skills:` section

`canon.yaml`'s `skills:` section names the harnesses the canon skill is
installed for (0.17):

```yaml
skills:
  providers: [claude, codex]   # any of claude, codex, omp, pi; [] for none
```

`canon init` writes it: on a terminal it asks, with the harnesses whose
`.claude`, `.codex`, `.omp` or `.pi` directory exists checked;
`--providers a,b` (or `none`) answers without asking; with neither it takes
the detected ones, and with none detected it writes only a commented example
and installs nothing. `.agents` alone never selects Codex. `canon skills
install|check|doctor` follow the section; `install` prunes projected files the bundle no longer ships when their bytes match the lock (a modified one stays, reported as `kept:`), `check` exits 1 (`SKILLS_DRIFT`) naming each orphan and `doctor` prints `orphan:` lines; `canon skills install
--providers=<list>` rewrites it in place. Without the section they use the
install lock's providers, then the detected ones; nothing undetected is ever
added. A present section that is not this shape, or names another harness,
fails those commands (exit `1`), and `canon init --check` reports
`[PASS] skills (providers: …)`, `[not configured] skills (…)`, or
`[FAIL] skills: …`.

## The scenario index

`canon inventory sync` materializes one `Scenario` record per
`(project_id, scenario_id)` under `.canon/ledger/kind=scenario/`. The
index is derived — a pure function of the `.feature` files. The index is
stale when a `.feature` file holds a scenario with no live index record,
or one whose recorded source digest differs from the file's current bytes,
or when the index holds a scenario of a spec root that no `.feature` file
of it declares any more.

The commands that write records (`canon subject move`, `canon review
scenario`, `canon evidence add` and `canon artifacts store`) rebuild a stale index on disk first: they
run `canon inventory sync`'s code on the changed files (validation still
covers the whole root) and print one stderr line:

```stderr
canon artifacts store: refreshed the scenario index from 1 changed feature file(s) (.canon/specs/kind=feature/area=cart/add.feature): 1 record(s) written
```

The commands that only read (`canon gate check`, `canon status`) write
nothing: they grade a stale index as the refresh would leave it, built in
memory, and print no refresh line. `canon query --kind scenario` and the
report panels read the index on disk as it is.

A deleted scenario is not erased: `canon inventory sync` — and a writer's
refresh, identically — appends a tombstone, a new generation of its
record with `removed: true`. The gate, `canon status`, `canon query` and
the report panels then treat it as absent; re-adding the scenario writes
a live generation that supersedes the tombstone. A root that fails to
sync writes no tombstone either.

```stderr
canon artifacts store: refreshed the scenario index from 1 scenario(s) no longer in any feature file (cart.pay.01): 1 record(s) written
```

`canon inventory sync` and every refresh hold an OS file lock on
`.canon/scenario-index.lock` while they write, so concurrent runs never
index one change twice; a refresh that waited checks staleness again and
is silent if another run already did the work. The lock is released when
its holder exits, however it exits, so it never goes stale; a writer
gives up after 30 s with a message naming the file (`canon inventory
sync` exits `2`; a writer's refresh is skipped and the command still runs).

A fresh index prints nothing. A root that fails to sync (format
violations, a duplicate id) is named and left as it was, and the command
still runs; `canon gate check` then fails with `uncovered-cell <root>`,
the `verifying → shipped` guard refuses, and `canon status` lists only
`canon inventory sync`:

```stderr
canon artifacts store: found the scenario index stale for 1 changed feature file(s) (…): 0 record(s) written; NOT refreshed: root `root` (2 format violation(s), 0 sync error(s)) — `canon inventory sync` lists them
```

Nothing is written in a directory without `canon.yaml`. `canon inventory
sync` stays the explicit command, and the one that lists violations and
tag diagnostics.

## The artifact store lock

Every write to the evidence artifact store (`.canon/artifacts/sha256/`:
`canon evidence add`, `canon review scenario`, `canon artifacts store`) holds an
OS file lock on `.canon/artifact-store.lock` from its "already stored?"
check through its last rename, so a write that fails and rolls back never
removes a blob another writer published or counted as stored. A rollback
removes only the blobs its own write renamed into place; a blob already
stored intact counts as stored and is not written again. The lock is
released when its holder exits, however it exits, so it never goes stale;
a writer gives up after 30 s with a message naming the file and saying to
retry (the command exits `2` and writes no record).

## What `canon init` keeps out of git

A fresh `canon init` writes these lines to `.gitignore` (each under a
comment, only the missing ones):

- `.canon/hot.db*` — the sqlite hot tier and its WAL/SHM sidecars.
- `.canon/audit/` — the query/export/purge access log.
- `.canon/ledger/kind=scenario/` — the scenario index; every reader
  rebuilds it.
- `.canon/archive/kind=scenario/` — superseded scenario index rows;
  derived like the index.
- `.canon/scenario-index.lock` — the lock serializing index rebuilds.
- `.canon/artifact-store.lock` — the lock serializing artifact store
  writes.
- `.canon/dashboard-snapshot/` — the snapshot `canon dashboard`
  regenerates on every run.
- `.canon/learn/` and `.canon/r2/` — local learn scratch and the local
  object-store stand-in `canon report build` writes.

A rerun on a repo that already has `canon.yaml` only refreshes
`AGENTS.md`; `canon migrate` adds the lines an existing `.gitignore`
lacks, and `canon skills doctor` and `canon init --check` name a
missing one.

## Upgrading a 0.27 repository

`canon status`, `canon gate check` and `canon context` warn on a repo that
still holds 0.27 canon state and put `canon migrate --json` first in
`meta.next`. `canon migrate` is destructive-class: the bare run is a
preview (a report with `planDigest` and `effects[]`, exit 0, nothing
written); `canon migrate --commit --plan-digest <digest from the preview>`
acts, and only when the plan recomputed now has the same digest. Afterwards
run `canon skills install`, which writes the stub guide; the guide itself is
read with `canon skills show canon`.

**Done when:** `canon status` no longer warns about 0.27 state and
`canon migrate --json` is no longer the first step in `meta.next`.
