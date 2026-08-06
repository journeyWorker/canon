Feature: gate task
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:gate-trust-spine
  @gate.task.01
  Scenario: A checkbox with no evidence record behind it does not flip
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a plan document carrying one open row and a ledger carrying no evidence for it
    When that task is asked to complete
    Then the command exits one and names unevidenced-flip
    And the row is left byte-unchanged, still open

  @subject:gate-trust-spine
  @gate.task.02
  Scenario: A matching non-divergent record flips the row, and a second run changes nothing
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a committed faithful evidence record for an open row
    When that task is asked to complete
    Then the command succeeds and the row is checked
    And a second identical call succeeds and leaves the document byte-identical
    And nothing about the second call depends on whether the first one ran

  @subject:gate-trust-spine
  @gate.task.03
  Scenario: A divergent verdict is not evidence of completion
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a committed evidence record for the row whose verdict is divergent
    When that task is asked to complete
    Then the command exits one
    And the row stays open, because a record saying the work did not hold is not a record saying it did

  @subject:gate-trust-spine
  @gate.task.04
  Scenario: Not-applicable passes the flip exactly as faithful does
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an evidence record for the task whose verdict is not-applicable
    When the flip is decided
    Then it is approved
    And only a divergent verdict is treated as no evidence

  @subject:gate-trust-spine
  @gate.task.05
  Scenario: A note carrying a fabrication marker blocks the flip it was meant to justify
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a faithful evidence record whose note is a placeholder promising to verify later
    When the flip is decided
    Then it is blocked
    And every violation raised is fabricated-evidence, so the refusal names the real reason

  @subject:gate-trust-spine
  @gate.task.06
  Scenario: A task id no plan row carries is reported rather than guessed at
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a plan document carrying row one and nothing else
    When completion is claimed for a row number that document does not have
    Then the command exits one and says there is no matching row
    And no row is flipped in place of the one that was named

  @subject:gate-trust-spine
  @gate.task.07
  Scenario: The flipped row carries the attested note, or a stated default when there is none
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a faithful evidence record whose note summarises a real command and its result
    When the flip is decided
    Then it is approved carrying that summary verbatim as the row's evidence note
    And a record with no note companion is approved carrying a default text that states the verdict instead

  @subject:gate-trust-spine
  @gate.task.08
  Scenario: A committed note spanning two lines cannot forge a second checked row
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a staged record whose note text carries a newline followed by a checked row of its own
    When it is promoted and the task is asked to complete
    Then promotion commits the record, because promotion validates records and not note shape
    And the flip is refused instead
    And the document carries no forged row and no checked row at all, with the real row still open
