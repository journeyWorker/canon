Feature: atip: error code names, <SUBJECT>_<CONDITION>
  # canon: {"schema":1,"at":"2026-11-04T01:32:51Z","actor":{"agent_id":"impl-codes-rename","role":"implementer","session_id":"s-0-36-codes-rename"}}

  # canon: {"schema":1,"at":"2026-11-04T01:32:51Z","actor":{"agent_id":"impl-codes-rename","role":"implementer","session_id":"s-0-36-codes-rename"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-rename.F1
  @atip.codenames.01
  Scenario: A call that raised an old name raises the new one with the same exit, retryable, field and reason
    Given a repository and the call that raised an old name of the brief's table, each with the fields the old name carried
    When the call runs again with --json
    Then it fails with the new code, the old exit, retryable false, the same field, and the reason where the table gives one

  # canon: {"schema":1,"at":"2026-11-04T01:32:59Z","actor":{"agent_id":"impl-codes-rename","role":"implementer","session_id":"s-0-36-codes-rename"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-rename.F2
  @atip.codenames.03
  Scenario: The renamed codes and the kept ones follow the name rule, and the renamed ones have the exit and reason of the table
    Given the registry of the renamed codes and of the kept ones
    When the contract's name rule runs on each code, with its exit and retryable
    Then each follows the rule, and each renamed code has the exit, retryable false and reason of the brief's table

  # canon: {"schema":1,"at":"2026-11-04T01:33:03Z","actor":{"agent_id":"impl-codes-rename","role":"implementer","session_id":"s-0-36-codes-rename"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-rename.F2
  @atip.codenames.04
  Scenario: A name that breaks the rule fails the registry test, and no list exempts a code
    Given a name that does not end in a condition, a name with the wrong exit, and the registry with no list of codes that do not follow the rule yet
    When the registry test and the name rule run
    Then the name without a condition or with the wrong exit is refused, and every code of the registry follows the rule with nothing exempt
