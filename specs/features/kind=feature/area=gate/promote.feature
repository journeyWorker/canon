Feature: gate promote
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:gate-trust-spine
  @case:happy
  @gate.promote.01
  Scenario: Run sequence numbers are monotonic and gap-free per role and surface
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given two staged records for one role and one surface
    When they are promoted in a single call
    Then they are numbered one and two, strictly increasing with no gap
    And staging is drained and both land committed, each carrying its own number
    And a third record for that same role and surface, promoted separately, continues at three rather than restarting

  @subject:gate-trust-spine
  @case:happy
  @gate.promote.02
  Scenario: Two surfaces number themselves independently
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given two staged records for one role on two different surfaces
    When they are promoted in a single call
    Then both are numbered one
    And the sequence is per surface, not a single counter shared across the ledger

  @subject:gate-trust-spine
  @case:failure
  @gate.promote.03
  Scenario: A malformed candidate is refused and consumes no run sequence number
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a staged body missing a required field, beside a well-formed sibling for the same role and surface
    When they are promoted
    Then the sibling is numbered one, so the malformed body burned no number
    And the malformed body is refused as malformed-evidence
    And its staging file is left on disk, never committed and never deleted

  @subject:gate-trust-spine
  @case:failure
  @gate.promote.04
  Scenario: A candidate whose partition cannot be derived is refused rather than filed somewhere
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a well-formed staged record whose actor names no role
    When it is promoted
    Then nothing is promoted and it is refused as malformed-evidence
    And the committed tier stays empty, because a record with no partition has no sequence to join

  @subject:gate-trust-spine
  @case:happy
  @gate.promote.05
  Scenario: A dry run computes the whole plan and writes nothing
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given one staged record
    When promotion runs as a dry run
    Then it reports the record it would promote and the number it would assign
    And the committed tier is still empty
    And the staging file is still there, undeleted

  @subject:gate-trust-spine
  @case:edge
  @gate.promote.06
  Scenario: A promote retried after an interruption recovers instead of appending a duplicate
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a record already committed by a promotion whose staging removal never happened
    When promotion is retried against that surviving staging file
    Then the retry exits clean, reporting a recovery rather than a promotion
    And it reports the number and committed location the interrupted call already assigned
    And exactly one committed record survives, and staging is drained
    And a genuinely different record for the same role and surface still promotes, at the next number

  @subject:gate-trust-spine
  @case:happy
  @gate.promote.07
  Scenario: The committed record carries the staged identity it was promoted under
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a staged record that is promoted
    When the committed ledger is read back
    Then the committed record carries the digest of the staged body it came from
    And recovery therefore works from the ledger alone, surviving the process that was interrupted

  @subject:gate-trust-spine
  @case:failure
  @gate.promote.08
  Scenario: Two different records at one natural key commit one and refuse the other
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given two staged findings that share a natural key but carry different summaries, so their staged paths differ
    When they are promoted
    Then exactly one becomes durable and the other is refused, not silently dropped
    And the refusal names the key and says two findings under one identity is the harm
    And the committed corpus holds one record for that key
    And the loser stays staged, byte-unmodified and distinguishable from the committed one, for a human to renumber
