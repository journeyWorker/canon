Feature: authoring change
  # canon: {"schema":1,"at":"2026-10-10T15:34:48Z","actor":{"agent_id":"canon"}}

  # canon: {"schema":1,"at":"2026-10-10T15:34:48Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.change.01
  Scenario: A new change is scaffolded, imported and adopted with no config edit
    Given a repo set up by canon init alone and a subject
    When canon change new is run with a slug, the subject and a title
    Then openspec/changes/<slug>/proposal.md and tasks.md are written, the title as the proposal's Why
    And the change is imported and carries the subject, and the subject lists the change
    And canon.yaml is byte-identical
    And a task row added to tasks.md imports as <slug>#1

  # canon: {"schema":1,"at":"2026-10-10T15:34:48Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.change.02
  Scenario: A change for an unknown subject is refused and nothing is written
    Given a repo set up by canon init
    When canon change new names a subject that does not exist
    Then it exits 2 naming the subject
    And no change directory and no change record is written

  # canon: {"schema":1,"at":"2026-10-10T15:34:48Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.change.03
  Scenario: A slug that already exists is refused and the existing change is untouched
    Given a change already created under a slug, or a change directory written by hand
    When canon change new is run with that slug again
    Then it exits 2 saying it already exists
    And the existing proposal is byte-identical on both paths
    And no file is added to the hand-written directory and no change record is written for it

  # canon: {"schema":1,"at":"2026-10-10T15:34:48Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.change.04
  Scenario: A repo without an openspec plans source is refused
    Given a canon.yaml whose plans sources hold no openspec source
    When canon change new is run
    Then it exits 2 naming the missing openspec source and what to add
    And no openspec directory is created

  # canon: {"schema":1,"at":"2026-10-10T15:51:40Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.change.05
  Scenario: A change whose later steps fail leaves nothing behind
    Given a repo where the change dir cannot be created, or where the change record cannot be written
    When canon change new is run
    Then it exits 2 and reports nothing as written
    And no change dir, no staging dir under .canon, no change record and no subject link remain
