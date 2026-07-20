## MODIFIED Requirements

### Requirement: canon.yaml declares tier routing and aging rules by capability rung
`canon.yaml` SHALL support a `routing` map from record kind to
CAPABILITY RUNG (`local`/`hot`/`cold` — see `tier-role-backend-
split`'s `Rung` capability, which this requirement now depends on)
and an `aging` map from record kind to an aging rule (`after`
duration, `to` destination RUNG); `canon-store` SHALL resolve every
write and every `canon tier age` run from this declarative policy,
never a hardcoded per-kind branch. This supersedes this capability's
original shape, where `routing`/`aging.*.to` named a BACKEND (`git`/
`pg`/`r2`) directly — the routing/aging vocabulary is now rung-only;
the backend a rung resolves to is declared separately in `canon.yaml`'s
`tiers.<rung>.backend` tag.

#### Scenario: Routing determines write destination by rung
- **WHEN** a record of kind `evidence_record` is written and
  `canon.yaml`'s `routing.evidence_record` is `local`
- **THEN** the write lands in whichever backend `tiers.local.
  backend` configures (git, by default convention), with no code-level
  branch on the literal kind name outside the policy resolution step

#### Scenario: A routing change moves future writes without a code change
- **WHEN** `canon.yaml`'s `routing.handoff` changes from `hot` to a
  different rung
- **THEN** subsequent writes of `handoff` records land in the newly
  configured rung's backend with no `canon-store` source change
  required

#### Scenario: Changing a rung's backend moves future writes without a routing change
- **WHEN** `canon.yaml`'s `tiers.hot.backend` changes from `postgres`
  to a different backend, while `routing.task` still names `hot`
- **THEN** subsequent writes of `task` records land in the newly
  configured backend, with no `routing`/`aging` entry needing to
  change — the rung a kind routes to and the backend implementing
  that rung are independently configurable

### Requirement: `canon tier age` applies aging rules with digest idempotence, moving between rungs
`canon tier age` SHALL move records whose age exceeds their
`TierPolicy` threshold from their current RUNG to the configured
destination RUNG, using a content digest to detect and skip
already-aged records.

#### Scenario: A record past its aging threshold moves rungs
- **WHEN** `canon tier age` runs and a `handoff` record's `at`
  timestamp is older than `aging.handoff.after`
- **THEN** the record is written to `aging.handoff.to`'s rung (via
  that rung's configured backend) and removed from its prior rung's
  backend in the same `canon tier age` run

#### Scenario: A record within its aging threshold is left untouched
- **WHEN** `canon tier age` runs and a record's age is within its
  configured threshold
- **THEN** the record remains in its current rung and is not written
  to the destination rung
