---
name: canon-artifact-ingest
description: How to run canon ingest artifacts — the S4 (artifact-ingest) pipeline that reads a repo's canon-native artifacts (divergence reports, review ledgers, openspec tasks.md checkbox flips, handoff transitions) through four source adapters, normalizes each into a regime-keyed ArtifactEvent, derives a role-scoped verdict via the review→verdict mapping table, and persists trajectories/verdicts idempotently into the tiered store. Use when ingesting review/divergence/task/handoff artifacts, when wiring the S7 reward flywheel's evidence source, or when reading a role's own verdict stream.
---

# canon-artifact-ingest

S4 (`s4-artifact-ingest`) turns a repo's canon-native ARTIFACTS — the
outputs of review/divergence/task/handoff work — into the regime-keyed
verdict trajectories the S6 strategy memory + S7 reward flywheel consume.
`canon ingest artifacts` (the S14 capstone driver,
`crates/canon-cli/src/artifact_ingest.rs`) runs the four adapters over a
repo, normalizes, derives verdicts, and writes idempotently.

## `canon ingest artifacts [--repo <dir>] [--watch]`

```bash
canon ingest artifacts                # one pass over the repo's artifacts
canon ingest artifacts --repo ../svc  # a specific repo
```

Source roots resolve from `canon.yaml`'s `ArtifactSourceConfig`
(`openspec_root`, ledger/divergence/handoff roots) — GENERIC, never a
hardcoded path; ordinarily the consumer repo's own roots.

## The four adapters

Each adapter (`crates/canon-ingest/src/artifact_adapters/`) reads one
artifact family and emits a normalized `ArtifactEvent` keyed by the join
key that family lives on:

- **`divergence`** — parity/divergence reports → verdict keyed by the
  lane derived from the report's on-disk path.
- **`ledger`** — review ledger records → verdict; the record's own
  trust-level tag (`@reviewed`/`@ratified`) rides through as a
  passthrough `trust_level` field, NOT collapsed into one "success"
  bucket. (The other three adapters emit `trust_level: None` — their
  sources carry no trust-level concept.)
- **`openspec_task`** — `tasks.md` checkbox rows: a `- [ ]` → `- [x] … —
  ✅ <evidence>` flip, or a `**DEFERRED**`/`**DROPPED**` rewrite, keyed by
  `task_id` (`<change_id>#<n>`). The evidence string is parsed for a
  mergeable PR / CI reference where present (CI-first).
- **`handoff`** — handoff transitions, keyed by `handoff_id`, driven
  purely by the records the driver hands it (a records-source adapter, no
  path resolution of its own).

## Verdict derivation (the mapping table)

`derive_verdict` (specs/review-verdict-mapping) is a PURE function from a
normalized `Event` to an optional verdict `{role, polarity, becomes}`:

- A matched row → a verdict for that role.
- No mapped row → NO verdict (a prose-only task flip, a deferred/dropped
  task, or a handoff transition alone produces none — this is
  deliberate, not a gap).

## Idempotence — a re-ingest is a no-op

Every normalized event + verdict reuses S3's content-digest
write-identity: `content_digest` derives BOTH the regime_key `<hash>`
segment (`regime_hash`) AND the trajectory write-identity
(`trajectory_content_digest`). Before writing a trajectory the driver
does a `query_by_regime_key` existence check — a second ingest over an
unchanged corpus re-derives the identical id and persists zero new
trajectories (`trajectories_skipped_duplicate` counts the skips). Proven
by `crates/canon-cli/tests/artifact_ingest.rs::a_second_ingest_over_an_unchanged_corpus_persists_zero_new_trajectories`.

## Reading a role's own verdict stream

Verdicts persist regime-keyed (`<role>/<repo>/<area>/<hash>`), so a
role-scoped agent reads only its own lane — the same regime namespacing
`canon retrieve --role <r> --regime <k>` (S8) and `canon learn` (S6)
query. The verdict stream is the evidence the S7 reward gate scores when
deciding promotion/demotion.

## What this skill does NOT cover

- Session/cost ingestion (`canon ingest sessions`) — a DIFFERENT pipeline
  (S3); see the session-ingest skill.
- Strategy promotion/demotion mechanics — see the `canon-learn` skill;
  this skill only produces the verdict evidence promotion reads.
- The regime_key grammar itself — see `canon-retrieve`'s pre-dispatch
  hook, which derives the SAME key via the `canon regime-key` serializer.
