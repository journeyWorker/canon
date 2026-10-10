Feature: context status
  # canon: {"schema":1,"at":"2026-10-10T15:35:47Z","actor":{"agent_id":"canon"}}

  # canon: {"schema":1,"at":"2026-10-10T15:35:47Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:happy
  @context.status.01
  Scenario: Status counts each subject's scenarios, evidence, reviews, blockers and missing cases through the gate's own joins
    Given a building subject owning scenarios on two surfaces, with evidence on some, one divergent verdict, a review by another actor on one, a self-review on another, and an open blocker on its adopted change
    When status is run
    Then the subject is listed under its status with its counted scenarios, evidenced, divergent and reviewed counts and its open blockers
    And a self-review does not count when the review rule wants a distinct actor
    And the surface with no failure case is named as a missing required case
    And the header names the canon version and the effective coverage, case and review settings

  # canon: {"schema":1,"at":"2026-10-10T15:35:47Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:happy
  @context.status.02
  Scenario: Status ends with the next commands, one per gap, chosen by fixed rules
    Given a verifying subject with a surface lacking a failure case, an unevidenced scenario, an open blocker and an evidenced but unreviewed scenario, and a building subject owning no scenario
    When status is run
    Then the next list names, in order, a new failure scenario with the next free number, an evidence attestation for the unevidenced scenario, the finding close for the blocker, a review by another actor, and a new scenario for the empty subject
    And each command carries a one-line reason

  # canon: {"schema":1,"at":"2026-10-10T15:35:47Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:happy
  @context.status.03
  Scenario: Status suggests shipping only a subject the ship gate accepts
    Given a verifying subject whose every scenario is evidenced and independently reviewed, with a failure case on its surface
    When status is run
    Then the only next command is moving that subject to shipped
    And running that command passes the ship and review gates
    And afterwards status lists the subject as shipped with nothing next

  # canon: {"schema":1,"at":"2026-10-10T15:35:47Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:edge
  @context.status.04
  Scenario: Status lists the scenarios no subject owns and how to tag them
    Given scenarios that carry no subject tag beside one that does
    When status is run
    Then the untagged scenarios are listed as unowned
    And the next list ends with tagging them and syncing the inventory

  # canon: {"schema":1,"at":"2026-10-10T15:35:47Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:failure
  @context.status.05
  Scenario: Status warns when no policy makes the gate enforce coverage
    Given a repository with subjects and scenarios but no policy file
    When status is run
    Then it exits zero, reports the policy absent and warns that the gate requires no evidence, failure cases or review
    And with a policy file that has no coverage section it warns that the section is missing instead

  # canon: {"schema":1,"at":"2026-10-10T15:35:47Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:failure
  @context.status.06
  Scenario: Status on an unreadable manifest still answers, naming the repair command
    Given a manifest whose tier section does not parse
    When status is run
    Then it still exits zero
    And it warns that the ledger cannot be read, naming the parse error
    And its only next command is checking the manifest

  # canon: {"schema":1,"at":"2026-10-10T15:35:47Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:edge
  @context.status.07
  Scenario: Status is a read that exits zero and writes nothing
    Given a repository with an unevidenced scenario and an open blocker
    When status is run in both the human and the JSON form
    Then each run exits zero
    And no file in the repository is created or changed, including the access audit log

  # canon: {"schema":1,"at":"2026-10-10T15:35:47Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:edge
  @context.status.08
  Scenario: Status caps the next list at five and counts the rest
    Given seven building subjects that each own no scenario and one unowned scenario
    When status is run
    Then exactly five next commands are listed
    And the three further commands are counted, not listed

  # canon: {"schema":1,"at":"2026-10-10T15:35:48Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:edge
  @context.status.09
  Scenario: Status on subjects with no scenario records points at the inventory sync
    Given subjects but no scenario records in the ledger
    When status is run
    Then it warns that the ledger holds no scenario records
    And its only next command is the inventory sync, not one empty-subject step per subject

  # canon: {"schema":1,"at":"2026-10-10T15:59:34Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:failure
  @context.status.10
  Scenario: Status fails closed when a corpus kind routes away from the rung it reads
    Given a manifest that routes scenario records to a rung other than the local one status reads
    And a verifying subject that would otherwise read as owning nothing
    When status is run
    Then it warns that the scenario kind routes away and its records are not counted
    And its only next command is the configuration check, with the reason naming the routing setting to restore
    And no step derived from the unread corpus is listed

  # canon: {"schema":1,"at":"2026-10-10T15:59:34Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:failure
  @context.status.11
  Scenario: An unusable coverage policy is a blocking gap that withholds every move-on suggestion
    Given a policy whose coverage section is unusable
    And a verifying subject whose every scenario is evidenced
    When status is run
    Then it warns that the coverage section is unusable
    And the first next command is the gate check, with the reason naming the section to fix
    And no subject is suggested to move to its next status

  # canon: {"schema":1,"at":"2026-10-10T15:59:34Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:edge
  @context.status.12
  Scenario: With several spec roots a suggested scenario command names its root
    Given a manifest that configures two spec roots
    And a subject owning a scenario under one root with no failure case, and a subject owning no scenario
    When status is run
    Then the suggested failure scenario names its owning root with the project flag, and running it as printed succeeds
    And the empty subject's suggestion carries a root placeholder and its reason lists the configured roots
    And with a single spec root no project flag is suggested

  # canon: {"schema":1,"at":"2026-10-10T15:59:34Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:edge
  @context.status.13
  Scenario: The header prints the policy settings as resolved, not as defaults
    Given a policy that narrows the coverage scope, excludes a lane, requires no case and has no review rule
    When status is run
    Then the header prints each setting as resolved, with no case required and the review rule absent

  # canon: {"schema":1,"at":"2026-10-10T18:06:29Z","actor":{"agent_id":"canon"}}
  @subject:context-capability
  @case:edge
  @context.status.14
  Scenario: Every status write suggestion names the unit and its session
    Given a subject with an unevidenced scenario, an open blocker finding and a due review
    When status is run
    Then every suggested command parses as printed
    And every write suggestion carries `--actor-id <unit>` and `--session-id <session>` wherever its command takes them
