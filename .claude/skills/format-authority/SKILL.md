---
name: format-authority
description: How to run canon fmt --check over a consumer repo's artifact-family corpus (ledger/divergences/features/inventory/policy.yaml) and read the violation report it prints. Use when touching crates/canon-fmt or running fmt --check against a corpus.
---

# format-authority

`canon-fmt` (`crates/canon-fmt`, S11) is canon's format-authority VALIDATOR
over an EXTERNAL consumer-repo artifact family — ledger run/drill/
review/clear/code-review/design-review records, divergence manifest/
review/remediation events, `features/`, `inventory/`, `policy.yaml`.
It validates someone else's on-disk corpus (e.g. a consumer repo's `spec/**`)
against the schemas + layout descriptors `canon-model`'s `family`
module registers (`crates/canon-model/src/family/`) — it never touches
canon's own storage tiers (that's `canon-store`/S2's job; see the
`tiered-storage` skill for that), and it never writes to the corpus it
validates.

## Running `canon fmt --check`

```bash
canon fmt --check <corpus-root>   # e.g. a consumer repo's spec/
```

Read-only, always. Exits nonzero if any violation is found (a linter's
own `--check` convention) and prints a human report grouped by failure
class, in this fixed order:

| Failure class | Meaning |
|---|---|
| `layout-grammar` | Wrong Hive path shape — a `features/`/`inventory/` file not yet under its `kind=<kind>/area=<area>/` prefix, a wrong leaf filename, or `assets.lock`'s fourth ad-hoc format. |
| `missing-envelope` | A YAML file (`inventory/`, `policy.yaml`) has no top-level `schema`/`kind`/`at`/`actor` keys. |
| `missing-provenance` | A `.feature` file's `Feature:`/`Scenario:` header has no `# canon: {...}` comment right after it. |
| `missing-actor` | No structured `actor` object — still a bare `by` string, or absent entirely. |
| `unspecified-evidence` | A ledger `run`/`drill` record's `evidence` field is absent or an empty, untyped array. |
| `free-text-ref` | `upstream_ref`/`port_ref` has a segment that doesn't match `<file>#<symbol>[:<a>-<b>]` — free prose, never guessed into a fake ref. |
| `joined-ref` | `upstream_ref`/`port_ref` is `;`- or `,`-joined but not yet split into a structured `refs` array (even if every segment DOES parse). |
| `abbreviated-sha` | `app_sha`/`harness_sha` is present but shorter than the full 40-hex grammar. |
| `one-way-backref` | A divergence review event's `ledger_ref` has no reciprocal `divergence_refs` entry on the ledger record it names. |
| `missing-join-identity` | Corpus-wide, ONE line: no ledger record anywhere carries `change_id`/`task_id` (new-only fields; historical backfill is out of scope — see "What `canon fmt --check` does NOT do" below). |

Every class is a stable wire string (`canon_fmt::FmtFailureClass::as_str()`)
— safe to grep/diff across runs.

## What `canon fmt --check` does NOT do

- **Rewrite anything.** There is no `canon migrate` — this crate only
  ever validates and reports; it never mutates a consumer repo's
  corpus. A one-shot migration of an existing corpus onto this format
  is a separate, later, per-consumer-repo concern (a throwaway script,
  not shipped by this skill or this crate).
- **Backfill `change_id`/`task_id` on historical records.** Both
  fields are schema-optional and stay absent on anything that predates
  this schema (design Non-Goal) — only records ingested by S4's future
  artifact-ingest populate them going forward. `missing-join-identity`
  in the fmt report is a standing, corpus-level note, not a per-record
  defect this tool will ever try to fix.
- **Guess a `{file, symbol}` from free prose.** A ref string that
  doesn't match `<file>#<symbol>[:<a>-<b>]` is reported (`free-text-ref`)
  exactly as found — never fabricated.
- **Touch a consumer repo's `parity.py` or any of its CI gates.** A
  consumer repo conforming its own corpus to this
  format, and reconciling its own tooling, is that repo's own
  operational step with its own review/CI discipline — out of scope
  here.

> Note: an earlier design for this change also specified `canon
> migrate` (an in-place rewrite tool), a one-shot consumer-repo corpus
> migration, and a `parity.py` sync patch. All three were removed per
> operator directive 2026-07-10 — the consumer repo will conform to canon's
> format on its own schedule, and a one-shot migration script (if ever
> needed) is a separate, later, throwaway concern per consumer repo.
> `canon-fmt` only ever validates.

## Reading the fixture corpus

`crates/canon-fmt/fixtures/consumer-corpus/pre/spec/` is a small,
representative corpus reproducing a real, audited consumer spec corpus's
drift shapes — every sample is grounded in a REAL corresponding record read
from that consumer corpus, not invented: a bare `by`, an abbreviated
`app_sha`/`harness_sha`, a free-text `upstream_ref`, a `;`-joined AND a
`,`-joined `port_ref` (including a same-file multi-symbol
continuation), the flat, non-Hive `features/`/`inventory/` layout,
`assets.lock`'s TSV format, an envelope-less `policy.yaml`, a one-way
divergence back-ref, and one deliberately ambiguous-partition
inventory file. `crates/canon-fmt/tests/fixtures_check.rs` exercises
`canon_fmt::check` against it end to end — this fixture corpus is
LOCAL to `canon-fmt`; it never reads a live consumer-repo checkout.
