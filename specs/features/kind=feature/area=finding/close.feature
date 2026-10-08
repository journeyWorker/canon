Feature: finding close
  # canon: {"schema":1,"at":"2026-08-05T15:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:finding-lifecycle
  @case:happy
  @finding.close.01
  Scenario: A finding raised open is closed by a commit and counted once
  # canon: {"schema":1,"at":"2026-08-05T15:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a committed finding whose disposition is open
    When its disposition is moved to fixed by a commit this repository holds
    Then both the original and the transition are committed records
    And the corpus reports one finding for that key, disposition fixed
    And the superseding record carries the closer as its actor, not the reviewer

  @subject:finding-lifecycle
  @case:happy
  @finding.close.02
  Scenario: Closing changes the disposition and nothing else
  # canon: {"schema":1,"at":"2026-08-05T15:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a committed finding carrying a severity a reviewer a summary and a file ref
    When it is closed
    Then every one of those fields is byte-identical on the superseding record
    And the sourced introducing commit survives unchanged
    And no command offered any way to edit them

  @subject:finding-lifecycle
  @case:failure
  @finding.close.03
  Scenario: A second body that is not a transition is refused
  # canon: {"schema":1,"at":"2026-08-05T15:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a committed finding at a natural key
    When a staged body moves the summary as well as the disposition
    Then promotion refuses it as two records under one identity
    And the refused body is left staged and unmodified
    And the committed corpus is unchanged

  @subject:finding-lifecycle
  @case:edge
  @finding.close.04
  Scenario: Closing to the disposition already recorded stages nothing
  # canon: {"schema":1,"at":"2026-08-05T15:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a finding whose current disposition is already fixed
    When it is closed to fixed again
    Then the command succeeds and reports a no-op
    And nothing is staged, because an identical disposition carries no new fact

  @subject:finding-lifecycle
  @case:happy
  @finding.close.05
  Scenario: A fix that did not hold reopens the finding
  # canon: {"schema":1,"at":"2026-08-05T15:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a committed finding whose current disposition is fixed
    When it is closed to open
    Then the transition is admitted
    And the corpus reports that finding open again

  @subject:finding-lifecycle
  @case:edge
  @finding.close.06
  Scenario: The current disposition is the latest version never the first read
  # canon: {"schema":1,"at":"2026-08-05T15:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a natural key carrying an open record and a later fixed record
    When the finding is read to decide what closing it would change
    Then the fixed record is what answers, through the shared supersession fold
    And a resubmission of the current disposition is refused rather than appended

  @subject:finding-lifecycle
  @case:failure
  @finding.close.07
  Scenario: Closing refuses a finding that was never committed
  # canon: {"schema":1,"at":"2026-08-05T15:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a finding that is staged but not yet promoted
    When it is closed
    Then the command refuses, naming that there is no recorded disposition to move
    And the staged original is left exactly as it was

  @subject:finding-lifecycle
  @case:failure
  @finding.close.08
  Scenario: A fixed finding must name a commit this repository holds
  # canon: {"schema":1,"at":"2026-08-05T15:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a committed finding
    When it is closed to fixed with no resolution sha, or with one this repo does not hold
    Then the command refuses before reading the corpus
    And a disposition other than fixed carrying a resolution sha is refused too
    And nothing is staged in either case
