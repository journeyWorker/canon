Feature: inventory sync
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:inventory-sync
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
  @inventory.sync.02
  Scenario: A validation violation aborts the whole root and writes nothing for it
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a spec root whose feature file omits its provenance comments
    When the root is synced
    Then the outcome is unclean and the root reports the violation
    And zero index records exist for that root
    And nothing was written and then rolled back, because validation precedes materialization

  @subject:inventory-sync
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
  @inventory.sync.04
  Scenario: Re-syncing an unchanged corpus writes nothing
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a root already synced once
    When it is synced again with no file changed
    Then zero new records are written
    And exactly one record still stands for that scenario
    And so idempotence is decided by the digest and title already recorded, not by a timestamp

  @subject:inventory-sync
  @inventory.sync.05
  Scenario: Two roots declaring the same scenario id stay distinct records
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository configuring two named spec roots
    And each root declaring a scenario under the identical scenario id
    When both are synced
    Then two records are written, one per root
    And they are told apart by their project ids, because the index key is the pair, not the scenario id alone

  @subject:inventory-sync
  @inventory.sync.06
  Scenario: The index derives from the feature corpus alone and never from an inventory directory
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a root holding one tagged scenario and a well-formed inventory directory naming coverage for it
    When the root is synced
    Then exactly one record is written, from the feature file
    And the inventory file produces no record of its own
    And the written record carries no coverage or surface reference field at all

  @subject:inventory-sync
  @inventory.sync.07
  Scenario: A subject tag on a scenario resolves the subject join on its record
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a scenario tagged with a single well-formed subject slug
    When the root is synced
    Then the outcome is clean and no tag diagnostic is raised
    And the materialized record carries that subject as its join
    And an untagged scenario instead leaves the join unset with no key on the wire

  @subject:inventory-sync
  @inventory.sync.08
  Scenario: Two subject tags on one scenario resolve to the first and are counted once
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a scenario carrying two subject tags
    When the root is synced
    Then the record joins to the first tag
    And exactly one diagnostic is counted, naming the tag that won
    And the root still syncs clean, because an over-tagged scenario is fail-soft, never an abort

  @subject:inventory-sync
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
  @inventory.sync.10
  Scenario: A lane tag becomes the scenario's lane on the index record
    Given a scenario tagged with one lane
    When the root is synced
    Then the materialized record carries that lane
    And the sync is clean with no diagnostic

  # canon: {"schema":1,"at":"2026-09-16T11:04:54Z","actor":{"agent_id":"canon"}}
  @subject:inventory-sync
  @lane:behavior
  @inventory.sync.11
  Scenario: An unrecognized tag namespace is counted and never aborts the root
    Given a scenario carrying a namespaced tag canon does not recognize
    When the root is synced
    Then the scenario is still indexed
    And exactly one diagnostic names the namespace, once per scenario and namespace
    And the root's sync exits clean
