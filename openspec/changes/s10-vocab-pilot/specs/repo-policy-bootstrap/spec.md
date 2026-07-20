## ADDED Requirements

### Requirement: The repo declares a policy-derived evidence-kind domain
The repo SHALL declare `canon/policy.yaml`'s `trust_required` evidence-kind
domain so canon's typed authoring vocabulary (S10 design.md D4) can
resolve `Type::Evidence`'s `kind` domain for this repo, instead of
resolving an empty one.

#### Scenario: canon-vocab resolves a non-empty evidence-kind domain
- **GIVEN** `canon/policy.yaml` declares `trust_required: { test-run:
  agent, manual-review: human }`
- **WHEN** `canon_vocab::resolve_snapshot` resolves this repo
- **THEN** the resolved snapshot's `evidence_kinds` includes `test-run`
  and `manual-review`

### Requirement: A typed task atom authored against this domain gates for real
A typed task atom SHALL validate, compile to the S1 `Task` model,
round-trip, and gate through `canon gate task` once a matching
`EvidenceRecord` of its declared `evidence.kind` (S10 design.md D2/D4)
exists.

#### Scenario: The pilot task gates and flips on matching typed evidence
- **GIVEN** `openspec/changes/s10-vocab-pilot/tasks.vocab.yaml` declares
  task atom `s10-vocab-pilot#1` with `evidence: { kind: test-run, ref:
  "cargo test -p canon-vocab policy_bridge" }`
- **AND** a matching, non-divergent `EvidenceRecord` of kind `test-run`
  exists in the ledger
- **WHEN** `canon gate task s10-vocab-pilot#1` runs
- **THEN** `tasks.md`'s row `1` flips to `- [x]` with an appended
  evidence note
