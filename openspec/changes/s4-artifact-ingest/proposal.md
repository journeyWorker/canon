## Why

Code/design reviews, a consumer repo's ledger + divergence records
(the donor consumer repo is the reference source and fixture-corpus origin), canon's own
handoffs table, and openspec change/task state accumulate as write-only
artifacts today — nothing distills them into a decision loop. canon needs a
second ingest surface that reads these four sources — each through a
`canon.yaml`-configured, GENERIC source location, never a hardcoded live path
or DSN (S4 foundation rescope, operator directive 2026-07-11: canon never
reads live donor consumer-repo or donor-monorepo state) — and emits a normalized
**verdict stream**: the mapping from
"what happened" (a review finding, a clear-record, a merge, a revert) to "what it
means for role-scoped learning" (a guardrail candidate or a strategy candidate).
This is the concrete answer to "리뷰 되먹임" (review feedback loop) named in the
design's problem statement (§1).

## What Changes

- New adapters in `canon-ingest` for: a `canon.yaml`-configured ledger tree
  (`kind=run|review|clear|drill`; the donor consumer repo is the reference source, never a
  hardcoded path), a configured divergence JSONL tree (manifest/review/
  remediation events), canon's own handoffs table (read via `canon-store`'s
  Postgres tier — never the donor monorepo's live hosted Postgres), and openspec change/task state
  (`tasks.md` checkbox rows).
- Each adapter normalizes its source into canon-model events plus a
  **verdict** record: `{role, polarity, becomes}` derived from the design's
  review→verdict mapping table (design §5 S4), reproduced verbatim below.
- Severity + area tags on the source artifact become regime-key components
  (S1 `regime_key` grammar: `<role>/<repo>/<area>/<hash>`), so a verdict is
  retrievable by the same key S6/S7 read at strategy-lookup time.
- Idempotent re-ingest: re-running the artifact-ingest adapters over an
  unchanged source produces the same verdict stream, no duplicate verdicts.
- Golden-file fixture corpora — frozen, checked-in snapshots captured
  point-in-time from the donor consumer repo's corpus, never a live read — produce the
  expected verdict stream byte-for-byte (design §8 golden-file pattern).

### Review → verdict mapping (design §5 S4, verbatim)

| Input artifact | Role | Polarity | Becomes |
|---|---|---|---|
| code-review finding (open/still-divergent) | dev | failure | guardrail candidate |
| design-review finding | design | failure | guardrail candidate |
| review-record (promotion to @reviewed) | authoring role | success | strategy candidate |
| clear-record after @flagged | review | corrective | guardrail (what the sample caught) |
| remediation + later `resolved` | dev | success | strategy candidate |
| CI fail / PR revert | dev | failure | guardrail candidate |
| PR merge (no revert window) | dev | success | strategy candidate |

## Capabilities

### New Capabilities

- `artifact-ingest-adapters`: the ledger/divergence (`canon.yaml`-configured
  source root), handoff (canon's own Postgres-tier table), and openspec
  change/task-state adapters, normalizing each source into canon-model events
  keyed by the S1 join spine.
- `review-verdict-mapping`: the review→verdict mapping table above as an
  enforced, golden-file-tested transform from normalized ingest events to
  verdict records.

### Modified Capabilities

_None — canon has no existing specs yet; S4 lands alongside S3/S11/S12 in the
W1 ingest wave._

## Impact

- Extends `crates/canon-ingest` (introduced by S3) with four more adapters;
  does not introduce a new crate.
- Depends on `canon-model` (S1: envelope, `EvidenceRecord`, `Review`,
  `Handoff` wire-compatible with the prior event store's `handoffs` schema, `regime_key`
  grammar — the SINGLE canonical function, `canon-model::ids::regime_key`,
  S4 foundation) and `canon-store` (S2 tiers — the Postgres tier's
  `Tier::read` is the handoff adapter's ONLY handoffs source, never a direct
  DB connection from `canon-ingest` itself).
- Reads (does not write) a `canon.yaml`-configured ledger/divergence Hive
  tree (`ArtifactSourceConfig.ledger_root`/`divergences_root`, S4
  foundation) in place (S11 later migrates their on-disk format; S4's
  adapters target the pre-migration shape first, so S11 must keep this
  adapter's contract in mind — see S4 design "Decisions"). The donor consumer repo's
  `spec/ledger/`/`spec/divergences/` trees are the FIRST configured source
  and this component's fixture-corpus source — never a hardcoded path (S4
  foundation rescope, operator directive 2026-07-11).
- Reads **canon's own** handoffs table (S1-wire-compatible with the prior event
  store's schema, via `canon-store`'s Postgres tier — NEVER the donor monorepo's
  live hosted Postgres connection) and a `canon.yaml`-configured repo's
  `openspec/changes/*/tasks.md` checkbox state.
- New fixture corpora: frozen ledger/divergence samples (captured
  point-in-time from the donor consumer repo, checked into canon — never a live read)
  with a checked-in expected verdict-stream golden file.
- Companion skill (design §5 cross-cutting deliverable, decision 9): a
  `canon` artifact-ingest / verdict-stream skill under `canon/skills/`.
