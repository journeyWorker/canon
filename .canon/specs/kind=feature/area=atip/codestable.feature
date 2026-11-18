Feature: atip: the replacement table and the upgrade note of the 0.36 error codes
  # canon: {"schema":1,"at":"2026-11-04T13:52:00Z","actor":{"agent_id":"impl-codes-table","role":"implementer","session_id":"s-0-36-codes-table"}}

  # canon: {"schema":1,"at":"2026-11-04T13:52:05Z","actor":{"agent_id":"impl-codes-table","role":"implementer","session_id":"s-0-36-codes-table"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-table.F2
  @atip.codestable.05
  Scenario: Every code of the registry follows the name rule and no list of codes still to rename exists
    Given the registry and the sources of the tests
    When the contract's name rule runs on every code, with its exit and retryable
    Then every code passes, and no test names a list of codes that do not follow the rule yet

  # canon: {"schema":1,"at":"2026-11-04T13:52:06Z","actor":{"agent_id":"impl-codes-table","role":"implementer","session_id":"s-0-36-codes-table"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-table.F2
  @atip.codestable.06
  Scenario: A code that breaks the name rule is found by the registry check
    Given a registry row whose name has no condition, one whose exit is not its condition's, and one whose retryable is not its condition's
    When the registry check runs on them
    Then each is named as breaking the rule
