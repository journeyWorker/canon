---
name: canon-storage
description: How canon's storage rungs (local/hot/cold) and their backends (git/postgres/sqlite/s3) work — configuring canon.yaml's tiers/routing/aging sections, aging records with canon tier age, reading them with canon query, and what canon report does and doesn't surface. Use when editing canon.yaml's tiers/routing/aging config or querying canon's stored records.
---

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
