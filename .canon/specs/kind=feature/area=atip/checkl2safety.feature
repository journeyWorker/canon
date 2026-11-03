Feature: ATIP: check L2 destructive safety, idempotency, async and effects
  # canon: {"schema":1,"at":"2026-11-02T10:34:20Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}

  # canon: {"schema":1,"at":"2026-11-02T11:48:42Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-35-check-l2-safety.F1
  @atip.checkl2safety.01
  Scenario: A conforming destructive command's preview has planDigest, effects and committed false and a commit next, changes nothing, and the preview with its arguments on stdin replays the same plan
    Given a conforming destructive command and its fixture preview
    When atip check runs at L2
    Then the preview has planDigest, effects and committed false, a commit next, changes nothing, and the stdin preview replays

  # canon: {"schema":1,"at":"2026-11-02T11:49:20Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-35-check-l2-safety.F1
  @atip.checkl2safety.02
  Scenario: A preview that writes a file fails only L2.destructive-preview and names the path, and a commit option that is not flag-only or a batch fails only L2.destructive-safety
    Given a preview that writes a file, a commit option that is not flag-only, a batch
    When atip check runs at L2
    Then each fails only its row, the preview names the path, and L0.refusal-effects is untouched

  # canon: {"schema":1,"at":"2026-11-02T11:50:40Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-35-check-l2-safety.F2
  @atip.checkl2safety.03
  Scenario: A digest that differs in one character is PLAN_STALE with the preview as next[0], and the right commit says committed true and changes the snapshot
    Given the preview of a destructive command
    When the digest is changed by one character and the right commit runs in the preview's sequence
    Then PLAN_STALE, exit 2, next[0] the preview; and the commit says committed true and changes the snapshot

  # canon: {"schema":1,"at":"2026-11-02T11:51:58Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-35-check-l2-safety.F2
  @atip.checkl2safety.04
  Scenario: A tool that accepts a stale digest or commits on --commit alone fails only its row, and a refusal with the code of the requires rule passes
    Given a tool that accepts a stale digest or commits on --commit alone, and one that refuses with the code of the requires rule
    When atip check runs at L2
    Then the first two fail their rows alone and the third passes

  # canon: {"schema":1,"at":"2026-11-02T11:53:06Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-35-check-l2-safety.F3
  @atip.checkl2safety.05
  Scenario: The three idempotency values pass for a conforming tool and appends is not tested
    Given write commands of the three idempotency values with fixture ok invocations
    When atip check runs at L2
    Then each is run twice in one sequence and passes, and appends is not tested

  # canon: {"schema":1,"at":"2026-11-02T11:54:05Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-35-check-l2-safety.F3
  @atip.checkl2safety.06
  Scenario: A second run with a new effect, a once that is accepted twice, and a write the fixture does not list is named and never run
    Given a write with a second run that has a new effect, a once that is accepted twice, and a write the fixture does not list
    When atip check runs at L2
    Then the first two fail only L2.idempotency and the third is named unlisted and never run

  # canon: {"schema":1,"at":"2026-11-02T11:54:34Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-35-check-l2-safety.F4
  @atip.checkl2safety.07
  Scenario: A --dry-run, --help or --version that writes fails L2.effects as a MUST at L2, and a read change that matches allowedEffects passes
    Given a --dry-run, --help or --version that writes, and a read whose change matches allowedEffects
    When atip check runs at L2
    Then the writes fail L2.effects as a MUST and the allowed read change passes

  # canon: {"schema":1,"at":"2026-11-02T11:55:01Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-35-check-l2-safety.F4
  @atip.checkl2safety.08
  Scenario: At L0 and L1 L2.effects is an advisory and its failure leaves the level met
    Given the same failures at L0 and L1
    When atip check runs at L0 and at L1
    Then L2.effects is an advisory and the level is met

  # canon: {"schema":1,"at":"2026-11-02T11:55:28Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-35-check-l2-safety.F4
  @atip.checkl2safety.09
  Scenario: A read change outside allowedEffects fails and the report names the path, and a read without an effectNote may change nothing
    Given a read change outside allowedEffects, and a read without an effectNote
    When atip check runs at L2
    Then L2.effects fails and names the path

  # canon: {"schema":1,"at":"2026-11-02T11:55:54Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-35-check-l2-safety.F5
  @atip.checkl2safety.10
  Scenario: The worker is driven to done, failed and timeout and the wait exits as the contract says
    Given an async start and a controllable worker
    When atip check runs at L2 with the worker at done, failed and timeout
    Then the waits exit 0, 1 with a registered code, and 1 retryable with the same wait as next[0]

  # canon: {"schema":1,"at":"2026-11-02T11:56:09Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-35-check-l2-safety.F5
  @atip.checkl2safety.11
  Scenario: The same operation key returns the same work, two concurrent starts start it once, and another payload is OPERATION_REUSED
    Given an async start with --operation
    When atip check runs at L2
    Then the same key returns the same work, two concurrent starts start it once, and another payload is OPERATION_REUSED

  # canon: {"schema":1,"at":"2026-11-02T11:56:39Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-35-check-l2-safety.F5
  @atip.checkl2safety.12
  Scenario: A tool with no destructive or async command has those rows unsupported, and a start that claims the work done fails L2.async
    Given a tool with no destructive or async command, and a start that claims the work done
    When atip check runs at L2
    Then those rows are unsupported, and the start fails L2.async

  # canon: {"schema":1,"at":"2026-11-02T11:56:58Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-35-check-l2-safety.F6
  @atip.checkl2safety.13
  Scenario: Every condition of the safety rows has a seeded case that fails its owner alone, and the conforming tool judges each
    Given the seeded cases of every condition of the safety rows
    When atip check runs at L2 with each case, with its condition off, and with its row forced to pass
    Then the owner alone fails and the conforming tool judges every condition

  # canon: {"schema":1,"at":"2026-11-02T11:57:17Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-35-check-l2-safety.F6
  @atip.checkl2safety.14
  Scenario: The conforming fake and the checker itself pass, and a row forced to pass makes its seeded cases fail
    Given the conforming fake and the checker itself
    When atip check runs at L2 on the fake and at L0 on atip
    Then both pass
