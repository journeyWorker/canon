Feature: atip check: the atip/1 L0 conformance checker
  # canon: {"schema":1,"at":"2026-10-31T11:45:03Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}

  # canon: {"schema":1,"at":"2026-10-31T11:45:04Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-33-atip-check.F1
  @atip.check.01
  Scenario: A conforming tool passes the 16 L0 checks: exit 0, data.checks lists the 16 ids and the 12 MUST checks pass
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:45:07Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.02
  Scenario: A tool that breaks one rule exits 1 with LEVEL_NOT_MET and data names the failing check, its rule numbers and the reason
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:45:10Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-33-atip-check.F1
  @atip.check.03
  Scenario: Both outcomes of atip check print one envelope that passes the contract envelope schema
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:45:13Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.04
  Scenario: A binary path that does not exist or cannot run exits 2 with BINARY_UNUSABLE
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:45:16Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-33-atip-check.F1
  @atip.check.05
  Scenario: Text mode prints one line per check and one summary line, and a failed level prints on stderr with stdout empty
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:45:19Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.06
  Scenario: A malformed atip command line is refused with USAGE, UNKNOWN_COMMAND or COMMAND_REQUIRED and names the argument at fault
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:45:22Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.07
  Scenario: L0.version fails when --version exits non-zero, prints nothing or writes to stderr
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:45:25Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.08
  Scenario: L0.version-format reports an advisory when --version is not one line of the tool name and the version
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:45:28Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.09
  Scenario: L0.unknown-command fails when the unknown-command envelope also goes to stderr
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:45:31Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.10
  Scenario: L0.unknown-command-text fails when the text-mode error goes to stdout
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:45:34Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.11
  Scenario: L0.text-error-form reports an advisory when stderr does not start with the tool name, UNKNOWN_COMMAND and a colon
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:45:36Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.12
  Scenario: L0.bare fails when a call with no command is not COMMAND_REQUIRED
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:45:39Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.13
  Scenario: L0.unknown-option fails when the unknown-option envelope leaves out the field
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:45:42Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.14
  Scenario: L0.json-mode-preparse fails when --json after an unknown option does not select JSON
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:45:45Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.15
  Scenario: L0.after-dashdash fails when --json after -- still selects JSON
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:45:48Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.16
  Scenario: L0.ok-exit fails when an unknown option exits 1, an exit is outside 0 to 2, or a failure disagrees with the registry
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:45:51Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.17
  Scenario: L0.no-null fails when an error carries hint: null
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:45:54Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.18
  Scenario: L0.compact reports an advisory when the envelope is indented
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:45:58Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.19
  Scenario: L0.one-document fails when the probe prints two JSON documents
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:46:01Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.20
  Scenario: L0.no-env-switch fails when an environment variable or a terminal turns JSON into text
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:46:04Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.21
  Scenario: L0.no-env-switch-text fails when CI=1 turns text mode into JSON
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:46:07Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.22
  Scenario: L0.contract reports an advisory when meta.contract is missing
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:46:10Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-33-atip-check.F1
  @atip.check.23
  Scenario: A failing SHOULD check is an advisory: the level is still met and the exit is 0
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:46:13Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-33-atip-check.F2
  @atip.check.24
  Scenario: The fixture's setup runs once and every invocation starts from a fresh copy of its state
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:46:16Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F2
  @atip.check.25
  Scenario: A fixture without a probe or a registry leaves the checks that need them unprovided and the level fails with exit 1
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:46:19Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F2
  @atip.check.26
  Scenario: An invocation that runs over the time limit fails its check with the reason
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:46:22Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F2
  @atip.check.27
  Scenario: An unreadable or wrongly shaped fixture, or a failing setup command, exits 2 with FIXTURE_INVALID
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:46:25Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-33-atip-check.F2
  @atip.check.28
  Scenario: An invocation sees only PATH, HOME, LANG and the fixture's variables, has stdin closed, and gets a terminal only where a check needs one
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:46:28Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-33-atip-check.F2
  @atip.check.29
  Scenario: The fixture keys commands, search and domainRejects are ignored at L0
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:46:31Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F3
  @atip.check.30
  Scenario: A refusal that writes a file in the working directory or in HOME fails and the report names the paths
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:46:35Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-33-atip-check.F3
  @atip.check.31
  Scenario: A refusal that writes nothing passes, and a successful call that writes a file is not judged at L0
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:46:38Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F4
  @atip.check.32
  Scenario: --level L1 and L2 exit 2 with LEVEL_UNSUPPORTED naming 0.34 and 0.35, and L9 is USAGE
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:46:41Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-33-atip-check.F4
  @atip.check.33
  Scenario: A refused level runs no binary and writes nothing
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:46:44Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-33-atip-check.F5
  @atip.check.34
  Scenario: Every valid envelope example of the contract repository passes the envelope schema
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:46:47Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F5
  @atip.check.35
  Scenario: Every invalid envelope example is rejected, and the examples that break only L1 or L2 rules are skipped with a reason
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:46:50Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F5
  @atip.check.36
  Scenario: A check forced to always pass makes its seeded-violation test fail
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:46:53Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-33-atip-check.F5
  @atip.check.37
  Scenario: atip check passes on atip itself
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:46:57Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-33-atip-check.F5
  @atip.check.38
  Scenario: The embedded contract schemas are the originals and a format keyword is an annotation, not an assertion
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:47:00Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-33-atip-check.F6
  @atip.check.39
  Scenario: The canon fixture's registry equals the one generated from canon schema --errors --json
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:47:03Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F6
  @atip.check.40
  Scenario: A stale registry in the canon fixture fails the generation test
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:47:13Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-33-atip-check.F6
  @atip.check.41
  Scenario: canon passes the 16 L0 checks through the committed fixture
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:47:16Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F6
  @atip.check.42
  Scenario: A canon whose error prints hint: null fails L0.no-null
    Given a step

  # canon: {"schema":1,"at":"2026-10-31T11:47:19Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-33-atip-check.F6
  @atip.check.43
  Scenario: The CI job and the just recipe run atip check against the canon binary with the same command
    Given a step
