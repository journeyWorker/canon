Feature: finding closer
  # canon: {"schema":1,"at":"2026-11-06T04:57:05Z","actor":{"agent_id":"impl-finding-closer","session_id":"s-0-38-finding-closer"}}

  # canon: {"schema":1,"at":"2026-11-06T04:57:05Z","actor":{"agent_id":"impl-finding-closer","session_id":"s-0-38-finding-closer"}}
  @subject:finding-lifecycle
  @case:happy
  @flow:canon-0-38-finding-closer.F1
  @finding.closer.01
  Scenario: An implementer or orchestrator closes a finding fixed and the record carries that role
    Given a committed open finding raised by a reviewer
    When a call that declares the implementer role, then one that declares the orchestrator role, closes it with --disposition fixed
    Then the close is recorded and the closing version's actor role is the declared role
    And the actor is the caller, and the reviewer who raised the finding is unchanged

  # canon: {"schema":1,"at":"2026-11-06T04:57:16Z","actor":{"agent_id":"impl-finding-closer","session_id":"s-0-38-finding-closer"}}
  @subject:finding-lifecycle
  @case:failure
  @flow:canon-0-38-finding-closer.F1
  @finding.closer.02
  Scenario: A role declared with --as or CANON_ROLE and a different --role is refused and writes nothing
    Given a committed open finding and a call that declares the implementer role with --as or CANON_ROLE
    When it passes --role reviewer
    Then the call is refused as FINDING_ARGUMENT_INVALID naming --role, with exit 2
    And no record is staged or committed and the finding stays open

  # canon: {"schema":1,"at":"2026-11-06T04:57:22Z","actor":{"agent_id":"impl-finding-closer","session_id":"s-0-38-finding-closer"}}
  @subject:finding-lifecycle
  @case:failure
  @flow:canon-0-38-finding-closer.F2
  @finding.closer.03
  Scenario: A call under a role other than reviewer cannot close a finding rejected or deferred
    Given a committed open blocker finding
    When a call that declares the implementer, the orchestrator or the planner role closes it with --disposition rejected, then with --disposition deferred, each with a reason
    Then each is refused with FINDING_ROLE_DENIED, exit 2, not retryable
    And the hint says a reviewer closes it and how to pick the role

  # canon: {"schema":1,"at":"2026-11-06T04:57:28Z","actor":{"agent_id":"impl-finding-closer","session_id":"s-0-38-finding-closer"}}
  @subject:finding-lifecycle
  @case:failure
  @flow:canon-0-38-finding-closer.F2
  @finding.closer.04
  Scenario: After the refusal the finding is still open and its blocker still blocks the gate
    Given a committed open blocker finding on a change a subject adopts, and a refused rejected close from an implementer
    When the finding list and the gate are read
    Then the finding is still open, nothing was staged, and the blocker still blocks the gate

  # canon: {"schema":1,"at":"2026-11-06T04:57:34Z","actor":{"agent_id":"impl-finding-closer","session_id":"s-0-38-finding-closer"}}
  @subject:finding-lifecycle
  @case:happy
  @flow:canon-0-38-finding-closer.F3
  @finding.closer.05
  Scenario: A reviewer closes a finding fixed, rejected or deferred and the record says reviewer
    Given a committed open finding
    When a call that declares the reviewer role closes it fixed, another rejected with a reason, another deferred with a reason
    Then each is closed and the closing version's actor role is reviewer

  # canon: {"schema":1,"at":"2026-11-06T04:57:39Z","actor":{"agent_id":"impl-finding-closer","session_id":"s-0-38-finding-closer"}}
  @subject:finding-lifecycle
  @case:failure
  @flow:canon-0-38-finding-closer.F3
  @finding.closer.06
  Scenario: A reviewer closing rejected without a reason is still refused
    Given a committed open finding
    When a reviewer closes it rejected without --reason
    Then the call is refused as FINDING_REASON_REQUIRED, exit 2, as before this change
    And nothing is recorded

  # canon: {"schema":1,"at":"2026-11-06T04:57:52Z","actor":{"agent_id":"impl-finding-closer","session_id":"s-0-38-finding-closer"}}
  @subject:finding-lifecycle
  @case:happy
  @flow:canon-0-38-finding-closer.F4
  @finding.closer.07
  Scenario: A call that declares no role closes a finding and the record carries no role
    Given a committed open finding and a call that declares no role and passes no --role
    When it closes the finding with any of the three dispositions
    Then the close is recorded and the closing version has no role, never reviewer

  # canon: {"schema":1,"at":"2026-11-06T04:58:07Z","actor":{"agent_id":"impl-finding-closer","session_id":"s-0-38-finding-closer"}}
  @subject:finding-lifecycle
  @case:failure
  @flow:canon-0-38-finding-closer.F4
  @finding.closer.08
  Scenario: Before a finding is closed the list names no closer
    Given a committed open finding
    When canon finding list reads it before any close
    Then the row has no closer and no closer role, in JSON and in the table

  # canon: {"schema":1,"at":"2026-11-06T04:58:20Z","actor":{"agent_id":"impl-finding-closer","session_id":"s-0-38-finding-closer"}}
  @subject:finding-lifecycle
  @case:happy
  @flow:canon-0-38-finding-closer.F5
  @finding.closer.09
  Scenario: The list names who closed each closed finding and under which role
    Given findings closed by a reviewer, by an implementer and by a call with no role
    When canon finding list runs, in JSON and as a table
    Then each closed finding names the actor that closed it and the role, or none, and an open one names neither

  # canon: {"schema":1,"at":"2026-11-06T04:58:26Z","actor":{"agent_id":"impl-finding-closer","session_id":"s-0-38-finding-closer"}}
  @subject:finding-lifecycle
  @case:failure
  @flow:canon-0-38-finding-closer.F5
  @finding.closer.10
  Scenario: A finding closed by 0.37 lists without an error as written
    Given a finding closed by canon 0.37, whose closing version has the actor canon and the role reviewer
    When canon finding list reads it
    Then it lists without an error, naming canon and reviewer as written
    And the old record's bytes are not rewritten

  # canon: {"schema":1,"at":"2026-11-06T04:58:39Z","actor":{"agent_id":"impl-finding-closer","session_id":"s-0-38-finding-closer"}}
  @subject:finding-lifecycle
  @case:happy
  @flow:canon-0-38-finding-closer.F6
  @finding.closer.11
  Scenario: The CHANGELOG names the new refusal and the recorded role and the guides say who closes
    Given the CHANGELOG of this change
    When its Unreleased section is read
    Then Breaking names the new refusal of rejected and deferred by a role other than reviewer, and Changed names the role the close records
    And it says the implementer guide has no finding close and the reviewer guide has

  # canon: {"schema":1,"at":"2026-11-06T04:59:02Z","actor":{"agent_id":"impl-finding-closer","session_id":"s-0-38-finding-closer"}}
  @subject:finding-lifecycle
  @case:failure
  @flow:canon-0-38-finding-closer.F6
  @finding.closer.12
  Scenario: A CHANGELOG without the entry fails the release check
    Given a CHANGELOG whose Unreleased section lacks either entry
    When the release check runs over it
    Then the check fails and names the missing entry
