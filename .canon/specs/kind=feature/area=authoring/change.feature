Feature: authoring change
  # canon: {"schema":1,"at":"2026-10-10T15:34:48Z","actor":{"agent_id":"canon"}}

  # canon: {"schema":1,"at":"2026-10-10T15:34:48Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:happy
  @authoring.change.01
  Scenario: A new change is written in canon's own changes dir, imported and adopted with no plans source
    Given a repo set up by canon init alone, whose canon.yaml lists no plans source, and a subject
    When canon change new is run with a slug, --subject and --title
    Then .canon/changes/<slug>/brief.md is written, the title as the brief's Intent, and no tasks.md is written
    And no openspec directory is created and canon.yaml is byte-identical
    And the change is imported with canon's own provenance and carries the subject, and the subject lists the change
    And a tasks.md written by hand imports its task row as <slug>#1, and canon task done <slug>#1 flips it there

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
    Given a change already created under a slug, or a directory under .canon/changes written by hand
    When canon change new is run with that slug again
    Then it exits 2 saying it already exists
    And the existing brief is byte-identical on both paths
    And no file is added to the hand-written directory and no change record is written for it

  # canon: {"schema":1,"at":"2026-10-10T15:51:40Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.change.05
  Scenario: A change that fails before any record is written leaves nothing behind
    Given a repo where the change dir under .canon/changes cannot be created, or where the first record, the subject, cannot be written
    When canon change new is run
    Then it exits 2 and reports nothing as written
    And no change dir, no staging dir under .canon, no change record and no subject link remain

  # canon: {"schema":1,"at":"2026-10-10T16:01:46Z","actor":{"agent_id":"canon"}}
  @subject:authoring-scaffold
  @case:failure
  @authoring.change.06
  Scenario: A change whose second record write fails keeps one record and prints the repair
    Given a repo where the subject record can be written but the change record cannot
    When canon change new is run
    Then it exits 1 with CHANGE_ADOPTION_INCOMPLETE, keeping the change dir and the subject record that now lists the change
    And it prints the command that completes the link: canon ingest plans, then canon subject adopt <subject> --change <slug>
    And running that command completes the link, and running the adopt again writes nothing

  # canon: {"schema":1,"at":"2026-10-12T12:00:00Z","actor":{"agent_id":"017-p2-impl"}}
  @subject:authoring-scaffold
  @case:edge
  @authoring.change.07
  Scenario: A configured external openspec source is never written into by change new
    Given a canon.yaml whose plans sources hold an external openspec source with a human-written change
    When canon change new is run
    Then the change is written under .canon/changes and nothing is written under the external source
    And the external change's proposal is byte-identical
    And canon ingest plans imports both, the canon change with canon's provenance and the external one with the openspec dialect's

  # canon: {"schema":1,"at":"2026-10-27T12:04:48Z","actor":{"agent_id":"impl-031-bf","role":"implementer","session_id":"bf-2026-10-27"}}
  @subject:authoring-scaffold
  @case:happy
  @flow:canon-0-31-brief-format.F3
  @authoring.change.08
  Scenario: change new writes the title and empty headed sections, with no comment, for a feature and a chore
    Given a fresh repo, in English and with language ko, and a chore
    When canon change new writes the brief
    Then it holds the title, the English headings of the kind and the title under Intent, with no HTML comment and no approval note, and the chore has a frontmatter holding only kind: chore
    And the intent summary imported from it is the title
