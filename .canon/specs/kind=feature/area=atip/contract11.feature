Feature: ATIP 1.1 contract
  # canon: {"schema":1,"at":"2026-11-01T07:25:19Z","actor":{"agent_id":"impl-atip-1-1","role":"implementer","session_id":"s-0-34-atip-1-1"}}

  # canon: {"schema":1,"at":"2026-11-01T07:25:36Z","actor":{"agent_id":"impl-atip-1-1","role":"implementer","session_id":"s-0-34-atip-1-1"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-34-atip-1-1.F1
  @atip.contract11.01
  Scenario: Every envelope declares contract atip/1 and the contractVersion canon declares (1.2 since 0.35), and a registry answer names its kind
    Given a repo with canon.yaml
    When canon status --json, canon nosuch --json, canon schema --errors --json and canon schema --json run
    Then every meta holds contract atip/1 and contractVersion 1.2 and passes the contract envelope schema; the errors answer has data.kind errors and meta.schemaVersion 1.2

  # canon: {"schema":1,"at":"2026-11-01T07:33:52Z","actor":{"agent_id":"impl-atip-1-1","role":"implementer","session_id":"s-0-34-atip-1-1"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-34-atip-1-1.F1
  @atip.contract11.02
  Scenario: An envelope with contractVersion 1.0, or with contractVersion and no contract, fails the contract envelope schema
    Given envelopes with contractVersion 1.0, with contractVersion and no contract, and with contractVersion 2.0
    When each is checked against the vendored v1.1.0 envelope schema
    Then every one is refused, and an envelope with contract atip/1 and contractVersion 1.1 passes

  # canon: {"schema":1,"at":"2026-11-01T07:33:57Z","actor":{"agent_id":"impl-atip-1-1","role":"implementer","session_id":"s-0-34-atip-1-1"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-34-atip-1-1.F2
  @atip.contract11.03
  Scenario: Each --input fault carries the contract's reserved reason under INPUT_INVALID or INPUT_TOO_LARGE and no x-canon
    Given a repo
    When --input names a missing file, is empty, holds text that is not JSON, names a file holding 42, is `-` on a terminal, and is an array given to a command that takes one object; and a document with an unknown key, a flag-only key, a wrong type and a non-object batch item; and a file over 8 MiB and a batch of 10001 items
    Then each is INPUT_INVALID (INPUT_TOO_LARGE for the last two) with error.reason UNREADABLE, EMPTY, NOT_JSON, NOT_OBJECT, STDIN_TTY, BATCH_UNSUPPORTED, UNKNOWN_KEY, FLAG_ONLY_KEY, INVALID_VALUE, NOT_OBJECT, TOO_MANY_BYTES and TOO_MANY_ITEMS, no error carries x-canon, and the registry lists each reason under its code

  # canon: {"schema":1,"at":"2026-11-01T07:34:00Z","actor":{"agent_id":"impl-atip-1-1","role":"implementer","session_id":"s-0-34-atip-1-1"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-34-atip-1-1.F2
  @atip.contract11.04
  Scenario: A reason that is not registered under its code, or an INPUT_INVALID without a reason, fails the tests
    Given the sources, the registry and the conformance corpus
    When a test emits a reason that the registry does not hold under the code, and the sources and the corpus are scanned for INPUT_INVALID and INPUT_TOO_LARGE without a reason
    Then the unregistered reason fails in a debug build, a construction without a reason fails the scan, and every reason the registry holds is emitted somewhere

  # canon: {"schema":1,"at":"2026-11-01T07:34:03Z","actor":{"agent_id":"impl-atip-1-1","role":"implementer","session_id":"s-0-34-atip-1-1"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-34-atip-1-1.F3
  @atip.contract11.05
  Scenario: The warning registry answers schema --warnings and --warning with the same entries, and a warning-only code leaves the error registry
    Given a repo
    When canon schema --warnings --json runs, then canon schema --warning CODE --json for each code, and canon schema --errors --json
    Then the entries of the two answers are equal, every row validates against the warning entry schema, data.kind is warnings and warning, meta.schemaVersion is 1.2, and a code that is only a warning is in --warnings and not in --errors

  # canon: {"schema":1,"at":"2026-11-01T07:34:06Z","actor":{"agent_id":"impl-atip-1-1","role":"implementer","session_id":"s-0-34-atip-1-1"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-34-atip-1-1.F3
  @atip.contract11.06
  Scenario: An unknown warning code, a warning-only code given to --error, an unknown error code and two registry options together are USAGE
    Given a repo
    When --warning NOSUCH, --error with a warning-only code, --error NOSUCH, and two of path, --errors, --error, --warnings and --warning together run with --json
    Then each exits 2 with USAGE; the field is --warning, --error, --error and the first option given; the warning-only code's hint is canon schema --warning CODE --json

  # canon: {"schema":1,"at":"2026-11-01T07:34:09Z","actor":{"agent_id":"impl-atip-1-1","role":"implementer","session_id":"s-0-34-atip-1-1"}}
  @subject:atip-contract
  @case:edge
  @flow:canon-0-34-atip-1-1.F3
  @atip.contract11.07
  Scenario: Every code in meta.warnings is in a registry, and a code that is both a warning and an error has one meaning in both
    Given the sources and a repo that makes status warn
    When the warning sites are scanned and status runs with --json
    Then every code in meta.warnings and at a warning site is in the warning registry, a code that is only a warning is not in the error registry, and a code in both has the same meaning in both

  # canon: {"schema":1,"at":"2026-11-01T07:34:12Z","actor":{"agent_id":"impl-atip-1-1","role":"implementer","session_id":"s-0-34-atip-1-1"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-34-atip-1-1.F4
  @atip.contract11.08
  Scenario: An empty value the caller sent is received empty, and a value never sent has no received
    Given a repo
    When canon finding add runs with --change given as an empty string, and again with no --change
    Then the first is USAGE with received empty string, the second is USAGE with no received member

  # canon: {"schema":1,"at":"2026-11-01T07:34:15Z","actor":{"agent_id":"impl-atip-1-1","role":"implementer","session_id":"s-0-34-atip-1-1"}}
  @subject:atip-contract
  @case:edge
  @flow:canon-0-34-atip-1-1.F4
  @atip.contract11.09
  Scenario: A USAGE that cannot name its argument has no field and the hint canon schema command --json
    Given a repo
    When a USAGE that cannot name its argument is raised under a command and with no command resolved
    Then its hint is canon schema COMMAND --json, and canon schema --json when no command resolved, and the error has no field

  # canon: {"schema":1,"at":"2026-11-01T07:34:18Z","actor":{"agent_id":"impl-atip-1-1","role":"implementer","session_id":"s-0-34-atip-1-1"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-34-atip-1-1.F4
  @atip.contract11.10
  Scenario: Every hint in the sources and the corpus is a command line, a template or prose, and a retrieve with no scope is not prose that starts with canon
    Given the sources and the conformance corpus
    When every hint is read and canon retrieve runs with no scope argument
    Then a hint that starts with canon is one command line of canon's tree with UPPER_CASE placeholders, a prose hint never starts with canon, and the retrieve hint is canon retrieve --for-role ROLE --domain DOMAIN

  # canon: {"schema":1,"at":"2026-11-01T07:34:21Z","actor":{"agent_id":"impl-atip-1-1","role":"implementer","session_id":"s-0-34-atip-1-1"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-34-atip-1-1.F5
  @atip.contract11.11
  Scenario: A tool-wide option after a group word is COMMAND_REQUIRED, and an unknown option there is still USAGE
    Given a repo
    When canon finding --repo . --json, canon finding --as planner --json and canon finding --bogus --json run, and canon finding --repo . list runs
    Then the first two exit 2 with COMMAND_REQUIRED and expected the commands of finding, the third is USAGE, and the fourth runs list with --repo .

  # canon: {"schema":1,"at":"2026-11-01T07:34:24Z","actor":{"agent_id":"impl-atip-1-1","role":"implementer","session_id":"s-0-34-atip-1-1"}}
  @subject:atip-contract
  @case:edge
  @flow:canon-0-34-atip-1-1.F6
  @atip.contract11.12
  Scenario: The dashboard, the launcher and the dogfood trace check keep reading when a document has members and enum strings they do not know
    Given an envelope with members and enum strings from a later minor: contractVersion 1.9, an unknown error reason, an unknown finding severity, an unknown approval outcome
    When the dashboard, the npm launcher and the dogfood trace check read it
    Then each keeps reading what it knows, skips only the item with the enum string it does not know, and the launcher passes the document through with the binary's exit status

  # canon: {"schema":1,"at":"2026-11-01T07:34:27Z","actor":{"agent_id":"impl-atip-1-1","role":"implementer","session_id":"s-0-34-atip-1-1"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-34-atip-1-1.F6
  @atip.contract11.13
  Scenario: None of the 0.33 names is left in the guides, the website, the evaluation scripts, the spec files or the scenario sentences, and reviving one fails
    Given the guides, the website in English and Korean, the command spec files, the conformance corpus, the evaluation scripts and the scenario sentences
    When they are scanned for the 0.33 reason extension of the error, the merged registry error code and the lowercase reason values, and one of the names is put back
    Then the scan finds none, and the revived name is found

  # canon: {"schema":1,"at":"2026-11-01T10:53:28Z","actor":{"agent_id":"impl-atip-1-1","role":"implementer","session_id":"s-0-34-atip-1-1"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-34-atip-1-1.F4
  @atip.contract11.14
  Scenario: A trace or search that refuses leaves the repository as it was: no index directory or file is written
    Given a repo with a finding and no index, and a repo whose index is fresh or stale
    When canon trace is given an id no record says, a sha prefix no record has, an operand with no shape, and canon search is given an empty text
    Then each exits 2 with TRACE_NOT_FOUND, TRACE_OPERAND_INVALID or SEARCH_QUERY_EMPTY, no file under the repo changes, and no index directory or file appears; a fresh or stale index stays as it was, and a trace that answers still writes the index

  # canon: {"schema":1,"at":"2026-11-01T13:29:54Z","actor":{"agent_id":"impl-atip-1-1","role":"implementer","session_id":"s-0-34-atip-1-1"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-34-atip-1-1.F4
  @atip.contract11.15
  Scenario: A refusal, or a read, leaves the repository as it was, .git included: the code tree is computed in a scratch object directory
    Given a git repo whose work tree has an untracked code file, and the commands that read or refuse run in it
    When the code tree is computed, and each read leaf is given an id or a value it refuses
    Then every refusal exits 2 with its code and no file in the work tree or under .git is added, removed or changed (the index file and logs aside), and the code tree is the same id twice

  # canon: {"schema":1,"at":"2026-11-01T19:32:28Z","actor":{"agent_id":"impl-atip-1-1","role":"implementer","session_id":"s-0-34-atip-1-1"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-34-atip-1-1.F5
  @atip.contract11.16
  Scenario: A group option that is not tool-wide, or a tool-wide option with no value, is still USAGE naming the option
    Given a repo
    When canon finding --bogus --json, canon finding --repo --json, canon finding --as --json and canon skills --repo . show canon --json run
    Then each exits 2 with USAGE and the field is --bogus, --repo, --as and --repo, and none is COMMAND_REQUIRED
