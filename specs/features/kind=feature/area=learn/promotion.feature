Feature: learn promotion
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:learn-promotion
  @learn.promotion.01
  Scenario: A proven strategy graduates into a git-tracked file
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a distilled strategy and a regime carrying enough corroborating successes to clear its gate
    When that strategy is promoted
    Then the command succeeds and a file appears in the git-tracked strategies tier
    And the strategy has moved from derived memory into something a person can review in a diff

  @subject:learn-promotion
  @learn.promotion.02
  Scenario: The promoted file opens active and carries the strategy's own content
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a distilled strategy authored under a role
    When it is promoted into a git-tier root
    Then the written path is scoped by that role and named by the strategy id
    And the file opens with front matter whose status is active
    And the body carries the strategy's content, not a summary of it

  @subject:learn-promotion
  @learn.promotion.03
  Scenario: A dry run previews the promotion and writes nothing
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a strategy that would otherwise pass its gate
    When promotion is run as a dry run
    Then the command succeeds and shows what it would write
    And no file is created in the git-tracked tier

  @subject:learn-promotion
  @learn.promotion.04
  Scenario: A strategy with no corroborating trajectories is blocked by the gate
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a distilled strategy whose regime holds no resolved trajectories at all
    When it is promoted
    Then the command refuses
    And the strategy stays out of the git-tracked tier, because nothing corroborates it

  @subject:learn-promotion
  @learn.promotion.05
  Scenario: Unresolved trajectories never count toward a promotion
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a regime carrying more pending trajectories than the gate's minimum
    When the strategy distilled from them is promoted
    Then the command still refuses
    And volume of unresolved evidence is not evidence

  @subject:learn-promotion
  @learn.promotion.06
  Scenario: A later contradiction resets the streak and blocks the promotion
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a regime with enough successes to clear the gate, followed by one failure
    When the strategy is promoted
    Then the command refuses
    And the gate counts the streak up to the contradiction, not the total number of successes

  @subject:learn-promotion
  @learn.promotion.07
  Scenario: A role that configures no promotion gate gets the conservative occurrence default
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository whose configuration names no promotion entry for a role
    When that role's promotion gate is resolved
    Then the mode is occurrence
    And the minimum sample count and the window come from the conservative defaults, never left unset

  @subject:learn-promotion
  @learn.promotion.08
  Scenario: Gate mode is chosen per role between crn and occurrence
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a configuration setting one role to occurrence with its own minimum and window, and another role to crn
    When both roles' gates are resolved
    Then the first role's mode is occurrence and carries exactly the configured minimum and window
    And the second role's mode is crn
    And the crn role's occurrence fields still hold well-formed defaults, because a mode change never leaves a field uninitialized

  @subject:learn-promotion
  @learn.promotion.09
  Scenario: Demoting soft-flags the promoted file and leaves the rest of it intact
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a strategy already promoted into the git-tracked tier
    When it is demoted under the default policy
    Then its front matter carries a demoted status
    And the front matter it already carried survives unchanged
    And the body survives too, because a demotion records a judgement rather than erasing the strategy
