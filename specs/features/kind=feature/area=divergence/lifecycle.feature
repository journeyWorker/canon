Feature: divergence lifecycle
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:divergence-lifecycle
  @divergence.lifecycle.01
  Scenario: Promotion assigns each staged divergence a monotonic order and a refusal consumes none
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given two open candidates staged against two scenarios
    When they are promoted
    Then both are committed carrying the first two run seq values, one each
    And the staging area is empty afterwards

  @subject:divergence-lifecycle
  @divergence.lifecycle.02
  Scenario: A candidate promote refuses consumes no run seq
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given one staged candidate whose actor carries no role and one well-formed candidate beside it
    When they are promoted
    Then the run reports a refusal without grading it a usage error
    And only the well-formed candidate is committed
    And it holds the first run seq, so the refused one burned no position in the order

  @subject:divergence-lifecycle
  @divergence.lifecycle.03
  Scenario: Resolving and deferring commit directly and leave a batch mid-stage alone
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an unrelated candidate a reviewer is still staging
    When one scenario is resolved and another is deferred with a reason and an expiry
    Then both are committed without a promote step
    And the staged candidate is still sitting there untouched, so a routine resolve never promotes somebody else's work

  @subject:divergence-lifecycle
  @divergence.lifecycle.04
  Scenario: A directly committed resolution is what the status view reports
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a scenario resolved directly
    When the divergence status is read
    Then the read exits zero, because it is a capability query and never a gate
    And the current state of that scenario is resolved

  @subject:divergence-lifecycle
  @divergence.lifecycle.05
  Scenario: Run seq alone decides which record is current
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a still-divergent record at a low run seq raised in a high round
    And an open record at a higher run seq raised in a lower round
    When the scenario is folded to its current state
    Then it is open
    And the round the record was raised in moved nothing, because it is not an ordering axis

  @subject:divergence-lifecycle
  @divergence.lifecycle.06
  Scenario: Round breaks a tie only between records sharing a run seq
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a still-divergent record and an open record at the same run seq
    When the scenario is folded to its current state
    Then the higher round wins and the scenario is open

  @subject:divergence-lifecycle
  @divergence.lifecycle.07
  Scenario: A deferral lapses back into still-divergent at its expiry
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a divergence deferred with a reason and an expiry
    When it is read before the expiry
    Then it is deferred, carrying that reason and that expiry
    And read after the expiry it is still-divergent, because time alone resolves nothing

  @subject:divergence-lifecycle
  @divergence.lifecycle.08
  Scenario: A resolution downgrades to resolved-invalid once the app sha moves off what it resolved against
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a divergence whose winning record is resolved against one app sha
    When the scenario's live binding names a different app sha
    Then the folded state is resolved-invalid
    And the record on disk is never rewritten, so the downgrade is derived at read time and not persisted

  @subject:divergence-lifecycle
  @divergence.lifecycle.09
  Scenario: A resolution whose app sha still matches stays resolved
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a divergence whose winning record is resolved against one app sha
    When the scenario's live binding still names that sha
    Then the folded state is resolved
    And the downgrade needs actual evidence of a mismatch, never merely the passage of a read
