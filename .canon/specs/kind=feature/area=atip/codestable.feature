Feature: atip: the replacement table and the upgrade note of the 0.36 error codes
  # canon: {"schema":1,"at":"2026-11-04T13:52:00Z","actor":{"agent_id":"impl-codes-table","role":"implementer","session_id":"s-0-36-codes-table"}}

  # canon: {"schema":1,"at":"2026-11-04T13:52:01Z","actor":{"agent_id":"impl-codes-table","role":"implementer","session_id":"s-0-36-codes-table"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-table.F1
  @atip.codestable.01
  Scenario: The CHANGELOG replacement table has one row for each of the 179 codes of 0.35, and its new names are the registry's
    Given the 179 codes of the 0.35 registry kept as test data and the replacement table of the CHANGELOG's Unreleased section
    When the table is read against the registry
    Then each old name is in the table exactly once, each new name is registered, and each registered code is the new name or the kept name of some row

  # canon: {"schema":1,"at":"2026-11-04T13:52:02Z","actor":{"agent_id":"impl-codes-table","role":"implementer","session_id":"s-0-36-codes-table"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-table.F1
  @atip.codestable.02
  Scenario: The exit, retryable and reasons of every row are the 0.35 registry's before and the registry's after
    Given the replacement table, the exit and retryable of each 0.35 code, and the registry of 0.36
    When each row is compared
    Then the before values are the 0.35 ones, the after values are the exits and retryable values of the new codes, each reason is registered under one of the new codes, and the kind of the row says what the row shows

  # canon: {"schema":1,"at":"2026-11-04T13:52:03Z","actor":{"agent_id":"impl-codes-table","role":"implementer","session_id":"s-0-36-codes-table"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-table.F1
  @atip.codestable.03
  Scenario: A table with a row deleted, a new name misspelled, an exit wrong or an old name twice fails the table test
    Given the table with one row removed, a new name changed to a code the registry lacks, an exit changed and a row repeated
    When the table is checked
    Then each fault is named and the check fails

  # canon: {"schema":1,"at":"2026-11-04T13:52:04Z","actor":{"agent_id":"impl-codes-table","role":"implementer","session_id":"s-0-36-codes-table"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-table.F1
  @atip.codestable.04
  Scenario: The four lists under the table are the table's rows by exit, merge, split and removal
    Given the replacement table and the four lists under it
    When the lists are compared with the rows
    Then the list of exits that moved from 1 to 2, the merged codes, the split codes and the removed code are exactly the rows of those kinds

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

  # canon: {"schema":1,"at":"2026-11-04T13:52:07Z","actor":{"agent_id":"impl-codes-table","role":"implementer","session_id":"s-0-36-codes-table"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-table.F3
  @atip.codestable.07
  Scenario: No guide, website page, dashboard source, eval script, conformance case, scenario, design document or source spells a gone code
    Given the skills and their references, the website in English and Korean, the dashboard sources and tests, the eval scripts, the conformance cases, the scenarios outside area=atip, the design documents and the sources of canon
    When each file is searched for the 0.35 codes the registry no longer holds
    Then none of them is spelled, and the CHANGELOG table, the changes' history, the ledger and the dated research are the only places that spell them

  # canon: {"schema":1,"at":"2026-11-04T13:52:08Z","actor":{"agent_id":"impl-codes-table","role":"implementer","session_id":"s-0-36-codes-table"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-table.F3
  @atip.codestable.08
  Scenario: A guide that revives a gone code or says a code the registry lacks fails the guard
    Given a guide text that says a gone code, one that says a code with a misspelled name, and one that says a registered code
    When the guard reads each
    Then the first two are named and the third passes

  # canon: {"schema":1,"at":"2026-11-04T13:52:09Z","actor":{"agent_id":"impl-codes-table","role":"implementer","session_id":"s-0-36-codes-table"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-table.F3
  @atip.codestable.09
  Scenario: The website says the same codes in English and in Korean
    Given each English page of the website and its Korean page
    When the registered codes each page names are collected
    Then the two pages of every pair name the same codes

  # canon: {"schema":1,"at":"2026-11-04T13:52:10Z","actor":{"agent_id":"impl-codes-table","role":"implementer","session_id":"s-0-36-codes-table"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-table.F3
  @atip.codestable.10
  Scenario: No guide says exit 1 for a refusal of a gate, and the help and the registry say exit 2
    Given the guides, the design documents, the website and the help of canon that name a gate refusal
    When each sentence that names exit 1 beside a refusal code is read
    Then a refusal is exit 2 in all of them and exit 1 is said only for a check's answer, a failure or a stop

  # canon: {"schema":1,"at":"2026-11-04T13:52:11Z","actor":{"agent_id":"impl-codes-table","role":"implementer","session_id":"s-0-36-codes-table"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-table.F4
  @atip.codestable.11
  Scenario: The table of changed scenarios has one row for each scenario whose sentence changed and says whether only a name changed
    Given the table of scenario changes kept beside the change
    When its rows are read against the scenarios of the specs
    Then each row names a scenario that exists once, says whether only a code name or the meaning changed, and the count in its header is the number of rows

  # canon: {"schema":1,"at":"2026-11-04T13:52:12Z","actor":{"agent_id":"impl-codes-table","role":"implementer","session_id":"s-0-36-codes-table"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-table.F5
  @atip.codestable.12
  Scenario: The upgrade note tells a script, a hook, an agent guide and the dashboard what to change, with every exit that moved and every command that moved data to details
    Given the CHANGELOG's upgrade note and the table's list of exits that moved
    When the note is read
    Then it speaks to the four callers, names each code of the list, names gate promote, subject move, task done and brief approve for the refusals that carry error.details, says STOPPED_MIDWAY is gone and says no alias exists

  # canon: {"schema":1,"at":"2026-11-04T14:42:37Z","actor":{"agent_id":"impl-code-reasons","role":"implementer","session_id":"s-0-36-code-reasons"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-table.F2
  @atip.codestable.13
  Scenario: No paragraph of the Unreleased section names a removed code as current behaviour
    Given the Unreleased section of the CHANGELOG, outside the old-name column of the table and the lists under it
    When each sentence that names a code the registry no longer holds is read
    Then it says the name is gone, replaced, renamed or a caller's old name, and a stage paragraph that still names STOPPED_MIDWAY as the code of a stop is found by the guard

  # canon: {"schema":1,"at":"2026-11-05T09:00:03Z","actor":{"agent_id":"impl-flowfix-036","role":"implementer","session_id":"s-0-36-flowfix-2"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-table.F4
  @atip.codestable.14
  Scenario: A table of changed scenarios that miscounts, repeats, misnames or leaves a cell out is found
    Given the table of changed scenarios kept beside the change and the scenarios of the specs
    When a row is dropped, the header count is raised, a row is repeated, a scenario id is made up, a kind or a change is blanked, a cell is removed and a scenario leaves the specs
    Then the table as it stands passes and each fault is named, so a gate check cannot be called clean on a table that does not list the reviews it asks for

  # canon: {"schema":1,"at":"2026-11-05T09:00:04Z","actor":{"agent_id":"impl-flowfix-036","role":"implementer","session_id":"s-0-36-flowfix-2"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-table.F5
  @atip.codestable.15
  Scenario: An upgrade note that leaves a caller, a moved exit, a details command, the gone code or the word on aliases out fails the note check
    Given the CHANGELOG's upgrade note and the list of the exits that moved
    When an entry for a caller is renamed, the no-alias sentence is changed, the sentence on STOPPED_MIDWAY is changed, the entry on details is renamed, a moved code is dropped from the list, a command is no longer named and the note is removed
    Then the note as it stands passes and each missing part is named, so the release owner cannot release on a note that lacks it
