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
    Then they are equal by code, exit, retryable and reason names, and the fixture's keys are only commands, contractVersion, globalOptions, notRun, prepare, probe, registry, setup, timeoutSeconds and warnings, with the declared contract minor, probe status and global option --repo . (a paging call, a next-step call and a missingValue under commands)

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
