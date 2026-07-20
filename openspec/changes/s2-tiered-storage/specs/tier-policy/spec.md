## ADDED Requirements

### Requirement: canon.yaml declares tier routing and aging rules
`canon.yaml` SHALL support a `routing` map from record kind to tier and an
`aging` map from record kind to an aging rule (`after` duration, `to`
destination tier); `canon-store` SHALL resolve every write and every
`canon tier age` run from this declarative policy, never a hardcoded
per-kind branch.

#### Scenario: Routing determines write destination
- **WHEN** a record of kind `evidence-record` is written and
  `canon.yaml`'s `routing.evidence-record` is `git`
- **THEN** the write lands in the git tier's Hive-partitioned layout, with
  no code-level branch on the literal kind name outside the policy
  resolution step.

#### Scenario: A routing change moves future writes without a code change
- **WHEN** `canon.yaml`'s `routing.handoff` changes from `pg` to a
  different tier
- **THEN** subsequent writes of `handoff` records land in the newly
  configured tier with no `canon-store` source change required.

### Requirement: `canon tier age` applies aging rules with digest idempotence
`canon tier age` SHALL move records whose age exceeds their `TierPolicy`
threshold from their current tier to the configured destination tier,
using a content digest to detect and skip already-aged records.

#### Scenario: A record past its aging threshold moves tiers
- **WHEN** `canon tier age` runs and a `handoff` record's `at` timestamp is
  older than `aging.handoff.after`
- **THEN** the record is written to `aging.handoff.to`'s tier and removed
  from its prior tier in the same `canon tier age` run.

#### Scenario: A record within its aging threshold is left untouched
- **WHEN** `canon tier age` runs and a record's age is within its
  configured threshold
- **THEN** the record remains in its current tier and is not written to
  the destination tier.
