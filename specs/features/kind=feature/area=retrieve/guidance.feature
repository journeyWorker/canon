Feature: retrieve guidance
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:retrieve-guidance
  @case:happy
  @retrieve.guidance.01
  Scenario: Guidance is served scoped to one role and one regime
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a strategy store holding one promoted strategy for a role under a regime key
    When guidance is retrieved for that exact role and regime
    Then the command exits zero and reports one guidance item
    And the item's title is the seeded strategy's, in both the human and the machine-readable shape

  @subject:retrieve-guidance
  @case:edge
  @retrieve.guidance.02
  Scenario: A result bound caps how much guidance is served
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given three strategies seeded under one role and regime
    When guidance is retrieved with the bound set to two
    Then exactly two items are served
    And the bound is enforced by the command itself, not left to the caller to trim

  @subject:retrieve-guidance
  @case:failure
  @retrieve.guidance.03
  Scenario: A malformed regime key is refused against the four-segment grammar
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given the regime grammar of a role, a repo, an area and a twelve-hex-character hash
    When a value with only three segments or a non-hex hash is parsed
    Then the parse is refused rather than accepted as a key nothing will ever match
    And passing such a value to retrieval exits nonzero naming the regime flag, never panicking

  @subject:retrieve-guidance
  @case:failure
  @retrieve.guidance.04
  Scenario: A role that does not lead its own regime key is a usage error
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a well-formed regime key whose leading role segment is one role
    When guidance is retrieved for a different role against that key
    Then the command exits with the usage code
    And the message says the two do not match, rather than silently serving the wrong scope

  @subject:retrieve-guidance
  @case:edge
  @retrieve.guidance.05
  Scenario: A repository with no strategy memory yet reports an explicit empty result
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository with no strategy store on disk at all
    When guidance is retrieved
    Then the command exits zero and states that it served zero items
    And a repository that has learned nothing is not an error, it is an empty answer

  @subject:retrieve-guidance
  @case:edge
  @retrieve.guidance.06
  Scenario: A recorded manifest replays its guidance verbatim after the live store moves on
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a run manifest that recorded one strategy as its injected guidance at dispatch time
    When that strategy is demoted afterwards
    Then a fresh live retrieval returns nothing, because the demotion took effect
    And replaying the manifest still returns the recorded snapshot, unchanged and still one item
    And the demotion never perturbs a manifest already written
