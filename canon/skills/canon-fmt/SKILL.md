---
name: canon-fmt
description: How to run canon fmt --check over a consumer repo's artifact corpus (ledger/divergences/features/inventory/policy.yaml), what each violation line means, and how to fix it. Use when validating a corpus against canon's format.
---

# canon-fmt

`canon fmt --check` is canon's read-only format validator over a
consumer-repo artifact corpus: ledger records (run/drill/review/clear/
code-review/design-review), divergence events (manifest/review/
remediation), `features/`, `inventory/`, and `policy.yaml`. It never
rewrites or mutates the corpus — it only validates and reports.

## Running

```bash
canon fmt --check <corpus-root>   # e.g. a consumer repo's spec/
```

Read-only, always. Exits nonzero if any violation is found, and prints
a report grouped by failure class in this fixed order:

| Failure class | Meaning | Fix |
|---|---|---|
| `layout-grammar` | Wrong Hive path shape — a `features/`/`inventory/` file not under a `kind=<kind>/area=<area>/` prefix, a wrong leaf filename, or `assets.lock`'s ad-hoc format. | Move the file under the correct `kind=…/area=…/` prefix with the expected leaf name. |
| `missing-envelope` | A YAML file (`inventory/`, `policy.yaml`) has no top-level `schema`/`kind`/`at`/`actor` keys. | Add the four envelope keys at the top level. |
| `missing-provenance` | A `.feature` file's `Feature:`/`Scenario:` header has no `# canon: {...}` comment right after it. | Add the `# canon: {...}` provenance comment beneath the header. |
| `missing-actor` | No structured `actor` object — a bare `by` string, or absent. | Replace `by: <name>` with a structured `actor` object. |
| `unspecified-evidence` | A ledger `run`/`drill` record's `evidence` field is absent or an empty, untyped array. | Add a typed `evidence` array. |
| `free-text-ref` | `upstream_ref`/`port_ref` has a segment not matching `<file>#<symbol>[:<a>-<b>]`. | Rewrite the ref into the `<file>#<symbol>[:<a>-<b>]` grammar. |
| `joined-ref` | `upstream_ref`/`port_ref` is `;`- or `,`-joined but not split into a structured `refs` array. | Split into a `refs:` array, one entry per ref. |
| `abbreviated-sha` | `app_sha`/`harness_sha` is present but shorter than the full 40-hex form. | Use the full 40-character SHA. |
| `one-way-backref` | A divergence review event's `ledger_ref` has no reciprocal `divergence_refs` entry on the ledger record it names. | Add the matching `divergence_refs` entry to that ledger record. |
| `missing-join-identity` | Corpus-wide, one line: no ledger record carries `change_id`/`task_id`. | Standing note only — new records carry these going forward; historical backfill is out of scope. |

Every class is a stable string, safe to grep/diff across runs.

## What it does NOT do

- **Rewrite anything.** It only validates and reports; a one-shot
  migration of an existing corpus is a separate, per-repo concern.
- **Backfill `change_id`/`task_id` on historical records.** Both stay
  absent on records that predate the schema; `missing-join-identity` is
  a standing corpus-level note, not a per-record defect it fixes.
- **Guess a `{file, symbol}` from free prose.** A non-conforming ref is
  reported as `free-text-ref` exactly as found, never fabricated.
- **Touch a consumer repo's CI gates.** Conforming a corpus and
  reconciling its own tooling is that repo's own operational step.
