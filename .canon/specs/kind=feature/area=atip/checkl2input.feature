Feature: ATIP: check L2 input and batches
  # canon: {"schema":1,"at":"2026-11-02T10:34:18Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}

  # canon: {"schema":1,"at":"2026-11-02T10:35:35Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-35-check-l2-input.F1
  @atip.checkl2input.01
  Scenario: atip check --level L2 runs the L0 and L1 checks and the 15 input and batch rows, and a conforming tool meets the level; --level L3 is USAGE
    Given a conforming L2 fake tool and its fixture
    When atip check runs at L2 and at L3
    Then the 15 input and batch rows run, the tool meets the level, and L3 is USAGE

  # canon: {"schema":1,"at":"2026-11-02T10:37:38Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-35-check-l2-input.F1
  @atip.checkl2input.02
  Scenario: A tree whose level is L1 fails L1.tree at L2, and a write command whose document has no input fails L2.input-present
    Given the L1 fake whose tree says L1, and an L2 fake whose write command has no input
    When atip check runs at L2
    Then L1.tree fails for the level, and L2.input-present fails for the write without input

  # canon: {"schema":1,"at":"2026-11-02T10:38:11Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-35-check-l2-input.F2
  @atip.checkl2input.03
  Scenario: L2.input-present and L2.input-example judge the documents alone, and each of their conditions fails only its owner
    Given a seeded violation of each condition of L2.input-present and L2.input-example
    When atip check runs at L2
    Then each fails only its owner

  # canon: {"schema":1,"at":"2026-11-02T10:38:28Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-35-check-l2-input.F2
  @atip.checkl2input.04
  Scenario: Every refusal document is refused with exit 2, INPUT_INVALID or INPUT_TOO_LARGE and a pointer or --input, the two-key document with two details
    Given the refusal documents of the input rows
    When atip check runs at L2
    Then each is refused with exit 2, INPUT_INVALID or INPUT_TOO_LARGE and a pointer or --input, and the two-key document has two details

  # canon: {"schema":1,"at":"2026-11-02T10:38:54Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-35-check-l2-input.F2
  @atip.checkl2input.05
  Scenario: A tool that accepts {"commit":true} fails only L2.input-refusals, and in a 1.2 claim so does one that accepts {"limit":1}
    Given a tool that accepts a flag-only key in a document
    When atip check runs at L2
    Then only L2.input-refusals fails

  # canon: {"schema":1,"at":"2026-11-02T10:39:08Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-35-check-l2-input.F2
  @atip.checkl2input.06
  Scenario: With stdin on a pty --input - is refused INPUT_INVALID on --input, and each condition of L2.input-tty fails only it
    Given a tool whose --input - on a terminal is not refused on --input
    When atip check runs at L2
    Then only L2.input-tty fails, and each of its conditions fails alone

  # canon: {"schema":1,"at":"2026-11-02T10:39:28Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-35-check-l2-input.F2
  @atip.checkl2input.07
  Scenario: L2.input-usage is a MUST: --input twice or as the last word is USAGE, never INPUT_INVALID
    Given a tool that answers --input twice or --input last as INPUT_INVALID, or not USAGE
    When atip check runs at L2
    Then only L2.input-usage fails, a MUST

  # canon: {"schema":1,"at":"2026-11-02T10:39:50Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-35-check-l2-input.F2
  @atip.checkl2input.08
  Scenario: L2.input-reasons is advisory: a framing refusal without the reserved reason of 7.2.3 fails it and the level is met
    Given a tool whose framing refusals lack the reserved reason
    When atip check runs at L2
    Then L2.input-reasons fails as an advisory and the level is met

  # canon: {"schema":1,"at":"2026-11-02T10:39:58Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-35-check-l2-input.F3
  @atip.checkl2input.09
  Scenario: A document and argv split three ways and with integers as numeric strings give the result of the argv form, and each seeded violation fails only its row
    Given a document and argv split three ways, and integers as numeric strings
    When atip check runs at L2
    Then the result is the argv form's, and each seeded violation fails only its row

  # canon: {"schema":1,"at":"2026-11-02T10:41:53Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-35-check-l2-input.F3
  @atip.checkl2input.10
  Scenario: A different value is INPUT_CONFLICT with the document pointer, the same value written another way succeeds: a refused member order, 1.0 or -0 fails only L2.input-conflict
    Given a different value in a document, and the same value written another way
    When atip check runs at L2
    Then the different value is INPUT_CONFLICT on the pointer and the same value succeeds

  # canon: {"schema":1,"at":"2026-11-02T10:42:31Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-35-check-l2-input.F3
  @atip.checkl2input.11
  Scenario: An env value that is applied before the merge and refuses a document that gives the key fails only L2.input-env
    Given an env value that is invalid and a document that gives the option
    When atip check runs at L2
    Then the document wins and only L2.input-env fails when the env applies first

  # canon: {"schema":1,"at":"2026-11-02T10:42:57Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-35-check-l2-input.F3
  @atip.checkl2input.12
  Scenario: A broken constraint and a required key left out of both are refused with the declared code or USAGE and a document pointer, and INPUT_INVALID for the missing key fails only L2.input-pointer
    Given a broken constraint and a required key left out
    When atip check runs at L2
    Then each is refused with the declared code or USAGE on a document pointer, and only L2.input-pointer fails otherwise

  # canon: {"schema":1,"at":"2026-11-02T10:44:07Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-35-check-l2-input.F3
  @atip.checkl2input.13
  Scenario: A document value that starts with - { or [ is a value, and a tool that reads it as an option fails only L2.input-dash-values
    Given values that start with a dash, a brace or a bracket in a document
    When atip check runs at L2
    Then they are values, and only L2.input-dash-values fails when they are read as options

  # canon: {"schema":1,"at":"2026-11-02T10:44:27Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-35-check-l2-input.F3
  @atip.checkl2input.14
  Scenario: L1.numbers also gives 9007199254740992 and its negation in a document as a number and as a string, and a rounded value fails only L1.numbers
    Given an integer beyond the safe range as a number and as a string in a document
    When atip check runs at L2
    Then it is refused INPUT_INVALID, and a rounded value fails only L1.numbers

  # canon: {"schema":1,"at":"2026-11-02T10:44:50Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-35-check-l2-input.F4
  @atip.checkl2input.15
  Scenario: A conforming batch tool passes L2.batch-preflight, L2.batch-incomplete and L2.batch-ok with the counts, order and statuses of spec 11.11
    Given a conforming batch tool
    When atip check runs at L2
    Then the three batch rows pass with the counts, the order and the statuses of 11.11

  # canon: {"schema":1,"at":"2026-11-02T10:45:07Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-35-check-l2-input.F4
  @atip.checkl2input.16
  Scenario: A first item that fails with exit 2 fails only L2.batch-incomplete, and a last item refused in preflight as BATCH_INCOMPLETE fails only L2.batch-preflight
    Given a tool whose first item failure is refused with exit 2 and whose preflight refusal is BATCH_INCOMPLETE
    When atip check runs at L2
    Then each fails only its own batch row

  # canon: {"schema":1,"at":"2026-11-02T10:45:45Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-35-check-l2-input.F4
  @atip.checkl2input.17
  Scenario: A tool with no batch command has the three batch rows unsupported, and a command without input.batch is not given a batch
    Given a tool with no command that takes a batch
    When atip check runs at L2
    Then the three batch rows are unsupported

  # canon: {"schema":1,"at":"2026-11-02T10:46:26Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-35-check-l2-input.F5
  @atip.checkl2input.18
  Scenario: A fixture without batchFailsAt or batchFailsFirst makes L2.batch-incomplete unprovided and exits 1, and an observed violation is still a fail
    Given a fixture without batchFailsAt or batchFailsFirst
    When atip check runs at L2
    Then L2.batch-incomplete is unprovided and the exit is 1, and an observed violation is still a fail

  # canon: {"schema":1,"at":"2026-11-02T10:46:53Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-35-check-l2-input.F5
  @atip.checkl2input.19
  Scenario: A malformed batchFailsAt or batchFailsFirst entry is FIXTURE_INVALID with exit 2 and no binary runs
    Given a malformed batchFailsAt or batchFailsFirst entry
    When atip check runs
    Then FIXTURE_INVALID, WRONG_SHAPE, exit 2 and no binary runs

  # canon: {"schema":1,"at":"2026-11-02T10:47:28Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:edge
  @flow:canon-0-35-check-l2-input.F5
  @atip.checkl2input.20
  Scenario: A write command the fixture does not list is named under unlisted and never run by an L2 row
    Given a write command the fixture does not list
    When atip check runs at L2
    Then the input rows name it under unlisted and never run it

  # canon: {"schema":1,"at":"2026-11-02T10:48:00Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:happy
  @flow:canon-0-35-check-l2-input.F6
  @atip.checkl2input.21
  Scenario: Every pass condition of the 15 input and batch rows and of L1.numbers at L2 has a seeded case that fails its owner alone, and the conforming tool and the checker itself pass
    Given the seeded cases of every condition of the 15 rows
    When atip check runs at L2 with each case, with its condition off, and with its row forced to pass
    Then the owner alone fails, stops failing without the condition, and stops failing when forced, and the conforming tool judges every condition

  # canon: {"schema":1,"at":"2026-11-02T10:48:26Z","actor":{"agent_id":"impl-check-l2","role":"implementer","session_id":"s-0-35-check-l2"}}
  @subject:atip-check
  @lane:behavior
  @case:failure
  @flow:canon-0-35-check-l2-input.F6
  @atip.checkl2input.22
  Scenario: A row forced to pass makes the seeded case of its conditions fail, and a condition switched off stops its case failing
    Given the conforming fake and the checker itself
    When atip check runs at L2 on the fake and at L0 on atip
    Then both pass
