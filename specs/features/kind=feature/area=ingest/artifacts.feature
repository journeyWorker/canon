Feature: ingest artifacts
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:ingest-pipelines
  @case:happy
  @ingest.artifacts.01
  Scenario: Every artifact adapter declares which shape of input it reads
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given the registered artifact adapters for reviews, divergences, tasks and handoffs
    When each entry is inspected
    Then the handoff adapter is tagged as reading canon's own records
    And the divergence, ledger and openspec-task adapters are each tagged as reading a raw path
    And the tag is a property of the adapter, not something a caller chooses per run

  @subject:ingest-pipelines
  @case:happy
  @ingest.artifacts.02
  Scenario: One pass drives both input shapes and lands one regime-keyed trajectory
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository holding a committed handoff and a review ledger carrying one open finding
    When artifacts are ingested
    Then the records-source handoff adapter reports read, not unavailable, and parses one event per state transition it implies
    And the path-source ledger adapter reports read and parses the finding
    And exactly one verdict is derived, because a handoff transition carries no verdict by design
    And one trajectory is persisted under a regime key naming the role, the repository and the area
    And that trajectory is readable back as a dev-role failure-polarity verdict

  @subject:ingest-pipelines
  @case:edge
  @ingest.artifacts.03
  Scenario: A regime is the grouping key and its recorded instant is the newest source record
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given two verdicts derived at different instants under one regime key
    When they are grouped
    Then both fold onto that one regime rather than splitting
    And the regime's recorded instant is the later of the two, never the first read

  @subject:ingest-pipelines
  @case:happy
  @ingest.artifacts.04
  Scenario: A code-review finding is a failure the dev role must not repeat
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a code-review finding
    When a verdict is derived from it
    Then the verdict is scoped to the dev role
    And its polarity is failure and it becomes a guardrail candidate, never a strategy

  @subject:ingest-pipelines
  @case:happy
  @ingest.artifacts.05
  Scenario: A native divergence takes the actor's own role, never a fixed default
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a resolved divergence whose actor is a non-dev role
    When its verdict is derived
    Then the verdict carries that actor's role
    And it is not silently coerced to dev

  @subject:ingest-pipelines
  @case:edge
  @ingest.artifacts.06
  Scenario: Re-ingesting an unchanged corpus persists nothing new
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a corpus already ingested once
    When artifacts are ingested again with nothing changed
    Then zero new trajectories are persisted
    And the second pass is a no-op rather than a second copy of the first pass's conclusions

  @subject:ingest-pipelines
  @case:edge
  @ingest.artifacts.07
  Scenario: The identity a trajectory is deduplicated on is its content, so changed verdicts are a different trajectory
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a trajectory over a set of verdicts under one regime key
    When the contents of those verdicts differ
    Then the content digest differs
    And the changed trajectory is therefore written rather than mistaken for the one already stored

  @subject:ingest-pipelines
  @case:failure
  @ingest.artifacts.08
  Scenario: Reading canon's own records and reading a raw path are mutually exclusive, and the conflict is caught first
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a configuration asking for native records and also naming a raw ledger root
    When artifacts are ingested
    Then the command fails before any source is read
    And the error names the conflicting field rather than quietly picking one of the two

  @subject:ingest-pipelines
  @case:failure
  @ingest.artifacts.09
  Scenario: A source with nowhere to write degrades to unavailable and the rest of the pass still runs
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository whose handoff kind is unrouted while its other artifact sources are routed
    When artifacts are ingested
    Then that source is reported unavailable rather than silently read as empty
    And the remaining sources are still driven to completion in the same pass
    And the command as a whole does not abort
