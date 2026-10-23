Feature: inventory sync
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:inventory-sync
  @case:happy
  @inventory.sync.01
  Scenario: A clean root materializes one index record per scenario carrying its title and a digest of its source
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a spec root holding one well-formed tagged scenario
    When the root is synced
    Then the outcome is clean and exactly one record was written
    And that record's project id is the root's declared id, not the checkout directory name
    And its scenario id and title are the ones the feature file declares
    And its source digest is a hash of that feature file's raw bytes

  @subject:inventory-sync
  @case:failure
  @inventory.sync.02
  Scenario: A validation violation aborts the whole root and writes nothing for it
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a spec root whose feature file omits its provenance comments
    When the root is synced
    Then the outcome is unclean and the root reports the violation
    And zero index records exist for that root
    And nothing was written and then rolled back, because validation precedes materialization

  @subject:inventory-sync
  @case:failure
  @inventory.sync.03
  Scenario: A duplicate scenario id aborts only its own root while a clean sibling still materializes
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given one root where two well-formed feature files declare the same scenario id
    And a second, clean root alongside it
    When both are synced in one pass
    Then the duplicate root writes zero records and reports a sync-level error
    And it reports no format violation, because a duplicate id is not a member of the frozen failure class set
    And the clean sibling still writes its one record
    And the whole run reports unclean rather than failing outright

  @subject:inventory-sync
  @case:edge
  @inventory.sync.04
  Scenario: Re-syncing an unchanged corpus writes nothing
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a root already synced once
    When it is synced again with no file changed
    Then zero new records are written
    And exactly one record still stands for that scenario
    And so idempotence is decided by the digest and title already recorded, not by a timestamp

  @subject:inventory-sync
  @case:edge
  @inventory.sync.05
  Scenario: Two roots declaring the same scenario id stay distinct records
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository configuring two named spec roots
    And each root declaring a scenario under the identical scenario id
    When both are synced
    Then two records are written, one per root
    And they are told apart by their project ids, because the index key is the pair, not the scenario id alone

  @subject:inventory-sync
  @case:edge
  @inventory.sync.06
  Scenario: The index derives from the feature corpus alone and never from an inventory directory
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a root holding one tagged scenario and a well-formed inventory directory naming coverage for it
    When the root is synced
    Then exactly one record is written, from the feature file
    And the inventory file produces no record of its own
    And the written record carries no coverage or surface reference field at all

  @subject:inventory-sync
  @case:happy
  @inventory.sync.07
  Scenario: A subject tag on a scenario resolves the subject join on its record
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a scenario tagged with a single well-formed subject slug
    When the root is synced
    Then the outcome is clean and no tag diagnostic is raised
    And the materialized record carries that subject as its join
    And an untagged scenario instead leaves the join unset with no key on the wire

  @subject:inventory-sync
  @case:edge
  @inventory.sync.08
  Scenario: Two subject tags on one scenario resolve to the first and are counted once
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a scenario carrying two subject tags
    When the root is synced
    Then the record joins to the first tag
    And exactly one diagnostic is counted, naming the tag that won
    And the root still syncs clean, because an over-tagged scenario is fail-soft, never an abort

  @subject:inventory-sync
  @case:failure
  @inventory.sync.09
  Scenario: A malformed subject tag drops the join but keeps the scenario indexed
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a scenario tagged with a value that is not a kebab-case subject slug
    When the root is synced
    Then the scenario is still indexed and the root is still clean
    And exactly one diagnostic is counted, quoting the offending value
    And the record's subject join is unset rather than holding an unparseable id

  # canon: {"schema":1,"at":"2026-09-16T11:04:54Z","actor":{"agent_id":"canon"}}
  @subject:inventory-sync
  @lane:behavior
  @case:happy
  @inventory.sync.10
  Scenario: A lane tag becomes the scenario's lane on the index record
    Given a scenario tagged with one lane
    When the root is synced
    Then the materialized record carries that lane
    And the sync is clean with no diagnostic

  # canon: {"schema":1,"at":"2026-09-16T11:04:54Z","actor":{"agent_id":"canon"}}
  @subject:inventory-sync
  @lane:behavior
  @case:edge
  @inventory.sync.11
  Scenario: An unrecognized tag namespace is counted and never aborts the root
    Given a scenario carrying a namespaced tag canon does not recognize
    When the root is synced
    Then the scenario is still indexed
    And exactly one diagnostic names the namespace, once per scenario and namespace
    And the root's sync exits clean

  # canon: {"schema":1,"at":"2026-10-08T14:02:46Z","actor":{"agent_id":"canon"}}
  @subject:inventory-sync
  @case:failure
  @inventory.sync.12
  Scenario: A case outside the repo vocabulary is dropped with a counted diagnostic
    Given a repository whose case vocabulary is happy, failure and edge
    And a scenario tagged with a case outside that vocabulary
    When the corpus is synced
    Then the scenario is indexed without a case and the sync reports a diagnostic naming the tag
    And a previously recorded case is not carried over to the new generation

  # canon: {"schema":1,"at":"2026-10-11T15:01:53Z","actor":{"agent_id":"canon"}}
  @subject:inventory-sync
  @case:happy
  @inventory.sync.40
  Scenario: A writer refreshes a stale scenario index once and names the files, and a reader never does
    Given a scenario written by scenario new and never synced
    When canon status runs with --json, and then canon artifacts store runs
    Then status writes no index record and prints no refresh line, and its stdout stays JSON that counts the scenario
    And exactly one stderr line from canon artifacts store names the changed feature file and the records written
    And the next writer is silent

  # canon: {"schema":1,"at":"2026-10-11T15:01:53Z","actor":{"agent_id":"canon"}}
  @subject:inventory-sync
  @case:happy
  @inventory.sync.41
  Scenario: Review scenario and subject move see a scenario that was never synced
    Given a scenario written by scenario new and never synced
    When canon review scenario and canon subject move run
    Then each refreshes the index first, says so in one line and succeeds without inventory sync

  # canon: {"schema":1,"at":"2026-10-11T15:01:53Z","actor":{"agent_id":"canon"}}
  @subject:inventory-sync
  @case:failure
  @inventory.sync.42
  Scenario: A root that cannot sync is named and a directory without canon.yaml is never written
    Given a feature file whose provenance comments are missing
    When canon artifacts store runs
    Then one stderr line names the root that was not refreshed and points at inventory sync
    And no scenario record is written
    And commands run in a directory without canon.yaml refuse with exit 2 and write nothing

  # canon: {"schema":1,"at":"2026-10-11T15:45:12Z","actor":{"agent_id":"canon"}}
  @subject:inventory-sync
  @case:happy
  @inventory.sync.43
  Scenario: A deleted scenario is marked removed the same way by a writer's refresh and by a full sync
    Given two indexed scenarios and the feature file of one of them deleted
    When one repository runs a writer such as canon artifacts store and an identical one runs inventory sync
    Then both append the same record marking the deleted scenario removed
    And the gate and query no longer report it
    And adding the scenario back makes the gate grade it again

  # canon: {"schema":1,"at":"2026-10-11T15:45:12Z","actor":{"agent_id":"canon"}}
  @subject:inventory-sync
  @case:failure
  @inventory.sync.44
  Scenario: A root that cannot sync marks no scenario removed
    Given an indexed scenario whose feature file is deleted
    And a remaining feature file whose provenance comments are missing
    When canon artifacts store and inventory sync run
    Then the root is named as not refreshed and inventory sync exits 1
    And no record marks the deleted scenario removed

  # canon: {"schema":1,"at":"2026-10-11T15:45:12Z","actor":{"agent_id":"canon"}}
  @subject:inventory-sync
  @case:edge
  @inventory.sync.45
  Scenario: Concurrent syncs and writers' refreshes write each index record once
    Given a stale scenario index and the index lock held by another process
    When two inventory syncs and two refreshing writers start
    Then none of them writes while the lock is held
    And once it is released exactly one of them writes one record per scenario

  # canon: {"schema":1,"at":"2026-10-11T16:04:57Z","actor":{"agent_id":"canon"}}
  @subject:inventory-sync
  @case:edge
  @inventory.sync.46
  Scenario: Record ingest names no scenario whose latest record marks it removed
    Given a titled scenario whose latest index record marks it removed
    When canon ingest records joins an event keyed to that scenario
    Then the event carries no scenario title in either record read order
    And once a later record adds the scenario back the event carries its title again

  # canon: {"schema":1,"at":"2026-10-12T04:08:25Z","actor":{"agent_id":"canon"}}
  @subject:inventory-sync
  @case:failure
  @inventory.sync.47
  Scenario: A root the refresh cannot index fails the gate, refuses shipping and holds status on the sync
    Given a verifying subject whose only scenario is indexed and evidenced, so it could ship
    And its feature file is then edited and loses a provenance comment, so the refresh refuses the root
    When canon gate check runs
    Then it exits 1 with an uncovered-cell violation naming the root and saying canon inventory sync lists the problems
    And canon subject move to shipped is refused with exit 1 by the same violation, and the subject stays verifying
    And canon status's only next step is canon inventory sync, naming the root, with no step that moves a subject on
    And once the provenance comment is restored the gate has no such violation and the subject ships

  # canon: {"schema":1,"at":"2026-10-12T10:34:27Z","actor":{"agent_id":"canon"}}
  @subject:inventory-sync
  @case:failure
  @inventory.sync.48
  Scenario: Sync aborts a root still on the pre-0.17 layout and names canon migrate
    Given a configured spec root that still holds its feature files under features/kind=feature/
    When canon inventory sync runs
    Then the root is ABORTED with one layout-grammar violation naming the root and canon migrate
    And the sync exits 1 and writes no scenario record

  # canon: {"schema":1,"at":"2026-10-12T10:34:27Z","actor":{"agent_id":"canon"}}
  @subject:inventory-sync
  @case:failure
  @inventory.sync.49
  Scenario: A root still on the pre-0.17 layout fails the gate closed and status names canon migrate
    Given a configured spec root that still holds its feature files under features/kind=feature/, with or without its scenarios already indexed
    When canon gate check runs
    Then its refresh says the root is not refreshed, naming the root and canon migrate, and writes no record that marks a scenario removed
    And it exits 1 with an uncovered-cell violation naming the root and canon migrate
    And canon status's only next step is canon migrate
    And once the files move to kind=feature/ directly under the root the gate has no such violation

  # canon: {"schema":1,"at":"2026-10-22T04:49:45Z","actor":{"agent_id":"canon-impl-027"}}
  @subject:inventory-sync
  @case:edge
  @inventory.sync.50
  Scenario: Status and gate check grade a stale scenario index as a sync would leave it without writing it
    Given a .feature file that holds a scenario the scenario index does not
    When canon status and canon gate check run
    Then each answers as it would after canon inventory sync and writes no index record, no archive file and no refresh line
    And a command that records something, such as canon artifacts store, still refreshes the index and says so in one line

  # canon: {"schema":1,"at":"2026-10-23T11:58:30Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:inventory-sync
  @case:happy
  @inventory.sync.51
  Scenario: A dry run reports the records a sync would write and writes none
    Given a repository with a spec root holding one scenario the index does not hold
    When canon inventory sync is run with --dry-run
    Then it exits 0 and reports dryRun true and the one record it would write
    And the working tree and .canon are byte-identical afterwards
    And the same command without --dry-run then writes that record

  # canon: {"schema":1,"at":"2026-10-23T11:58:38Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:inventory-sync
  @case:failure
  @inventory.sync.52
  Scenario: A dry run refuses exactly as the real run would and writes nothing
    Given a repository whose configuration makes canon inventory sync refuse
    When canon inventory sync is run with --dry-run and without it
    Then both exit with the same code and the same error code
    And the working tree and .canon are byte-identical after the dry run
