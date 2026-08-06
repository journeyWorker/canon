Feature: dispatch manifest
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:dispatch-manifest
  @dispatch.manifest.01
  Scenario: Beginning a dispatch mints a manifest carrying the guidance it retrieved
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository holding one strategy for the dispatched role and regime
    When a dispatch is begun
    Then a manifest is written at the path the command reports
    And the summary lists the one retrieved strategy as injected guidance
    And the file itself reads back as a run carrying that same guidance, so what was injected is on disk

  @subject:dispatch-manifest
  @dispatch.manifest.02
  Scenario: A dispatch binding nothing writes a manifest with no binding keys at all
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a dispatch begun with neither a task nor a parent run
    When the manifest is written
    Then it carries no task key and no parent run key, rather than null placeholders
    And it is byte-identical to the run serialized without those fields, because the content digest keys on exactly these bytes

  @subject:dispatch-manifest
  @dispatch.manifest.03
  Scenario: A task and a parent run supplied together both round-trip through the manifest
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository configuring a plan source that carries the named task
    When a dispatch is begun bound to that task and to a parent run
    Then the manifest deserializes back into a run carrying both values
    And the parent is recorded without being verified to exist, because a parent may not have flushed yet

  @subject:dispatch-manifest
  @dispatch.manifest.04
  Scenario: Validating a task reads the plan corpus and never writes to it
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a plan document carrying the task a dispatch names
    When the dispatch is begun against it
    Then the dispatch succeeds
    And the plan document is byte-identical afterwards, because resolution is a read

  @subject:dispatch-manifest
  @dispatch.manifest.05
  Scenario: A task row the plan corpus does not carry is refused before anything is minted
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository configuring a plan source that carries no such row
    When a dispatch is begun bound to that task
    Then the command refuses as a fixable invocation and names the rejected id
    And no dispatch record is left behind, because validation runs before the mint

  @subject:dispatch-manifest
  @dispatch.manifest.06
  Scenario: An already-completed task row is still a bindable task
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a plan document whose row for the named task is already checked off
    When a dispatch is begun bound to it
    Then the dispatch succeeds and records that task
    And membership asks whether the row exists, never whether it is still open

  @subject:dispatch-manifest
  @dispatch.manifest.07
  Scenario: A repo configuring no plan sources refuses distinctly from an unknown task
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository whose configuration names no plan sources at all
    When a dispatch is begun bound to any task
    Then the command refuses as a fixable invocation, saying the repo configures no plan sources
    And the message is not confusable with the one for a task that simply is not there
    And no dispatch record is left behind
