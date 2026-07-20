## ADDED Requirements

### Requirement: The eight join keys are typed, not bare strings
`canon-model` SHALL express each of the eight join-spine keys
(`change_id`, `task_id`, `scenario_id`, `session_id`, `run_id`,
`handoff_id`, `sha`/`pr`, `regime_key`) as a distinct newtype with its
grammar documented on the type, and every record field that carries a join
key SHALL use that newtype rather than a bare `String`.

#### Scenario: Cross-kind join keys share one type per key
- **WHEN** two different record kinds (e.g. `Task` and `EvidenceRecord`)
  each reference the same key (`task_id`)
- **THEN** both fields use the identical newtype (`TaskId`), so a
  compile-time type mismatch — not a runtime string-format bug — catches
  any attempt to join on a wrongly-shaped value.

#### Scenario: `scenario_id` grammar is enforced and never renumbered
- **WHEN** a `ScenarioId` is constructed from a string
- **THEN** construction validates the `<area>.<surface>.<nn>` grammar and
  rejects malformed input; nothing in `canon-model` provides a mechanism to
  renumber an existing `scenario_id`'s `<nn>` component.

### Requirement: Generated join-spine documentation
The repo SHALL provide a join-spine document generated from the same
source as the join-key newtypes (never hand-authored prose describing the
keys separately from their types).

#### Scenario: Generated doc matches the eight-key table
- **WHEN** the join-spine doc generator runs
- **THEN** its output contains exactly the eight keys above, each with the
  grammar and "joins" description sourced from that key's newtype doc
  comment, and the generation step is part of `canon fmt --check`/`canon
  selftest`'s diff-against-committed-output check.

#### Scenario: A key's grammar change updates the doc in the same build
- **WHEN** a join-key newtype's grammar doc comment changes
- **THEN** regenerating the join-spine doc reflects the new grammar with no
  second edit required anywhere else.
