Feature: finding list
  # canon: {"schema":1,"at":"2026-10-12T14:00:00Z","actor":{"agent_id":"canon"}}

  # canon: {"schema":1,"at":"2026-10-12T14:00:00Z","actor":{"agent_id":"canon"}}
  @subject:finding-lifecycle
  @case:happy
  @finding.list.01
  Scenario: A change's committed findings list one row each at their latest version
    Given a change with a blocker raised open and later closed rejected, an open should-fix, a staged finding, and a finding on another change
    When canon finding list --change-id runs for that change
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
    And a change with no open findings prints that it has none and exits 0

  # canon: {"schema":1,"at":"2026-10-12T14:00:00Z","actor":{"agent_id":"canon"}}
  @subject:finding-lifecycle
  @case:happy
  @finding.list.03
  Scenario: --json prints the same rows as one JSON document
    Given the same findings
    When canon finding list runs with --json, alone and with --open
    Then stdout is one JSON document with the change id and a findings array carrying round, seq, severity, disposition, reviewer, summary, resolution_sha and file_ref
    And with --open the array holds only the open finding

  # canon: {"schema":1,"at":"2026-10-12T14:00:00Z","actor":{"agent_id":"canon"}}
  @subject:finding-lifecycle
  @case:failure
  @finding.list.04
  Scenario: A malformed change id is refused before anything is read
    Given a repo with a canon.yaml
    When canon finding list runs with a change id outside the grammar
    Then it exits 2 naming --change-id and prints nothing to stdout
