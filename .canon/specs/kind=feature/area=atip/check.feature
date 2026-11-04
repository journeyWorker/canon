Feature: atip check: the atip/1 L0 conformance checker
  # canon: {"schema":1,"at":"2026-10-31T11:45:03Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}

  # canon: {"schema":1,"at":"2026-10-31T11:47:00Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-33-atip-check.F6
  @atip.check.39
  Scenario: The canon fixture's registry and warnings equal the ones generated from canon schema
    Given canon built from this checkout
    When the fixture's registry and warnings are compared with canon schema --errors --json and --warnings --json
    Then they are equal by code, exit, retryable and reason names, and the fixture's keys are only contractVersion, setup, probe, globalOptions, registry, warnings and commands (a paging call, a next-step call and a missingValue)

  # canon: {"schema":1,"at":"2026-10-31T11:47:03Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-contract
  @lane:behavior
  @case:failure
  @flow:canon-0-33-atip-check.F6
  @atip.check.40
  Scenario: A stale registry in the canon fixture fails the generation test
    Given a canon fixture with a row dropped, an exit changed, a retryable flipped, a member of prose added, a warning dropped, a call changed or a key added
    When the generation test compares it with the generated one
    Then each difference is noticed

  # canon: {"schema":1,"at":"2026-10-31T11:47:19Z","actor":{"agent_id":"impl-atip-check","role":"implementer","session_id":"s-0-33-atip-check"}}
  @subject:atip-contract
  @lane:behavior
  @case:happy
  @flow:canon-0-33-atip-check.F6
  @atip.check.43
  Scenario: The CI job and the just recipe run atip check against the canon binary with the same command
    Given the workflow job atip and the just recipes atip-deep, atip and atip-install
    When their commands are read
    Then the job and the recipe atip-deep install the checker from the atip repository at its tag, build canon-cli only, and run atip check once on the canon binary at L2 with --contract 1.4, --deep and the canon fixture
    And the recipe atip runs the same command without --deep
