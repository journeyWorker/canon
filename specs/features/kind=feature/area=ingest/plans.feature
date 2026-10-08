Feature: ingest plans
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:ingest-pipelines
  @case:happy
  @ingest.plans.01
  Scenario: The planning dialects canon can import are a closed registered set
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given the registered plan dialects
    When the registry is enumerated
    Then it holds exactly two entries, openspec first and superpowers second
    And that is the whole set a configuration may name

  @subject:ingest-pipelines
  @case:happy
  @ingest.plans.02
  Scenario: A one-shot dialect and source override imports a corpus the configuration never named
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a change directory holding a proposal and a task list with one done and one open row
    When plans are imported naming the dialect and the source
    Then one change record and one task record per row are persisted
    And the given source is imported even though the configuration named no source at all

  @subject:ingest-pipelines
  @case:happy
  @ingest.plans.03
  Scenario: A second dialect imports end to end through the same one-shot override
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a plan document in a second dialect carrying one completed task and one unfinished task
    When it is imported
    Then querying changes returns that change with its own id and summary, in progress
    And querying tasks returns the completed one as done and the unfinished one as open

  @subject:ingest-pipelines
  @case:failure
  @ingest.plans.04
  Scenario: An unregistered dialect id is refused by name, never scanned as zero sources
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a configuration naming a dialect that is not registered
    When plans are imported
    Then the command fails
    And the error names the unknown id and lists the ids that are registered

  @subject:ingest-pipelines
  @case:failure
  @ingest.plans.05
  Scenario: A one-shot import needs both halves of the override or none
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository with no plan sources configured
    When a dialect is given with no source to read it from
    Then the command fails
    And the error says the two are given together, so a half-specified override never falls back to a guessed root

  @subject:ingest-pipelines
  @case:edge
  @ingest.plans.06
  Scenario: Re-importing an unchanged plan writes nothing
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a plan source imported once, whose cursor advanced
    When plans are imported again with the source unchanged
    Then zero changes and zero tasks are written and the source reports itself skipped
    And rewriting the source with byte-identical content still skips it, because the gate reads the bytes and not the modification time

  @subject:ingest-pipelines
  @case:happy
  @ingest.plans.07
  Scenario: Ticking a checkbox in the source appends the refreshed task records
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a plan source already imported with every task open
    When one checkbox is flipped and plans are imported again
    Then the source is not reported skipped
    And the refreshed task records are appended rather than dropped
    And the cursor advances past the new content

  @subject:ingest-pipelines
  @case:edge
  @ingest.plans.08
  Scenario: Two sources claiming one change id resolve to the first configured, not to two histories
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given two configured sources that each define a change under the same id
    When plans are imported
    Then exactly one change record lands, carrying the first-configured source's content
    And only the first source's tasks are imported
    And the collision is counted against the second source, never against the first

  @subject:ingest-pipelines
  @case:edge
  @ingest.plans.09
  Scenario: A repository that imports no plans says so and exits clean
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a configuration with no plans section at all
    When plans are imported
    Then the command succeeds and its summary totals are zero changes, zero tasks, zero skipped
    And it reports zero sources, never a hardcoded default root
