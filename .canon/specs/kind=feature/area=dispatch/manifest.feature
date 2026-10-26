Feature: dispatch manifest
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:dispatch-manifest
  @case:happy
  @dispatch.manifest.01
  Scenario: Beginning a dispatch mints a manifest carrying the guidance it retrieved
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository holding one strategy for the dispatched role and regime
    When a dispatch is begun
    Then a manifest is written at the path the command reports
    And the summary lists the one retrieved strategy as injected guidance
    And the file itself reads back as a run carrying that same guidance, so what was injected is on disk

  @subject:dispatch-manifest
  @case:edge
  @dispatch.manifest.02
  Scenario: A dispatch binding nothing writes a manifest with no binding keys at all
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a dispatch begun with neither a task nor a parent run
    When the manifest is written
    Then it carries no task key and no parent run key, rather than null placeholders
    And it is byte-identical to the run serialized without those fields, because the content digest keys on exactly these bytes

  @subject:dispatch-manifest
  @case:happy
  @dispatch.manifest.03
  Scenario: A task and a parent run supplied together both round-trip through the manifest
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository configuring a plan source that carries the named task
    When a dispatch is begun bound to that task and to a parent run
    Then the manifest deserializes back into a run carrying both values
    And the parent is recorded without being verified to exist, because a parent may not have flushed yet

  @subject:dispatch-manifest
  @case:happy
  @dispatch.manifest.04
  Scenario: Validating a task reads the plan corpus and never writes to it
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a plan document carrying the task a dispatch names
    When the dispatch is begun against it
    Then the dispatch succeeds
    And the plan document is byte-identical afterwards, because resolution is a read

  @subject:dispatch-manifest
  @case:failure
  @dispatch.manifest.05
  Scenario: A task row the plan corpus does not carry is refused before anything is minted
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository configuring a plan source that carries no such row
    When a dispatch is begun bound to that task
    Then the command refuses as a fixable invocation and names the rejected id
    And no dispatch record is left behind, because validation runs before the mint

  @subject:dispatch-manifest
  @case:edge
  @dispatch.manifest.06
  Scenario: An already-completed task row is still a bindable task
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a plan document whose row for the named task is already checked off
    When a dispatch is begun bound to it
    Then the dispatch succeeds and records that task
    And membership asks whether the row exists, never whether it is still open

  @subject:dispatch-manifest
  @case:failure
  @dispatch.manifest.07
  Scenario: A repo configuring no plan sources refuses distinctly from an unknown task
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository whose configuration names no plan sources at all
    When a dispatch is begun bound to any task
    Then the command refuses as a fixable invocation, saying the repo configures no plan sources
    And the message is not confusable with the one for a task that simply is not there
    And no dispatch record is left behind

  # canon: {"schema":1,"at":"2026-10-26T11:43:01Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:dispatch-manifest
  @case:happy
  @flow:canon-0-30-spec-backfill.F1
  @dispatch.manifest.08
  Scenario: adapter authorize reports execution authorized when the policy budget covers the declared capabilities
    Given a repo whose .canon/policy.yaml declares adapter_capabilities with runtime_enforced true and a read-only filesystem budget, and a well-formed response declaring read-only filesystem, no network and no secrets
    When canon adapter authorize --response names that file
    Then it exits 0 with executionAuthorized true, executionPerformed false, sandboxEnforced false and an empty reasons list
    And the declared capabilities are echoed in the report

  # canon: {"schema":1,"at":"2026-10-26T11:43:01Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:dispatch-manifest
  @case:failure
  @flow:canon-0-30-spec-backfill.F1
  @dispatch.manifest.09
  Scenario: adapter authorize denies by default when the policy declares no capability budget
    Given a repo with no adapter_capabilities in its policy and a well-formed response declaring a read-only filesystem
    When canon adapter authorize --response names that file
    Then it exits 1 with ADAPTER_EXECUTION_NOT_AUTHORIZED, and data keeps the report with executionAuthorized false and executionPerformed false
    And data.reasons names runtime_enforced=false and the filesystem capability exceeding the policy budget

  # canon: {"schema":1,"at":"2026-10-26T11:43:01Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:dispatch-manifest
  @case:failure
  @flow:canon-0-30-spec-backfill.F1
  @dispatch.manifest.10
  Scenario: adapter authorize on a missing response file is refused as a fixable invocation
    Given a repo and a --response path that names no file
    When canon adapter authorize --response names that path
    Then it exits 2 with ADAPTER_REFUSED naming the unreadable path
    And no file is created or changed in the repo

  # canon: {"schema":1,"at":"2026-10-26T11:43:02Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:dispatch-manifest
  @case:happy
  @flow:canon-0-30-spec-backfill.F1
  @dispatch.manifest.11
  Scenario: adapter validate prints the normalized summary of a well-formed response
    Given a well-formed adapter response file
    When canon adapter validate --response names that file
    Then it exits 0 with the normalized summary: runId, provider, model, contextPackId, status, capabilities, evidenceCount, telemetry and extensionKeys
    And contextJoinVerified is false because the response names a context pack the repo does not hold

  # canon: {"schema":1,"at":"2026-10-26T11:43:02Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:dispatch-manifest
  @case:failure
  @flow:canon-0-30-spec-backfill.F1
  @dispatch.manifest.12
  Scenario: adapter validate rejects a response whose envelope breaks a rule
    Given an adapter response whose telemetry cost is negative
    When canon adapter validate --response names that file
    Then it exits 1 with ADAPTER_REFUSED and a message naming telemetry.cost_usd
    And no file is created or changed in the repo

  # canon: {"schema":1,"at":"2026-10-26T11:43:02Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:dispatch-manifest
  @case:failure
  @flow:canon-0-30-spec-backfill.F1
  @dispatch.manifest.13
  Scenario: adapter validate on a missing response file is refused as a fixable invocation
    Given a --response path that names no file
    When canon adapter validate --response names that path
    Then it exits 2 with ADAPTER_REFUSED naming the unreadable path
    And no file is created or changed in the repo

  # canon: {"schema":1,"at":"2026-10-26T11:43:02Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:dispatch-manifest
  @case:happy
  @flow:canon-0-30-spec-backfill.F1
  @dispatch.manifest.14
  Scenario: context-pack add stores an immutable pack and prints its id
    Given a canon repo under git and a context manifest of manifestVersion 1 selecting one existing document
    When canon context-pack add --manifest names that manifest
    Then it exits 0 and the data carries an id of the form sha256:<64 hex> and the selected document with its digest
    And repeating the same add yields the same id

  # canon: {"schema":1,"at":"2026-10-26T11:43:02Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:dispatch-manifest
  @case:failure
  @flow:canon-0-30-spec-backfill.F1
  @dispatch.manifest.15
  Scenario: context-pack add refuses a manifest that names a missing input and stores nothing
    Given a context manifest that selects a document which does not exist
    When canon context-pack add --manifest names that manifest
    Then it exits 2 with CONTEXT_INPUT_MISSING naming the missing file
    And the repo tree is identical to before the call

  # canon: {"schema":1,"at":"2026-10-26T11:43:03Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:dispatch-manifest
  @case:happy
  @flow:canon-0-30-spec-backfill.F1
  @dispatch.manifest.16
  Scenario: context-pack show prints the manifest of a stored pack
    Given a context pack made by canon context-pack add
    When canon context-pack show runs with that pack id
    Then it exits 0 and prints the pack manifest with the same id and the selected document

  # canon: {"schema":1,"at":"2026-10-26T11:43:03Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:dispatch-manifest
  @case:failure
  @flow:canon-0-30-spec-backfill.F1
  @dispatch.manifest.17
  Scenario: context-pack show on an unknown pack id is refused
    Given a repo and a well-formed pack id that no pack carries
    When canon context-pack show runs with that id
    Then it exits 2 with CONTEXT_PACK_NOT_FOUND naming the id
    And the repo tree is identical to before the call

  # canon: {"schema":1,"at":"2026-10-26T11:43:03Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:dispatch-manifest
  @case:happy
  @flow:canon-0-30-spec-backfill.F1
  @dispatch.manifest.18
  Scenario: context-pack verify confirms a stored pack is intact
    Given a context pack made by canon context-pack add
    When canon context-pack verify runs with that pack id
    Then it exits 0 with data.id the pack id and data.verified true

  # canon: {"schema":1,"at":"2026-10-26T11:43:03Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:dispatch-manifest
  @case:failure
  @flow:canon-0-30-spec-backfill.F1
  @dispatch.manifest.19
  Scenario: context-pack verify on an unknown pack id is refused
    Given a repo and a well-formed pack id that no pack carries
    When canon context-pack verify runs with that id
    Then it exits 2 with CONTEXT_PACK_NOT_FOUND naming the id
    And the repo tree is identical to before the call

  # canon: {"schema":1,"at":"2026-10-26T11:43:03Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:dispatch-manifest
  @case:happy
  @flow:canon-0-30-spec-backfill.F1
  @dispatch.manifest.20
  Scenario: prompt add registers a prompt bundle under name and version
    Given a canon repo under git and a prompt manifest whose prompt is an existing file
    When canon prompt add reviewer@v1 --manifest names that manifest
    Then it exits 0 with name reviewer, version v1, a sha256 digest and the prompt file snapshot under prompts
    And repeating the same add succeeds with the same digest

  # canon: {"schema":1,"at":"2026-10-26T11:43:04Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:dispatch-manifest
  @case:failure
  @flow:canon-0-30-spec-backfill.F1
  @dispatch.manifest.21
  Scenario: prompt add refuses an operand that is not NAME@VERSION and writes nothing
    Given a prompt manifest and an operand that has no @VERSION part
    When canon prompt add runs with that operand
    Then it exits 2 with USAGE on field <NAME@VERSION> saying the bundle must be NAME@VERSION
    And the repo tree is identical to before the call

  # canon: {"schema":1,"at":"2026-10-26T11:43:04Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:dispatch-manifest
  @case:failure
  @flow:canon-0-30-spec-backfill.F1
  @dispatch.manifest.22
  Scenario: prompt add refuses different content under an already registered name and version
    Given reviewer@v1 is registered from one prompt manifest, and a second manifest whose prompt file has different content
    When canon prompt add reviewer@v1 --manifest names the second manifest
    Then it exits 1 with CONTEXT_OBJECT_CONFLICT
    And canon prompt show reviewer@v1 still prints the digest of the first registration

  # canon: {"schema":1,"at":"2026-10-26T11:43:04Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:dispatch-manifest
  @case:happy
  @flow:canon-0-30-spec-backfill.F1
  @dispatch.manifest.23
  Scenario: prompt show prints a registered prompt bundle
    Given reviewer@v1 registered by canon prompt add
    When canon prompt show reviewer@v1 runs
    Then it exits 0 and prints the bundle with name reviewer, version v1, the digest and the prompt file snapshot

  # canon: {"schema":1,"at":"2026-10-26T11:43:04Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:dispatch-manifest
  @case:failure
  @flow:canon-0-30-spec-backfill.F1
  @dispatch.manifest.24
  Scenario: prompt show on an unregistered bundle is refused
    Given a repo where no bundle is registered under the name and version
    When canon prompt show runs with that NAME@VERSION
    Then it exits 2 with PROMPT_BUNDLE_INVALID saying the version is not registered
    And the repo tree is identical to before the call

  # canon: {"schema":1,"at":"2026-10-26T11:43:04Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:dispatch-manifest
  @case:happy
  @flow:canon-0-30-spec-backfill.F1
  @dispatch.manifest.25
  Scenario: dispatch end closes a begun run with its outcome
    Given a run begun by canon dispatch begin
    When canon dispatch end <run-id> --run-outcome succeeded runs
    Then it exits 0 with data.runId the run id, data.status succeeded and an endedAt timestamp
    And the run's manifest records the status and the end time

  # canon: {"schema":1,"at":"2026-10-26T11:43:04Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:dispatch-manifest
  @case:failure
  @flow:canon-0-30-spec-backfill.F1
  @dispatch.manifest.26
  Scenario: dispatch end on an already ended run is refused and the manifest is unchanged
    Given a run that canon dispatch end already closed as succeeded
    When canon dispatch end runs again for that run with --run-outcome failed
    Then it exits 2 with DISPATCH_ALREADY_ENDED
    And the run's manifest is byte-identical to before the second call

  # canon: {"schema":1,"at":"2026-10-26T11:43:05Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:dispatch-manifest
  @case:failure
  @flow:canon-0-30-spec-backfill.F1
  @dispatch.manifest.27
  Scenario: dispatch end on a run that was never begun is refused and writes nothing
    Given a repo and a well-formed run id that dispatch begin never minted
    When canon dispatch end runs for that id
    Then it exits 2 with DISPATCH_RUN_NOT_FOUND
    And the repo tree is identical to before the call

  # canon: {"schema":1,"at":"2026-10-26T11:43:05Z","actor":{"agent_id":"impl-030-sb-f1","role":"implementer","session_id":"sb-f1-2026-10-26"}}
  @subject:dispatch-manifest
  @case:failure
  @flow:canon-0-30-spec-backfill.F1
  @dispatch.manifest.28
  Scenario: dispatch diff outside a canon repository is refused and writes nothing
    Given a directory with no canon.yaml in it or any parent
    When canon dispatch diff runs there
    Then it exits 2 with CONFIG_MISSING on field --repo
    And no file is created in the directory
