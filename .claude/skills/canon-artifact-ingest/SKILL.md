---
name: canon-artifact-ingest
description: How to run canon ingest artifacts — the pipeline that reads a repo's canon-native artifacts (divergence reports, review ledgers, openspec tasks.md checkbox flips, handoff transitions), normalizes each into a regime-keyed event, derives a role-scoped verdict, and persists trajectories/verdicts idempotently. Use when ingesting review/divergence/task/handoff artifacts, feeding the reward flywheel's evidence source, or reading a role's own verdict stream.
---

# canon-artifact-ingest

`canon ingest artifacts` turns a repo's canon-native ARTIFACTS — the
outputs of review/divergence/task/handoff work — into the regime-keyed
verdict trajectories that strategy memory and the reward gate consume
(see `canon-learn`). One pass runs the four adapters, normalizes, derives
verdicts, and writes idempotently.

## `canon ingest artifacts [--repo <dir>] [--watch]`

```bash
canon ingest artifacts                # one pass over the repo's artifacts
canon ingest artifacts --repo ../svc  # a specific repo
canon ingest artifacts --watch        # poll loop
```

Source roots resolve from `canon.yaml`'s `artifacts.sources` config
(`openspec_root`, ledger/divergence/handoff roots) — generic, never a
hardcoded path; ordinarily the consumer repo's own roots. Unconfigured =
no scan for that family.

## The four adapters

Each reads one artifact family and emits a normalized event keyed by that
family's join key:

- **`divergence`** — parity/divergence reports → verdict keyed by the
  lane derived from the report's on-disk path.
- **`ledger`** — review ledger records → verdict; the record's
  trust-level tag (`@reviewed`/`@ratified`) rides through as a
  passthrough `trust_level` field, not collapsed into one "success"
  bucket. (The other three adapters carry no trust-level concept.)
- **`openspec_task`** — `tasks.md` checkbox rows: a `- [ ]` → `- [x] … —
  ✅ <evidence>` flip, or a `**DEFERRED**`/`**DROPPED**` rewrite, keyed by
  `task_id` (`<change_id>#<n>`). The evidence string is parsed for a
  mergeable PR / CI reference where present (CI-first).
- **`handoff`** — handoff transitions, keyed by `handoff_id`.

## Verdict derivation

Verdict derivation is a pure function from a normalized event to an
optional verdict `{role, polarity, becomes}`:

- A matched mapping row → a verdict for that role.
- No mapped row → NO verdict. A prose-only task flip, a deferred/dropped
  task, or a handoff transition alone produces none — deliberate, not a
  gap.

## Idempotence — a re-ingest is a no-op

Write-identity is content-digest based: the digest derives both the
`regime_key` `<hash>` segment and the trajectory write-identity. Before
writing, the driver does a regime-key existence check — a second ingest
over an unchanged corpus re-derives the identical id and persists zero
new trajectories (reported as `trajectories_skipped_duplicate`).

## Reading a role's own verdict stream

Verdicts persist regime-keyed (`<role>/<repo>/<area>/<hash>`), so a
role-scoped agent reads only its own lane — the same regime namespacing
`canon retrieve --role <r> --regime <k>` and `canon learn` query. This
verdict stream is the evidence the reward gate scores when deciding
promotion/demotion.

## What this skill does NOT cover

- Session/cost ingestion (`canon ingest sessions`) — a different
  pipeline; see the `canon-session-ingest` skill.
- Strategy promotion/demotion mechanics — see the `canon-learn` skill;
  this skill only produces the verdict evidence promotion reads.
- The `regime_key` grammar itself — see `canon-retrieve`, which derives
  the same key via the `canon regime-key` serializer.
