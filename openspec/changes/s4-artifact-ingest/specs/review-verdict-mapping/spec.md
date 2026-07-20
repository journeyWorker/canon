## ADDED Requirements

### Requirement: Review-to-verdict mapping table is enforced verbatim
`canon-ingest` SHALL derive a verdict `{role, polarity, becomes}` from each
normalized artifact event according to exactly the following table (design
§5 S4, verbatim):

| Input artifact | Role | Polarity | Becomes |
|---|---|---|---|
| code-review finding (open/still-divergent) | dev | failure | guardrail candidate |
| design-review finding | design | failure | guardrail candidate |
| review-record (promotion to @reviewed) | authoring role | success | strategy candidate |
| clear-record after @flagged | review | corrective | guardrail (what the sample caught) |
| remediation + later `resolved` | dev | success | strategy candidate |
| CI fail / PR revert | dev | failure | guardrail candidate |
| PR merge (no revert window) | dev | success | strategy candidate |

#### Scenario: Open code-review finding becomes a dev guardrail candidate
- **WHEN** the ledger adapter normalizes a `kind=code-review` record whose
  `verdict` field is not `faithful` (an open/still-divergent finding)
- **THEN** the emitted verdict has `role=dev`, `polarity=failure`,
  `becomes=guardrail candidate`

#### Scenario: Review-record promotion becomes a strategy candidate
- **WHEN** the ledger adapter normalizes a `kind=review` record marking a
  scenario's promotion to `@reviewed`
- **THEN** the emitted verdict has `role=<authoring role of the scenario>`,
  `polarity=success`, `becomes=strategy candidate`

#### Scenario: Clear-record after @flagged becomes a corrective guardrail
- **WHEN** the ledger adapter normalizes a `kind=clear` record clearing a
  previously `@flagged` scenario
- **THEN** the emitted verdict has `role=review`, `polarity=corrective`,
  `becomes=guardrail (what the sample caught)`

#### Scenario: PR merge with no revert window becomes a dev success
- **WHEN** the openspec/handoff-joined event stream observes a PR merge with
  no subsequent revert recorded within the configured revert window
- **THEN** the emitted verdict has `role=dev`, `polarity=success`,
  `becomes=strategy candidate`

#### Scenario: CI fail or PR revert becomes a dev failure
- **WHEN** the event stream observes a CI failure or a PR revert
- **THEN** the emitted verdict has `role=dev`, `polarity=failure`,
  `becomes=guardrail candidate`

### Requirement: Verdicts carry a table-driven regime key
Every emitted verdict SHALL carry a `regime_key` in the `<role>/<repo>/
<area>/<hash>` grammar (S1), derived by the same shared function every
adapter uses, from the source event's severity/area tags.

#### Scenario: Two verdicts from the same area and role share a regime key prefix
- **WHEN** two verdicts are derived from events tagged with the same `role`,
  `repo`, and `area`
- **THEN** both verdicts' `regime_key` values share the identical
  `<role>/<repo>/<area>/` prefix

### Requirement: Golden-file verdict stream from a frozen fixture corpus
A fixed, checked-in ledger + divergence fixture corpus SHALL produce an
exact, checked-in verdict-stream output, diffed byte-for-byte by `canon
selftest` — captured from the donor consumer repo as a point-in-time snapshot, never a
live read.

#### Scenario: Fixture corpus reproduces the golden verdict stream
- **WHEN** `canon selftest` runs the S4 fixture corpus through the
  artifact-ingest adapters and verdict mapping
- **THEN** the produced verdict stream is byte-identical to the checked-in
  golden file, and re-running ingest over the same corpus produces no
  additional or duplicate verdicts (idempotent re-ingest)
