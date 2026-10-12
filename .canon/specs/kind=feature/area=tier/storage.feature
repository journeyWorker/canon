Feature: tier storage
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:tier-storage
  @case:happy
  @tier.storage.01
  Scenario: Each rung admits only the class of backend that rung is for
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a configuration declaring a local, a hot, and a cold rung
    When each rung is paired with the backend its class calls for
    Then local on git, hot on postgres, and cold on an object store all parse
    And each rung resolves to exactly the backend it declared

  @subject:tier-storage
  @case:failure
  @tier.storage.02
  Scenario: A rung pointed at the wrong class of backend is refused with the pairing named
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a cold rung declared on a live-database backend
    When the policy is parsed
    Then it is refused rather than accepted and left to fail at read time
    And the error names the rung, the backend it was given, and the class of backend that rung expects

  @subject:tier-storage
  @case:happy
  @tier.storage.03
  Scenario: Asking what aging would do changes nothing
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given one record past its aging threshold and one still within it
    When aging is run as a dry run
    Then it reports one candidate, naming the kind, the rungs it would cross, and the threshold
    And both records are still in the source rung, and nothing was written to the destination

  @subject:tier-storage
  @case:happy
  @tier.storage.04
  Scenario: Aging moves what is past the threshold, leaves what is not, and settles
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given one record past its aging threshold and one still within it
    When aging is run for real
    Then it reports one moved and none already aged
    And the aged record has left the source rung and landed on the destination
    And the within-threshold record is untouched
    And running it again moves nothing and duplicates nothing

  @subject:tier-storage
  @case:happy
  @tier.storage.05
  Scenario: A kind whose records straddle two rungs reads as one merged history
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a kind routed to one rung with an aging destination on another, holding one record each
    When that kind is queried
    Then both records are returned, from the routed rung and the aging destination alike
    And they come back ordered by their recorded instant, not by which rung answered first
    And the read reports no layout violation

  @subject:tier-storage
  @case:failure
  @tier.storage.06
  Scenario: A kind routed to a rung that cannot be reached fails by name rather than reading empty
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a kind routed to a hot rung whose database is not reachable
    When that kind is queried
    Then the command fails instead of returning an empty result
    And the error names the rung, the backend, and that there is no live connection

  @subject:tier-storage
  @case:edge
  @tier.storage.07
  Scenario: A rung this read does not need never blocks it
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a configuration declaring local, hot and cold rungs with neither hot nor cold reachable
    When a kind routed to the local rung is queried
    Then the query succeeds and returns that kind's records
    And no credential for an unrelated rung was ever required

  @subject:tier-storage
  @case:edge
  @tier.storage.08
  Scenario: One natural key reads as one current record, the one recorded latest
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given two records under one natural key recorded at different instants
    When they are folded to what is current
    Then exactly one record answers for that key
    And it is the one with the later recorded instant, whatever its digest sorts as

  @subject:tier-storage
  @case:edge
  @tier.storage.09
  Scenario: Two generations sharing an instant resolve by format generation, never by digest luck
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given two records under one natural key carrying the same recorded instant
    And the stale one carries the greater digest while the fresh one carries the greater schema
    When they are folded
    Then the record of the greater schema wins
    And the digest never overrules a newer format generation, so the winner is stated by the data rather than decided by a hash
