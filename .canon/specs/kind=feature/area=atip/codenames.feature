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

  # canon: {"schema":1,"at":"2026-11-04T01:32:55Z","actor":{"agent_id":"impl-codes-rename","role":"implementer","session_id":"s-0-36-codes-rename"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-rename.F1
  @atip.codenames.02
  Scenario: An old name is in no registry view, in no command's errors and in no source, and a source that builds one does not match the registry
    Given the registry, every command's list of errors and the sources of canon
    When the registry views are read and the sources are matched against the registry
    Then no old name is in a view, a source that builds one has no registry row, and the match test fails for it

  # canon: {"schema":1,"at":"2026-11-04T01:32:59Z","actor":{"agent_id":"impl-codes-rename","role":"implementer","session_id":"s-0-36-codes-rename"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-rename.F2
  @atip.codenames.03
  Scenario: The renamed codes and the kept ones follow the name rule, and the renamed ones have the exit and reason of the table
    Given the registry of the renamed codes and of the 48 kept ones
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

  # canon: {"schema":1,"at":"2026-11-04T01:33:07Z","actor":{"agent_id":"impl-codes-rename","role":"implementer","session_id":"s-0-36-codes-rename"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-rename.F3
  @atip.codenames.05
  Scenario: schema --error refuses each old name as an unknown code
    Given each old name of the brief's table
    When canon schema --error is asked for it
    Then it fails with USAGE, field --error, exit 2, the answer an unknown code gets, and no word about a rename

  # canon: {"schema":1,"at":"2026-11-04T01:33:14Z","actor":{"agent_id":"impl-codes-rename","role":"implementer","session_id":"s-0-36-codes-rename"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-rename.F4
  @atip.codenames.06
  Scenario: No guide, page, dashboard source, eval script, design document or scenario spells a renamed code
    Given the guides, the website pages, the dashboard sources, the eval scripts, the design documents and the scenarios
    When each is scanned for a renamed code
    Then none spells one

  # canon: {"schema":1,"at":"2026-11-04T01:33:19Z","actor":{"agent_id":"impl-codes-rename","role":"implementer","session_id":"s-0-36-codes-rename"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-rename.F4
  @atip.codenames.07
  Scenario: A consumer that says a code canon does not register, or revives an old name, fails the test
    Given a consumer text that says a code canon does not register, one that revives an old name, and one that says a registered code
    When the consumer scan runs on each
    Then the first two are found and fail the test, the third passes, and the real consumers say no unregistered code

  # canon: {"schema":1,"at":"2026-11-04T01:33:24Z","actor":{"agent_id":"impl-codes-rename","role":"implementer","session_id":"s-0-36-codes-rename"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-rename.F5
  @atip.codenames.08
  Scenario: The CHANGELOG holds a row for each code that left the registry
    Given the CHANGELOG's Unreleased section and the code names of the 0.35 registry
    When each code that left the registry is looked up in the replacement table
    Then every one has a row with its new name, and every new name in the table is registered

  # canon: {"schema":1,"at":"2026-11-04T01:33:29Z","actor":{"agent_id":"impl-codes-rename","role":"implementer","session_id":"s-0-36-codes-rename"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-rename.F5
  @atip.codenames.09
  Scenario: A code that left the registry with no row in the CHANGELOG, or a row that names an unregistered code, is found
    Given a table that lacks the row of a code that left the registry, and a row that names a code the registry does not hold
    When the table is checked
    Then the missing code is named and the unregistered new name is found
