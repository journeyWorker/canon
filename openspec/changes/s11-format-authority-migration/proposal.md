## Why

canon is declared the format authority for the whole review/ledger/divergence
artifact family (design decision 4). The 2026-07-10 artifact audit of
the donor consumer repo's `spec/**` (read sample-by-sample) found real, documented Hive
drift — three partition grammars, no schema envelope, no session/actor
identity anywhere, free-text refs instead of structured arrays. The gaps are
**family coherence and per-artifact expressiveness**, not gate errors. canon
needs `canon fmt --check` to make this measurable: a versioned schema +
layout-descriptor registry, and a read-only validator any consumer-repo
corpus (the donor consumer repo's included, or a fixture root) can be checked against.

> **Scope note (2026-07-10, operator directive):** an earlier version of
> this change also specified `canon migrate` (an in-place corpus rewrite
> tool), an in-place migration run against the donor consumer repo's real corpus, and a
> `parity.py` sync patch. **All three are removed from this change's scope.**
> the donor consumer repo will conform its own corpus to canon's format on its own
> schedule and with its own review discipline; a one-shot migration script,
> if one is ever written, is a separate, later, throwaway concern per
> consumer repo — not something this change ships or commits to maintaining.
> This change's test corpus is entirely LOCAL fixtures
> (`crates/canon-fmt/fixtures/consumer-corpus/`, the donor consumer repo-SHAPED data
> adapted into canon's format for testing) — `canon fmt --check` never reads
> a live the donor consumer repo checkout.

## What Changes

- canon-model gains versioned JSON-schemas for the whole artifact family: run/
  review/clear/drill records, divergence manifest/review/remediation events,
  inventory entries, policy, trajectories, strategies, sessions.
- New CLI command `canon fmt --check` validates any corpus against those
  schemas and reports exactly the audited gaps below. It is READ-ONLY: it
  never writes to, or otherwise mutates, the corpus it validates.
- canon-model declares ONE canonical Hive partition grammar
  (`kind=<kind>/[key=value/]*<leaf>`) per artifact kind, with layout
  enforcement generalizing `parity.py`'s existing `_ledger_layout_problem`.
- Schema fields carry the expressiveness upgrades the audit calls for
  (structured `actor`, `refs` arrays, full-length shas, optional
  `change_id`/`task_id`) as additive, optional fields — a legacy-shaped
  record that predates this schema still deserializes and validates against
  the prior required-field set.
- Local fixtures (`crates/canon-fmt/fixtures/consumer-corpus/`) reproduce
  the donor consumer repo's real, audited drift shapes for `canon fmt --check`'s own
  test corpus, without ever reading the donor consumer repo's live checkout.

### Artifact-family audit table (design §5 S11, verbatim)

| Artifact | Today | Gap |
|---|---|---|
| `features/` | plain dirs `<area>/<surface>.feature` | third partition grammar; rationale comment-bound; no authoring provenance (who/when/which session authored a scenario) |
| `inventory/` | FLAT files, convention drifted (`world.yaml` area-level and `world-place-map.yaml` surface-level coexist; README still says `inventory/<area>.yaml`) | no schema envelope, no at/actor, partition key smeared into filenames; `assets.lock` is a fourth ad-hoc format |
| `ledger/` | Hive `kind=/area=` ✓ | run records: no actor/session/cost/duration, `evidence: []` unspecified, no change/task join; review records: **free-text `upstream_ref`** ("reconciled vs upstream @… (see …)"), zero content on success (what was checked); code/design-review: `;`-joined ref strings, not structured arrays |
| `divergences/` | Hive, richest artifact (structured `aspects`, digest anchoring, `ledger_*` back-refs) ✓ | abbreviated `app_sha` in 48 files (validate advisory); back-refs one-way |
| cross-family | — | **no session/actor identity anywhere; no change_id/task_id anywhere**; three partition grammars (Hive / path-dirs / flat-hyphen) |

## Capabilities

### New Capabilities

- `artifact-family-schema`: canon-model's versioned JSON-schemas for the
  artifact family, ONE canonical partition grammar with layout enforcement,
  and the `canon fmt --check` command, run over local fixtures (and
  optionally any consumer-repo corpus a caller points it at).

### Modified Capabilities

_None — canon has no existing specs yet; S11 lands alongside S3/S4/S12 in the
W1 wave._

## Impact

- Extends `canon-model` (S1) with the artifact-family schema set; extends
  `canon-cli` with the `fmt` subcommand, backed by the dedicated
  `canon-fmt` format-authority crate.
- Read-only: this change never writes to any consumer repo, the donor consumer repo
  included — no live corpus is rewritten, and no other repo's tooling
  (`parity.py` or otherwise) is patched by this change.
- New fixture corpus: a small, representative, LOCAL sample reproducing
  the donor consumer repo's real, audited drift shapes, grounded in real records read
  from the donor consumer repo (read-only) but never itself read from the donor consumer repo at
  test time.
- Companion skill (design §5 cross-cutting deliverable, decision 9): a
  `canon fmt --check` authoring skill under `canon/skills/format-authority/`.
