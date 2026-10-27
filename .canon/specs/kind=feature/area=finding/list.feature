Feature: finding list
  # canon: {"schema":1,"at":"2026-10-12T14:00:00Z","actor":{"agent_id":"canon"}}

  # canon: {"schema":1,"at":"2026-10-12T14:00:00Z","actor":{"agent_id":"canon"}}
  @subject:finding-lifecycle
  @case:happy
  @finding.list.01
  Scenario: A change's committed findings list one row each at their latest version
    Given a change with a blocker raised open and later closed rejected, an open should-fix, a staged finding, and a finding on another change
    When canon finding list --change runs for that change
    Then it prints a header and one row per committed finding with round, seq, severity, disposition, reviewer and summary, ordered by round and seq
    And the closed finding shows rejected, its latest disposition
    And the other change's finding is not listed
    And the staged finding is not listed but named on stderr with canon gate promote

  # canon: {"schema":1,"at":"2026-10-12T14:00:00Z","actor":{"agent_id":"canon"}}
  @subject:finding-lifecycle
  @case:happy
  @finding.list.02
  Scenario: --open lists only the findings whose current disposition is open
    Given the same findings
    When canon finding list runs with --open
    Then only the open should-fix is listed, never the finding that was raised open and closed
    And a change whose only committed finding was raised open and later closed prints that it has no open findings and exits 0

  # canon: {"schema":1,"at":"2026-10-12T14:00:00Z","actor":{"agent_id":"canon"}}
  @subject:finding-lifecycle
  @case:happy
  @finding.list.03
  Scenario: --json prints the same rows in the standard envelope
    Given the same findings
    When canon finding list runs with --json, alone and with --open
    Then data is one object with the change id and a findings array carrying round, seq, severity, disposition, reviewer, summary, resolutionSha, fileRef and reason, and meta carries the paging counts
    And with --open the array holds only the open finding

  # canon: {"schema":1,"at":"2026-10-12T14:00:00Z","actor":{"agent_id":"canon"}}
  @subject:finding-lifecycle
  @case:failure
  @finding.list.04
  Scenario: A malformed change id is refused before anything is read
    Given a repo with a canon.yaml
    When canon finding list runs with a change id outside the grammar
    Then it exits 2 naming --change and prints nothing to stdout

  # canon: {"schema":1,"at":"2026-10-21T11:47:25Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:finding-lifecycle
  @case:happy
  @finding.list.05
  Scenario: finding list shows each closed finding's reason in the table and in JSON
    Given a rejected finding closed with a reason and a fixed one closed without
    When canon finding list runs, and again with --json
    Then the table appends the reason to the summary of the finding that has one, leaving the columns as they were
    And the JSON rows carry a reason field, null for the finding without one
