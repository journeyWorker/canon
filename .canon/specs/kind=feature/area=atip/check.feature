Feature: atip check: the atip/1 L0 conformance checker
  # canon: {"schema":1,"at":"2026-10-31T11:45:03Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}

  # canon: {"schema":1,"at":"2026-10-31T11:45:04Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-33-atip-check.F1
  @atip.check.01
  Scenario: A conforming 1.1 tool passes the 27 applicable L0 checks: exit 0, data.checks lists the 28 ids and the 21 MUST checks pass
    Given a fake tool configured to follow atip/1 1.1 L0 and a fixture with contractVersion, setup, probe, globalOptions, registry, warnings and commands
    When atip check --bin <tool> --level L0 --fixture <file> --json runs
    Then it exits 0 with ok true, data.passed true and data.contractVersion 1.1 and data.checks listing L0.version … L0.global-on-group in table order, 28 of them, every one pass except L0.contract (unsupported: a 1.0 rule), 21 of them MUST and 6 of the applicable SHOULD, each with its spec rule numbers

  # canon: {"schema":1,"at":"2026-10-31T11:45:07Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.02
  Scenario: A tool that breaks one rule exits 1 with LEVEL_NOT_MET and data names the failing check, its rule numbers and the reason
    Given a fake tool whose errors print hint: null
    When atip check runs with --json
    Then it exits 1 with ok false and error.code LEVEL_NOT_MET, data.passed false, and data.checks has L0.no-null as fail with strength MUST, rules [6.5], a reason naming hint and the invocation (argv and stdout) that decided it

  # canon: {"schema":1,"at":"2026-10-31T11:45:10Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-33-atip-check.F1
  @atip.check.03
  Scenario: Both outcomes of atip check print one envelope that passes the contract envelope schema
    Given atip check finishing as a pass, as a failed level, as a refused level and as an unknown command
    When each prints with --json
    Then stdout is one line holding one envelope that validates against envelope.schema.json, with meta.tool atip and meta.contract atip/1, and a refusal has no meta.command when no word resolved

  # canon: {"schema":1,"at":"2026-10-31T11:45:13Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.04
  Scenario: A binary path that does not exist or cannot run exits 2 with BINARY_UNUSABLE
    Given a --bin that is missing, not executable, a directory, a file that cannot be started, or a name not on PATH
    When atip check runs with --json
    Then it exits 2 with error.code BINARY_UNUSABLE and field --bin, no data, and it writes nothing in the working directory or in TMPDIR; a bare name found on PATH is run

  # canon: {"schema":1,"at":"2026-10-31T11:45:16Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-33-atip-check.F1
  @atip.check.05
  Scenario: Text mode prints one line per check and one summary line, and a failed level prints on stderr with stdout empty
    Given a conforming tool and a tool whose --version exits 1
    When atip check runs in text mode
    Then the pass prints one line per check and a summary line `L0 met: …` on stdout with stderr empty; the failure prints nothing on stdout and `atip: LEVEL_NOT_MET: …`, the check lines and the summary on stderr

  # canon: {"schema":1,"at":"2026-10-31T11:45:19Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.06
  Scenario: A malformed atip command line is refused with USAGE, UNKNOWN_COMMAND or COMMAND_REQUIRED and names the argument at fault
    Given atip is run with no command, an unknown command, an unknown option before or after --json, a missing required option, a stray operand or after --
    When it runs with --json or in text mode
    Then it exits 2 with COMMAND_REQUIRED, UNKNOWN_COMMAND (field command, expected [check]) or USAGE naming the argument in field, stderr is empty in JSON mode and stdout is empty in text mode, and nothing is written; a panic is INTERNAL with exit 1

  # canon: {"schema":1,"at":"2026-10-31T11:45:22Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.07
  Scenario: L0.version fails when --version exits nonzero or prints nothing
    Given a fake tool whose --version exits 1, and another whose --version prints nothing
    When atip check runs
    Then L0.version is fail for each with the condition in the reason, and no other MUST check fails

  # canon: {"schema":1,"at":"2026-10-31T11:45:25Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.08
  Scenario: L0.version-format reports an advisory when --version is not one line of the tool name and the version
    Given a fake tool whose --version prints a bare `1.2.3`
    When atip check runs
    Then L0.version-format is fail, a SHOULD, so the level is met and the exit is 0, and L0.version still passes

  # canon: {"schema":1,"at":"2026-10-31T11:45:28Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.09
  Scenario: L0.unknown-command fails on each part of the unknown-command envelope
    Given a fake tool whose unknown-command answer is not an envelope, has another code, lacks error.expected, names meta.command, has another schemaVersion, or is also printed on stderr
    When atip check runs
    Then L0.unknown-command is fail for each and no other MUST check fails

  # canon: {"schema":1,"at":"2026-10-31T11:45:31Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.10
  Scenario: L0.unknown-command-text fails on the exit or either stream of the text-mode error
    Given a fake tool whose text-mode unknown command exits 1, prints the error on stdout, or prints nothing on stderr
    When atip check runs
    Then L0.unknown-command-text is fail for each and no other MUST check fails

  # canon: {"schema":1,"at":"2026-10-31T11:45:34Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.11
  Scenario: L0.text-error-form reports an advisory when stderr does not start with the tool name, UNKNOWN_COMMAND and a colon
    Given a fake tool whose text-mode error starts `error:` instead of `<tool>: UNKNOWN_COMMAND: `
    When atip check runs
    Then L0.text-error-form is fail, a SHOULD, so the level is met

  # canon: {"schema":1,"at":"2026-10-31T11:45:36Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.12
  Scenario: L0.bare fails when a call with no command is not a COMMAND_REQUIRED envelope
    Given a fake tool that answers a bare --json with USAGE, with text, or with the error also on stderr
    When atip check runs
    Then L0.bare is fail for each and no other MUST check fails

  # canon: {"schema":1,"at":"2026-10-31T11:45:39Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.13
  Scenario: L0.unknown-option fails on the code, the field or the command of the envelope
    Given a fake tool whose unknown-option error has a code that is not a registered refusal, no field, no meta.command, or is also on stderr; and a registry that gives USAGE exit 1
    When atip check runs
    Then L0.unknown-option is fail for each and no other MUST check fails

  # canon: {"schema":1,"at":"2026-10-31T11:45:42Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.14
  Scenario: L0.json-mode-preparse fails when --json after an unknown option does not select JSON
    Given a fake tool that prints text when an unknown option comes after --json, and another when it comes before
    When atip check runs
    Then L0.json-mode-preparse is fail for each and no other MUST check fails

  # canon: {"schema":1,"at":"2026-10-31T11:45:45Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.15
  Scenario: L0.after-dashdash fails when --json after -- still selects JSON
    Given a fake tool that treats --json after -- as selecting JSON
    When atip check runs
    Then L0.after-dashdash is fail and no other MUST check fails

  # canon: {"schema":1,"at":"2026-10-31T11:45:48Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.16
  Scenario: L0.ok-exit fails on an exit outside 0 to 2, an ok that disagrees with the exit, or a failure that disagrees with the registry
    Given a fake tool that exits 3, exits 1 with ok true, fails with an unregistered code, an exit other than the registry's, a retryable other than the registry's, data on a refusal, or a registry that gives a reserved code another exit
    When atip check runs
    Then L0.ok-exit is fail for each with the invocation named, and no other MUST check fails

  # canon: {"schema":1,"at":"2026-10-31T11:45:51Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.17
  Scenario: L0.no-null fails when an error carries hint: null
    Given a fake tool whose errors print hint: null
    When atip check runs
    Then L0.no-null is fail with the schema's rejection of /error/hint in the reason

  # canon: {"schema":1,"at":"2026-10-31T11:45:54Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.18
  Scenario: L0.compact reports an advisory when the envelope is indented
    Given a fake tool that prints an indented envelope
    When atip check runs
    Then L0.compact is fail, a SHOULD, so the level is met

  # canon: {"schema":1,"at":"2026-10-31T11:45:58Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.19
  Scenario: L0.one-document fails when the probe prints two JSON documents
    Given a fake tool whose probe, unknown command or bare call prints two documents or no final newline
    When atip check runs
    Then L0.one-document is fail and no other MUST check fails

  # canon: {"schema":1,"at":"2026-10-31T11:46:01Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.20
  Scenario: L0.no-env-switch fails when the environment or a terminal changes the probe
    Given a fake tool whose probe output changes with OUTPUT_FORMAT, on a terminal, or with CI
    When atip check runs with the probe run under each variation
    Then L0.no-env-switch is fail naming the variation, and no other MUST check fails

  # canon: {"schema":1,"at":"2026-10-31T11:46:04Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.21
  Scenario: L0.no-env-switch-text fails when CI=1 turns text mode into JSON
    Given a fake tool that prints JSON in text mode when CI is set
    When atip check runs
    Then L0.no-env-switch-text is fail and no other MUST check fails

  # canon: {"schema":1,"at":"2026-10-31T11:46:07Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.22
  Scenario: L0.contract reports an advisory when meta.contract is missing
    Given a fake tool whose envelopes lack meta.contract
    When atip check runs
    Then L0.contract is fail, a SHOULD, so the level is met

  # canon: {"schema":1,"at":"2026-10-31T11:46:10Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-33-atip-check.F1
  @atip.check.23
  Scenario: A failing SHOULD check is a recommendation: the level is still met and the exit is 0
    Given a fake tool that fails every applicable SHOULD check and passes every MUST check
    When atip check runs with --json
    Then it exits 0 with ok true and data.passed true, the SHOULD checks are fail, meta.warnings has SHOULD_FAILED, and the text summary counts them as advisories

  # canon: {"schema":1,"at":"2026-10-31T11:46:13Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-33-atip-check.F2
  @atip.check.24
  Scenario: The fixture's setup runs once and every invocation starts from a fresh copy of its state
    Given a fixture whose setup writes files and a tool that fails when a file an earlier invocation left is present
    When atip check runs
    Then the setup ran once, every invocation started from a fresh copy of the setup state with HOME inside the copy, and the level is met

  # canon: {"schema":1,"at":"2026-10-31T11:46:16Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F2
  @atip.check.25
  Scenario: A fixture without a probe, a registry or commands leaves the checks that need them unprovided and the level fails with exit 1
    Given a fixture without probe, without registry, without commands, or without all three
    When atip check runs with --json
    Then the checks that need the missing entry and saw no violation are unprovided with the entry named, the level fails with exit 1, and a violation already seen is fail instead of unprovided

  # canon: {"schema":1,"at":"2026-10-31T11:46:19Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F2
  @atip.check.26
  Scenario: An invocation that runs over the time limit fails its check with the reason
    Given a fake tool whose --version outlives timeoutSeconds
    When atip check runs
    Then the call is killed with its process group, L0.version is fail with `ran over the 1 second limit` and the check's other results are unchanged

  # canon: {"schema":1,"at":"2026-10-31T11:46:22Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F2
  @atip.check.27
  Scenario: An unreadable or wrongly shaped fixture, or a failing setup command, exits 2 with FIXTURE_INVALID
    Given a fixture that is unreadable, not JSON, an array, has a setup of the wrong shape, an env that sets HOME, a bad probe, a bad registry row, or a setup command that fails
    When atip check runs with --json
    Then it exits 2 with FIXTURE_INVALID and field --fixture, no data, and writes nothing, its scratch space included

  # canon: {"schema":1,"at":"2026-10-31T11:46:25Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-33-atip-check.F2
  @atip.check.28
  Scenario: An invocation sees only PATH, HOME, LANG and the fixture's variables, has stdin closed, and gets a terminal only where a check needs one
    Given a process run by the checker
    When it prints its environment, reads stdin and tests its stdout
    Then the environment is only PATH, HOME, LANG=C.UTF-8 and the fixture's variables, stdin is /dev/null unless the call asks for a terminal, and stdout is a pipe unless the call asks for a raw terminal whose output is read completely however fast the process exits

  # canon: {"schema":1,"at":"2026-10-31T11:46:28Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-33-atip-check.F2
  @atip.check.29
  Scenario: The fixture keys of L1 and L2 are ignored at L0
    Given a fixture whose commands also hold fields, domainRejects, batchFailsAt, preview and async, and that has search and an unknown key
    When atip check runs
    Then the results are the same as without those keys

  # canon: {"schema":1,"at":"2026-10-31T11:46:31Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F3
  @atip.check.30
  Scenario: L0.refusal-effects fails when a refusal writes in the working directory or in HOME and names the paths
    Given a fake tool that writes ./refused.txt, or ~/telemetry.log, when it refuses
    When atip check runs
    Then L0.refusal-effects is fail with the changed paths (./refused.txt, ~/telemetry.log) in the reason and observed, rules [8.1, 8.2], and no other check fails

  # canon: {"schema":1,"at":"2026-10-31T11:46:35Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-33-atip-check.F3
  @atip.check.31
  Scenario: A refusal that writes nothing passes, and a successful call that writes a file is not judged at L0
    Given a fake tool whose successful call writes a file, and a conforming tool whose refusals write nothing
    When atip check runs
    Then every check passes: a refusal that writes nothing passes, and a success that writes is not judged at L0

  # canon: {"schema":1,"at":"2026-10-31T11:46:38Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F4
  @atip.check.32
  Scenario: --level L1 and L2 exit 2 with LEVEL_UNSUPPORTED naming 0.34 and 0.35, and L9 is USAGE
    Given --level L1, L2, L9, l0, 0 or an empty level
    When atip check runs with --json and in text mode
    Then L1 and L2 exit 2 with LEVEL_UNSUPPORTED, field --level, expected [L0] and a hint naming 0.34 or 0.35; the others exit 2 with USAGE and expected [L0, L1, L2]; L0 still runs

  # canon: {"schema":1,"at":"2026-10-31T11:46:41Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-33-atip-check.F4
  @atip.check.33
  Scenario: A refused level runs no binary and writes nothing
    Given --level L1, L2 or L9 with a binary that touches a marker file when run
    When atip check runs
    Then it exits 2, the marker was never created, and nothing was written in the working directory or TMPDIR; L0 does start the binary

  # canon: {"schema":1,"at":"2026-10-31T11:46:44Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-33-atip-check.F5
  @atip.check.34
  Scenario: Every valid envelope example of the contract repository passes the envelope schema
    Given the report-*, schema-error*, schema-warning* and stream-* examples of the contract repository pinned at v1.1.0
    When each is validated against the envelope schema and the failure rules against the registries the examples ship
    Then every one is accepted

  # canon: {"schema":1,"at":"2026-10-31T11:46:47Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F5
  @atip.check.35
  Scenario: Every invalid envelope example is rejected, and the examples that break only L1 or L2 rules are skipped with a reason
    Given the invalid examples of the contract repository and the table that says what each should do
    When each is run through the layers atip check applies to one envelope
    Then each that breaks an envelope or L0 rule is rejected by the one layer the table names and no other, the ones that break only L1 or L2 rules are skipped with a reason and no L0 layer rejects them, and every file has a row

  # canon: {"schema":1,"at":"2026-10-31T11:46:50Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F5
  @atip.check.36
  Scenario: A check forced to always pass makes its seeded-violation test fail
    Given a seeded violation for each of the 28 checks
    When its check is forced to pass and atip check runs
    Then the seeded violation's test fails, so a check that never fails cannot go unnoticed

  # canon: {"schema":1,"at":"2026-10-31T11:46:53Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-33-atip-check.F5
  @atip.check.37
  Scenario: atip check passes on atip itself
    Given atip and its fixture, which claims 1.1, whose probe is check, whose registry and warnings are generated from atip's own and whose commands are the refusal and the missing value of atip check
    When atip check runs on atip
    Then it exits 0 with 18 MUST and the other applicable checks passing and 6 not applicable (L0.next, L0.page, L0.registry, L0.contract, L0.usage-hint and L0.global-on-group: atip has no next step, page, schema, USAGE without a field or group), and the committed fixture equals the one generated from the registry and warnings in the code

  # canon: {"schema":1,"at":"2026-10-31T11:46:57Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-33-atip-check.F5
  @atip.check.38
  Scenario: The embedded contract schemas are the originals and a format keyword is an annotation, not an assertion
    Given the five contract schemas atip embeds (envelope, error, registry-entry, warning-entry, stream-line)
    When their digests are compared with the originals at v1.1.0 and a format keyword is validated
    Then they are byte for byte the originals, a null where the contract forbids one is rejected, and a value that is not a date-time passes a schema that says format date-time

  # canon: {"schema":1,"at":"2026-10-31T11:47:00Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-33-atip-check.F6
  @atip.check.39
  Scenario: The canon fixture's registry and warnings equal the ones generated from canon schema
    Given canon built from this checkout
    When the fixture's registry and warnings are compared with canon schema --errors --json and --warnings --json
    Then they are equal by code, exit, retryable and reason names, and the fixture's keys are only contractVersion, setup, probe, globalOptions, registry, warnings and commands (a paging call, a next-step call and a missingValue)

  # canon: {"schema":1,"at":"2026-10-31T11:47:03Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F6
  @atip.check.40
  Scenario: A stale registry in the canon fixture fails the generation test
    Given a canon fixture with a row dropped, an exit changed, a retryable flipped, a member of prose added, a warning dropped, a call changed or a key added
    When the generation test compares it with the generated one
    Then each difference is noticed

  # canon: {"schema":1,"at":"2026-10-31T11:47:13Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-33-atip-check.F6
  @atip.check.41
  Scenario: canon passes the 26 applicable L0 checks through the committed fixture
    Given canon built from this checkout and its committed fixture
    When atip check runs at L0
    Then the 21 MUST and 5 SHOULD checks pass, L0.next and L0.page among them because the fixture has a paging call and a next-step call, L0.contract (a 1.0 rule) and L0.usage-hint (canon's every USAGE names a field) do not apply, and the level is met

  # canon: {"schema":1,"at":"2026-10-31T11:47:16Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F6
  @atip.check.42
  Scenario: A canon whose error prints a null member fails L0.no-null
    Given canon wrapped so that every error that has a hint also prints a null member before it
    When atip check runs at L0
    Then L0.no-null is fail and it is the only check that fails

  # canon: {"schema":1,"at":"2026-10-31T11:47:19Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-33-atip-check.F6
  @atip.check.43
  Scenario: The CI job and the just recipe run atip check against the canon binary with the same command
    Given the workflow job atip-l0 and the just recipe atip
    When their commands are read
    Then both build canon-cli and atip-check with --locked and run atip check on the canon binary and on atip with the same arguments

  # canon: {"schema":1,"at":"2026-10-31T15:10:12Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.44
  Scenario: L0.next fails on an empty argv, a placeholder, a missing --json or a command that is not its argv
    Given a fake tool whose next step has an empty argv, a placeholder, no --json in a JSON-mode call, or a command that is not its argv
    When atip check runs
    Then L0.next is fail for each with the rule 6.8 and no other MUST check fails

  # canon: {"schema":1,"at":"2026-10-31T15:10:16Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.45
  Scenario: L0.page fails on a malformed page or a nextCursor with no cursor step
    Given a fake tool whose page is malformed, or whose nextCursor has no next step with --cursor <value>
    When atip check runs
    Then L0.page is fail for each with the rule 6.9 and no other MUST check fails

  # canon: {"schema":1,"at":"2026-10-31T15:10:20Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-33-atip-check.F1
  @atip.check.46
  Scenario: L0.next and L0.page do not apply when no envelope carries a next step or a page
    Given a fake tool whose envelopes carry no meta.next and no meta.page and whose commands refuse --cursor
    When atip check runs
    Then L0.next and L0.page are unsupported, every other check passes, and the level is met

  # canon: {"schema":1,"at":"2026-10-31T15:10:23Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-33-atip-check.F1
  @atip.check.47
  Scenario: Stderr output that carries no error or result is a recommendation, and an error or result on stderr fails L0.stderr-result
    Given a fake tool that prints noise on stderr in JSON mode, and another that prints the error on stderr
    When atip check runs
    Then the noise fails only L0.stderr-quiet, a SHOULD, and the level is met; the error on stderr fails only L0.stderr-result

  # canon: {"schema":1,"at":"2026-10-31T15:10:27Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-33-atip-check.F1
  @atip.check.48
  Scenario: A signal exit of either form is accepted and an exit of 3 is not
    Given a fake tool that ends `<probe> -- --json` by SIGTERM, by exit 130, or by exit 3
    When atip check runs
    Then the signal and the exit 130 are accepted, the exit 3 fails L0.ok-exit

  # canon: {"schema":1,"at":"2026-10-31T15:10:30Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F1
  @atip.check.49
  Scenario: The registries the tool prints with schema are compared with the fixture's, and so are the reasons and the warning codes
    Given a fake tool that offers schema --errors --json and --warnings --json with registries that equal, or differ in one row, one reason or one warning from, the fixture's, or with rows only on one side
    When atip check runs
    Then a difference fails L0.registry naming the code, the reason or the warning, and a tool that does not offer schema is not compared

  # canon: {"schema":1,"at":"2026-10-31T15:10:34Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F2
  @atip.check.50
  Scenario: A tool that waits on a terminal for a missing value fails L0.no-prompt
    Given a fake tool that reads a line, or prints a prompt in raw mode, or reacts to end-of-file when --name is missing and stdin is a terminal
    When atip check runs the missingValue invocation once with stdin closed and once on a canonical-mode terminal with the deadline twice the first run plus two seconds
    Then the call is stopped at the deadline, L0.no-prompt is fail naming that the tool waits for input or asks for a value, and no other check fails; a tool that sleeps without reading and prints nothing is unprovided

  # canon: {"schema":1,"at":"2026-10-31T15:10:37Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F2
  @atip.check.51
  Scenario: A probe whose data keys change with the environment, or that is no envelope, fails L0.no-env-switch
    Given a fake tool whose probe's top-level data keys change with CI, or whose probe prints no envelope
    When atip check runs
    Then L0.no-env-switch is fail naming the data keys or the baseline, and no other MUST check fails

  # canon: {"schema":1,"at":"2026-10-31T15:10:41Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-33-atip-check.F2
  @atip.check.52
  Scenario: A violation seen is a failure even when the fixture lacks the entry, and nothing seen is unprovided
    Given a fixture without the registry and a tool that exits 3, and a fixture without the probe and a tool whose unknown command prints two documents
    When atip check runs
    Then L0.ok-exit and L0.one-document are fail, not unprovided, while the checks that saw no violation are unprovided

  # canon: {"schema":1,"at":"2026-10-31T15:10:45Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-33-atip-check.F2
  @atip.check.53
  Scenario: A completed run leaves the working directory, home and scratch space as it found them
    Given atip check run from an empty directory with an empty TMPDIR and an empty HOME
    When it completes, as a pass and as a failed level
    Then the working directory, TMPDIR and HOME are empty afterwards

  # canon: {"schema":1,"at":"2026-10-31T15:10:48Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F2
  @atip.check.54
  Scenario: A closed stdout ends atip by SIGPIPE, never by exit 101
    Given atip with a stdout whose reader has already gone
    When it runs --help, --version, check --help, a passing check, a text check, an unknown command and a failing check
    Then each ends by SIGPIPE and none panics or exits 101; a failed level in text mode, which writes only to stderr, still exits 1

  # canon: {"schema":1,"at":"2026-10-31T15:10:52Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-33-atip-check.F5
  @atip.check.55
  Scenario: The fake tool is a test-only binary the checker does not ship
    Given the atip-check package
    When its cargo metadata is read
    Then atip-fake-tool is a bin with required-features test-support, no default feature builds it, and no workspace crate depends on atip-check as a normal dependency

  # canon: {"schema":1,"at":"2026-10-31T15:10:56Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F5
  @atip.check.56
  Scenario: Each pass condition fails exactly its owner, and a run with the condition switched off does not
    Given every pass condition of the 28 checks, each with the fake tool configuration that breaks only it
    When atip check runs with the condition planted and again with the condition switched off
    Then planted, exactly the owner of the condition fails; switched off, nothing fails; and the conditions the checker judges are exactly the conditions the tests plant

  # canon: {"schema":1,"at":"2026-11-01T07:27:17Z","actor":{"agent_id":"impl-atip-check-1-1","role":"implementer","session_id":"s-0-34-check-1-1"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-34-atip-check-1-1.F1
  @atip.check.57
  Scenario: The --contract option and the fixture's contractVersion pick the claim, and two that differ are refused with USAGE before any binary runs
    Given a conforming fake tool and a fixture with and without contractVersion, and runs with --contract 1.1, --contract 1.0 and --contract 1.9
    When atip check runs
    Then --contract alone claims 1.1 (28 checks, the 21 MUST of the applicable 27 pass, L0.contract is unsupported); nothing claimed is 1.0; a --contract that differs from the fixture's contractVersion, or a minor the checker does not know, exits 2 with USAGE naming --contract and runs no binary, not even the setup

  # canon: {"schema":1,"at":"2026-11-01T07:27:22Z","actor":{"agent_id":"impl-atip-check-1-1","role":"implementer","session_id":"s-0-34-check-1-1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-1-1.F1
  @atip.check.58
  Scenario: A tool that claims 1.0 and prints a member of 1.1 fails L0.contract-version, and its prose hint is not judged by L0.hint-form
    Given a fake tool that claims 1.0 and prints error.reason, and one that claims 1.0 and prints a prose hint starting with its name
    When atip check runs
    Then the first fails L0.contract-version and nothing else; the second passes with L0.hint-form unsupported, and fails L0.hint-form when it is run with --contract 1.1

  # canon: {"schema":1,"at":"2026-11-01T07:27:26Z","actor":{"agent_id":"impl-atip-check-1-1","role":"implementer","session_id":"s-0-34-check-1-1"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-34-atip-check-1-1.F2
  @atip.check.59
  Scenario: A rule that reads one document is judged once, by the check that owns it, and the others leave its findings alone
    Given documents that break the envelope schema, hold a null, carry a member of a later minor or an unregistered code
    When each rule that reads one document is applied to it
    Then the finding is reported by the one check that owns the rule (envelope, no-null, contract-version, registry) and the other checks report nothing for it

  # canon: {"schema":1,"at":"2026-11-01T07:27:29Z","actor":{"agent_id":"impl-atip-check-1-1","role":"implementer","session_id":"s-0-34-check-1-1"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-34-atip-check-1-1.F2
  @atip.check.60
  Scenario: An invocation of the fixture runs in JSON mode and the cursor probe replaces its cursor before any --
    Given an argv of the fixture with and without --json and with a -- in it, and one with a --cursor already
    When the checker builds the invocation and the cursor probe
    Then --json is put before any --, and the probe's argv has --cursor x in place of any other cursor

  # canon: {"schema":1,"at":"2026-11-01T07:27:33Z","actor":{"agent_id":"impl-atip-check-1-1","role":"implementer","session_id":"s-0-34-check-1-1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-1-1.F5
  @atip.check.61
  Scenario: A violation that fails two checks fails the seeded test that named one
    Given a fake tool configured with two planted violations that belong to two checks
    When the seeded verdict for one of them is judged
    Then the verdict is refused naming the second failing check, and a run with no failing owner is refused too

  # canon: {"schema":1,"at":"2026-11-01T07:27:37Z","actor":{"agent_id":"impl-atip-check-1-1","role":"implementer","session_id":"s-0-34-check-1-1"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-34-atip-check-1-1.F4
  @atip.check.62
  Scenario: The output of a terminal run is read to its end and the tail of a long output stays in the report
    Given a fake tool that refuses a missing value and then prints 60000 characters ending in a marker on a terminal
    When atip check runs the missingValue invocation on a terminal
    Then the output is read to its end, L0.no-prompt fails on the extra output and the report keeps the marker at the end of the shown output

  # canon: {"schema":1,"at":"2026-11-01T07:27:41Z","actor":{"agent_id":"impl-atip-check-1-1","role":"implementer","session_id":"s-0-34-check-1-1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-1-1.F4
  @atip.check.63
  Scenario: A fixture without a missingValue invocation leaves L0.no-prompt unprovided, and a missingValue the tool does not refuse is no control run
    Given a fixture whose missingValue invocation is absent, and one whose missingValue the tool does not refuse
    When atip check runs with --json
    Then L0.no-prompt is unprovided naming missingValue in the first and the control run in the second, and the level fails with exit 1

  # canon: {"schema":1,"at":"2026-11-01T07:27:45Z","actor":{"agent_id":"impl-atip-check-1-1","role":"implementer","session_id":"s-0-34-check-1-1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-1-1.F5
  @atip.check.64
  Scenario: L0.envelope fails when a document breaks the envelope schema, and not for what other checks own
    Given a fake tool whose envelopes carry an unknown meta member
    When atip check runs
    Then L0.envelope is fail and no other check fails; a null, a later-minor member or a code in the wrong case is judged by its own check, and a document that is no envelope (also one carrying an unregistered error.code) is judged by L0.envelope alone

  # canon: {"schema":1,"at":"2026-11-01T07:27:48Z","actor":{"agent_id":"impl-atip-check-1-1","role":"implementer","session_id":"s-0-34-check-1-1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-1-1.F5
  @atip.check.65
  Scenario: L0.contract-version fails on a missing or wrong marker, or a member of a later minor than the claim
    Given fake tools whose answers lack the 1.1 markers, say another minor, or (claiming 1.0) carry a 1.1 member or the wrong schemaVersion in a schema answer
    When atip check runs
    Then L0.contract-version is fail naming the condition and no other check fails

  # canon: {"schema":1,"at":"2026-11-01T07:27:52Z","actor":{"agent_id":"impl-atip-check-1-1","role":"implementer","session_id":"s-0-34-check-1-1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-1-1.F5
  @atip.check.66
  Scenario: L0.stderr-result fails when stderr carries the error or the result
    Given a fake tool that prints its error as an envelope on stderr
    When atip check runs
    Then L0.stderr-result is fail and no other check fails

  # canon: {"schema":1,"at":"2026-11-01T07:27:56Z","actor":{"agent_id":"impl-atip-check-1-1","role":"implementer","session_id":"s-0-34-check-1-1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-1-1.F5
  @atip.check.67
  Scenario: L0.stderr-quiet reports an advisory when stderr is not empty in JSON mode
    Given a fake tool that prints a progress line on stderr in JSON mode
    When atip check runs
    Then L0.stderr-quiet is fail, a SHOULD, so the level is met with the warning SHOULD_FAILED

  # canon: {"schema":1,"at":"2026-11-01T07:28:00Z","actor":{"agent_id":"impl-atip-check-1-1","role":"implementer","session_id":"s-0-34-check-1-1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-1-1.F5
  @atip.check.68
  Scenario: L0.no-prompt fails when a missing value makes the tool wait on a terminal, and one that refuses passes
    Given fake tools that read a line, print a prompt in raw mode, or react to end-of-file on a terminal when a value is missing, and one that refuses; and tools that sleep or ignore input without printing
    When atip check runs the missingValue invocation under its control run and on a terminal
    Then the waiting tools fail L0.no-prompt, the refusing tool passes, and a silent slow tool is unprovided

  # canon: {"schema":1,"at":"2026-11-01T07:28:04Z","actor":{"agent_id":"impl-atip-check-1-1","role":"implementer","session_id":"s-0-34-check-1-1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-1-1.F5
  @atip.check.69
  Scenario: L0.registry fails when the registries the tool prints, or the codes, reasons and warnings it emits, disagree with the contract or the fixture
    Given fake tools whose registries differ from the fixture's, whose entries break the entry schemas, whose answers differ from the list, or that emit a code, a reason or a warning no registry has
    When atip check runs
    Then L0.registry is fail naming the difference and no other check fails; a tool without schema is unsupported

  # canon: {"schema":1,"at":"2026-11-01T07:28:08Z","actor":{"agent_id":"impl-atip-check-1-1","role":"implementer","session_id":"s-0-34-check-1-1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-1-1.F5
  @atip.check.70
  Scenario: L0.usage-hint reports an advisory for a USAGE without a field whose hint is not the command's schema
    Given a fake tool whose USAGE without a field has no hint naming the command's schema
    When atip check runs
    Then L0.usage-hint is fail, a SHOULD, so the level is met

  # canon: {"schema":1,"at":"2026-11-01T07:28:11Z","actor":{"agent_id":"impl-atip-check-1-1","role":"implementer","session_id":"s-0-34-check-1-1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-1-1.F5
  @atip.check.71
  Scenario: L0.hint-form fails when a hint that starts with the tool name is not one command line, for a 1.1 claim only
    Given a fake tool whose hint starts with its name and is prose, claiming 1.1
    When atip check runs
    Then L0.hint-form is fail and no other check fails

  # canon: {"schema":1,"at":"2026-11-01T07:28:15Z","actor":{"agent_id":"impl-atip-check-1-1","role":"implementer","session_id":"s-0-34-check-1-1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-1-1.F5
  @atip.check.72
  Scenario: L0.global-on-group reports an advisory when a tool-wide option after a group word is an error
    Given a fake tool whose group word rejects a tool-wide option
    When atip check runs with the fixture's globalOptions
    Then L0.global-on-group is fail, a SHOULD, so the level is met

  # canon: {"schema":1,"at":"2026-11-01T07:28:19Z","actor":{"agent_id":"impl-atip-check-1-1","role":"implementer","session_id":"s-0-34-check-1-1"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-34-atip-check-1-1.F3
  @atip.check.73
  Scenario: Whether a tool pages is decided by asking each command of the fixture for a cursor, not by the fixture
    Given a fake tool that pages but whose fixture lists no paging call, one whose paging command is listed without an ok invocation, one that refuses --cursor on every command, and a fixture without commands
    When atip check runs
    Then L0.page is unprovided for the first, the second (also when its cursor probe is inconclusive, as when the command needs an operand first) and the fourth, and unsupported only for the third

  # canon: {"schema":1,"at":"2026-11-01T07:28:23Z","actor":{"agent_id":"impl-atip-check-1-1","role":"implementer","session_id":"s-0-34-check-1-1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-1-1.F3
  @atip.check.74
  Scenario: A refusal that changes what the probe reports fails L0.refusal-effects
    Given a fake tool whose refusal changes the state the probe reports
    When atip check runs
    Then L0.refusal-effects is fail naming what the probe then reports

  # canon: {"schema":1,"at":"2026-11-01T07:59:46Z","actor":{"agent_id":"impl-atip-check-1-1","role":"implementer","session_id":"s-0-34-check-1-1"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-34-atip-check-1-1.F6
  @atip.check.75
  Scenario: A canon whose hint is prose starting with its name fails L0.hint-form
    Given canon wrapped so that every hint of its errors is prose that starts with `canon ` and is not one command line
    When atip check runs at L0 with the committed fixture
    Then L0.hint-form is fail naming the hint and no other check fails, so the CI job goes red
