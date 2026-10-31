Feature: ATIP L0 output contract
  # canon: {"schema":1,"at":"2026-10-31T11:35:03Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}

  # canon: {"schema":1,"at":"2026-10-31T11:35:13Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-33-atip-l0.F1
  @atip.l0.01
  Scenario: status --json names the tool, its version and the contract in meta and has no canonVersion
    Given a repo with canon.yaml
    When canon status runs with --json
    Then meta holds tool canon, toolVersion equal to `canon --version`, contract atip/1 and schemaVersion, and no canonVersion

  # canon: {"schema":1,"at":"2026-10-31T11:35:16Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-33-atip-l0.F1
  @atip.l0.02
  Scenario: A write with several outputs names the output the data follows in meta.output
    Given a write command that declares several outputs (brief approve, init, a batch input of a write command)
    When it runs with --json
    Then meta.output is the report name the spec file gives that output (BriefApproveReport, InitCheckReport, BatchFindingCloseReport)

  # canon: {"schema":1,"at":"2026-10-31T11:35:18Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:edge
  @flow:canon-0-33-atip-l0.F1
  @atip.l0.03
  Scenario: An unresolved command leaves meta.command out and a half-resolved one names the path so far
    Given no repo
    When canon nosuch runs with --json, and canon finding nosuch runs with --json
    Then the first envelope has no meta.command and schemaVersion 1.0, and the second has meta.command finding

  # canon: {"schema":1,"at":"2026-10-31T11:35:21Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:edge
  @flow:canon-0-33-atip-l0.F1
  @atip.l0.04
  Scenario: meta.context holds only the context options, with paths relative to the working directory
    Given a repo
    When a leaf that takes --repo runs with --json from a subdirectory, and a leaf that takes no context option runs
    Then meta.context holds only repo, as a path relative to the working directory, and the second has no context

  # canon: {"schema":1,"at":"2026-10-31T11:35:24Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-33-atip-l0.F2
  @atip.l0.05
  Scenario: A member with no value is absent from the envelope and the error, never null or empty
    Given a failure with no hint and no suggestions, and a success with no next step and no warning
    When canon prints each with --json
    Then the error has no hint, no field, no expected and no suggestions key, meta has no next, warnings or context key, and retryable is present

  # canon: {"schema":1,"at":"2026-10-31T11:35:27Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-33-atip-l0.F2
  @atip.l0.06
  Scenario: Every envelope the suite prints passes the vendored contract schemas
    Given the conformance cases and the unit tests that print envelopes
    When each envelope is checked against the vendored envelope schema in crates/atip
    Then every envelope passes

  # canon: {"schema":1,"at":"2026-10-31T11:35:30Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-33-atip-l0.F2
  @atip.l0.07
  Scenario: A changed schema copy fails the digest test and an envelope with a null meta is rejected
    Given the three vendored contract schemas and their pinned digests
    When one character of a copy changes, and an envelope whose meta is null is validated
    Then the digest test fails for the copy and the envelope is rejected

  # canon: {"schema":1,"at":"2026-10-31T11:35:33Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-33-atip-l0.F3
  @atip.l0.08
  Scenario: A paged list carries meta.page and a next entry for the next page, and the last page has neither
    Given more records than one page
    When canon query --kind subject --limit 1 runs with --json, then again with its cursor, to the last page
    Then meta.page has returned, total and nextCursor, meta has no hasMore, meta.next holds the same call plus --cursor, and the last page has neither nextCursor nor a cursor step

  # canon: {"schema":1,"at":"2026-10-31T11:35:36Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-33-atip-l0.F3
  @atip.l0.09
  Scenario: A repeated write reports unchanged in data and the role split in data.waitingOn
    Given an evidence record already written
    When the same canon evidence add runs twice with --json under a declared role
    Then the second data.unchanged is true, meta has no unchanged, data.waitingOn holds the other roles' step counts, and a report that is a stored record carries them in meta.x-canon

  # canon: {"schema":1,"at":"2026-10-31T11:35:39Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:edge
  @flow:canon-0-33-atip-l0.F3
  @atip.l0.10
  Scenario: Projected and paged output carries only meta members the contract defines or x-canon
    Given a paged and a --fields listing
    When canon query --fields … --json and a page of finding list run
    Then every meta member is one the envelope schema defines or x-canon, and a stray meta key fails the envelope validation

  # canon: {"schema":1,"at":"2026-10-31T11:35:42Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-33-atip-l0.F3
  @atip.l0.11
  Scenario: A cursor canon did not issue is refused with CURSOR_INVALID and exit 2
    Given a listing
    When --cursor is given a value canon never issued, or one issued for another listing
    Then it exits 2 with CURSOR_INVALID and field --cursor, and writes nothing

  # canon: {"schema":1,"at":"2026-10-31T11:35:45Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-33-atip-l0.F4
  @atip.l0.12
  Scenario: Next entries carry reason and a runnable argv with --json in JSON mode
    Given a write whose next steps need no placeholder
    When it runs with --json
    Then each meta.next entry has reason instead of why, an argv with no word holding <, --json in argv, x-canon with actor and role, and its command split like a shell equals canon followed by argv

  # canon: {"schema":1,"at":"2026-10-31T11:35:48Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-33-atip-l0.F4
  @atip.l0.13
  Scenario: Every warning is a coded object and every warning code is in the registry
    Given every place canon raises a warning
    When a command raises one with --json
    Then each warning is an object with a code and a message and its code is in the registry, and a test finds every warning site's code registered

  # canon: {"schema":1,"at":"2026-10-31T11:35:51Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-33-atip-l0.F4
  @atip.l0.14
  Scenario: Guidance that needs a placeholder is a hint and never a next entry
    Given a next step that needs a value only the agent has
    When canon builds meta.next
    Then the step is not an entry and the same guidance is in the error hint or the report text

  # canon: {"schema":1,"at":"2026-10-31T11:35:54Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-33-atip-l0.F5
  @atip.l0.15
  Scenario: An unknown command answers UNKNOWN_COMMAND with the valid words
    Given no repo
    When canon nosuch runs with --json, and canon finding nosuch runs with --json
    Then both exit 2 with UNKNOWN_COMMAND, expected holds the valid words of that level, received holds the word and suggestions the nearest ones

  # canon: {"schema":1,"at":"2026-10-31T11:35:57Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-33-atip-l0.F5
  @atip.l0.16
  Scenario: A bare tool or group answers COMMAND_REQUIRED with the commands of that level
    Given no repo
    When canon runs with --json and no command, and canon finding runs with --json
    Then both exit 2 with COMMAND_REQUIRED and expected holds the commands of that level

  # canon: {"schema":1,"at":"2026-10-31T11:36:00Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-33-atip-l0.F5
  @atip.l0.17
  Scenario: An unknown option, a missing value or an unknown --fields name answers USAGE naming the argument
    Given a repo
    When canon status runs with --bogus, with an option missing its value, and with --fields naming an unknown field, each with --json
    Then each exits 2 with USAGE, field names the argument, meta.command is status, and the --fields refusal's expected lists the selectable fields

  # canon: {"schema":1,"at":"2026-10-31T11:36:03Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:edge
  @flow:canon-0-33-atip-l0.F5
  @atip.l0.18
  Scenario: A text-mode error leaves stdout empty and opens stderr with the tool, the code and the message
    Given the same failing calls without --json
    When canon nosuch, canon status --bogus and a refusal from a handler run
    Then stdout is empty and the first stderr line is canon: CODE: message, with field, expected and hint on the following lines

  # canon: {"schema":1,"at":"2026-10-31T11:36:06Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-33-atip-l0.F6
  @atip.l0.19
  Scenario: The registry holds the reserved codes with the contract's exits and none of the old names
    Given the error registry
    When it is read through canon schema --errors
    Then PLAN_STALE, BATCH_INCOMPLETE, CURSOR_INVALID, COMMAND_REQUIRED, UNKNOWN_COMMAND, INTERNAL and the three INPUT codes are there with the contract's exits, and no old code name is in any source, spec file or skill

  # canon: {"schema":1,"at":"2026-10-31T11:36:09Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-33-atip-l0.F6
  @atip.l0.20
  Scenario: A panic answers INTERNAL with exit 1 in JSON mode, never 101
    Given a debug build and a forced panic
    When canon runs with --json
    Then stdout holds an INTERNAL envelope whose message says how to report it, the exit is 1 and never 101, and stderr is empty

  # canon: {"schema":1,"at":"2026-10-31T11:36:12Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:edge
  @flow:canon-0-33-atip-l0.F6
  @atip.l0.21
  Scenario: A closed stdout ends the process by SIGPIPE, outside the registry
    Given a reader that closes stdout
    When canon prints a long listing into it
    Then the process ends by SIGPIPE, a signal exit outside the registry, and prints no envelope

  # canon: {"schema":1,"at":"2026-10-31T11:36:15Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-33-atip-l0.F6
  @atip.l0.22
  Scenario: --input - with another stdin reader is USAGE and --input - on a terminal is INPUT_INVALID
    Given --input - together with another option that reads stdin, and --input - on a terminal
    When each runs
    Then the first exits 2 with USAGE and field --input and the second exits 2 with INPUT_INVALID and field --input

  # canon: {"schema":1,"at":"2026-10-31T11:36:18Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-33-atip-l0.F7
  @atip.l0.23
  Scenario: Every error code ends in the exit its registry row holds and no call site sets an exit
    Given every error code and every call site
    When the sources are scanned and a failing command runs for each exit
    Then no call site overrides an exit, every exit is 1 or 2, and each error exits with its registry row's value

  # canon: {"schema":1,"at":"2026-10-31T11:36:21Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-33-atip-l0.F7
  @atip.l0.24
  Scenario: A target that cannot be written refuses with exit 2 before anything changes and a failure after the first write is exit 1
    Given a directory that cannot be written, and a store that fails after its first write
    When a command writes into the first, and a write fails after the first record in the second
    Then the first exits 2 with WRITE_UNAVAILABLE and changes nothing, and the second exits 1 with WRITE_FAILED

  # canon: {"schema":1,"at":"2026-10-31T11:36:24Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-33-atip-l0.F7
  @atip.l0.25
  Scenario: The thirteen checked codes are decided and the decision is in the changelog
    Given the thirteen exit-2 codes named for a failure during the work
    When each is decided as split, moved to exit 1 or kept at exit 2
    Then the CHANGELOG lists the split, moved and kept codes in one table, and the registry holds the decided exits

  # canon: {"schema":1,"at":"2026-10-31T11:36:27Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-33-atip-l0.F8
  @atip.l0.26
  Scenario: The workspace tests, the conformance cases and the generated docs agree with the new contract
    Given the 0.33 contract change
    When cargo test --workspace, the conformance cases and the generated-docs drift test run
    Then they pass, and the conformance regeneration differs from the committed cases only by the contract's changes

  # canon: {"schema":1,"at":"2026-10-31T11:36:30Z","actor":{"agent_id":"impl-atip-l0","role":"implementer","session_id":"s-0-33-atip-l0"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-33-atip-l0.F8
  @atip.l0.27
  Scenario: An old name in a guide, the website, the dashboard or an eval script fails the check, and a consumer that reads an old key fails its own test
    Given the guides, the website, the dashboard and the eval scripts
    When the old-name check runs, and a consumer reads an old key
    Then it passes on the new text, fails when one old name is restored, and the dashboard and launcher tests fail on an old key
