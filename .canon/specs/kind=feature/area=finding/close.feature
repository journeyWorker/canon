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
    And the disposition, the resolution sha and the optional --reason are all a close adds

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

  # canon: {"schema":1,"at":"2026-10-10T16:02:37Z","actor":{"agent_id":"canon"}}
  @subject:finding-lifecycle
  @case:happy
  @finding.close.09
  Scenario: A close records the session that authored the transition, not the raiser's
    Given a finding raised with --session-id sess-raise and promoted
    When it is closed with --session-id sess-close and promoted
    Then the raised version keeps sess-raise and the transition carries sess-close

  # canon: {"schema":1,"at":"2026-10-10T19:28:02Z","actor":{"agent_id":"canon"}}
  @subject:finding-lifecycle
  @case:happy
  @finding.close.80
  Scenario: Closing with a short resolution sha records the full commit
    Given a committed open finding and the commit that fixed it
    When canon finding close --disposition fixed names that commit by a short --resolution-sha
    Then the staged transition records the full sha git resolved it to
    And a short sha that names no commit is refused with exit 2

  # canon: {"schema":1,"at":"2026-10-21T09:00:02Z","actor":{"agent_id":"canon-impl-0241"}}
  @subject:finding-lifecycle
  @case:happy
  @finding.close.81
  Scenario: A close carries the scenarios the finding names and takes no list of its own
    Given a committed blocker raised with --scenario-id naming one of a subject's two evidenced scenarios
    When it is closed fixed with canon finding close, which has no --scenario-id
    Then the closed version carries the same scenario ids as the version it closes
    And the subject's ship refusal names that scenario only, until evidence for it is recorded after the close

  # canon: {"schema":1,"at":"2026-10-21T11:47:25Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:finding-lifecycle
  @case:failure
  @finding.close.82
  Scenario: Closing rejected or deferred without a reason is refused
    Given a committed open finding
    When it is closed rejected, or deferred, with no --reason
    Then the command refuses with FINDING_REASON_REQUIRED and exit 2, its hint giving the exact command shape
    And nothing is staged or committed

  # canon: {"schema":1,"at":"2026-10-21T11:47:25Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:finding-lifecycle
  @case:happy
  @finding.close.83
  Scenario: A rejected or deferred close records its reason on the closing version
    Given a committed open finding
    When it is closed rejected, or deferred, with --reason "works as designed"
    Then the closing version carries that reason and the raised version stays without one
    And canon finding list shows the reason for that finding

  # canon: {"schema":1,"at":"2026-10-21T11:47:25Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:finding-lifecycle
  @case:happy
  @finding.close.84
  Scenario: A fixed close takes a reason but does not need one
    Given a committed open finding and the commit that fixed it
    When it is closed fixed with and without --reason
    Then both closes are admitted
    And the reason is recorded only when it was given

  # canon: {"schema":1,"at":"2026-10-21T11:47:25Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:finding-lifecycle
  @case:failure
  @finding.close.85
  Scenario: A blank or multi-line reason is refused like a missing one
    Given a committed open finding
    When it is closed with a blank --reason, or one holding a line break
    Then the command refuses with FINDING_REASON_REQUIRED, whatever the disposition
    And nothing is staged or committed

  # canon: {"schema":1,"at":"2026-10-21T11:47:25Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:finding-lifecycle
  @case:happy
  @finding.close.86
  Scenario: A staged close carrying a reason promotes under write.promote manual
    Given write.promote is manual and a committed open finding
    When it is closed rejected with --reason and canon gate promote then runs
    Then the staged close is promoted with its reason
    And a staged body that changes the reason without moving the disposition is still refused

  # canon: {"schema":1,"at":"2026-10-21T11:47:25Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:finding-lifecycle
  @case:edge
  @finding.close.87
  Scenario: A finding written before reasons existed reads, lists and closes unchanged
    Given a finding record committed before reasons existed, with no reason field
    When it is read, listed and closed fixed
    Then the untouched record's JSON is byte-identical to what was committed
    And the list row shows no reason for it
