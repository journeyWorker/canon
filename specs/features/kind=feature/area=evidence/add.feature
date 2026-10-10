Feature: evidence add
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:evidence-attestation
  @case:happy
  @evidence.add.01
  Scenario: An authored record satisfies the gate only after promotion, so the loop is add then promote then flip
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository whose plan corpus carries one open task row
    When evidence is added for that task, then promoted, then the task is asked to complete
    Then the add reports the record staged, and a committed read sees nothing
    And the gate refuses the flip while the record is only staged, leaving the row open
    And after promotion the flip succeeds and the row carries the authored summary
    And the committed record still carries the evidence kind and reference that were authored

  @subject:evidence-attestation
  @case:happy
  @evidence.add.02
  Scenario: Promotion assigns the run sequence number, so authoring alone never carries one
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an authored record sitting in staging
    When it is promoted
    Then the committed record carries run sequence number one
    And the evidence companion survives the rewrite promotion performs to stamp it

  @subject:evidence-attestation
  @case:happy
  @evidence.add.03
  Scenario: A scenario-keyed attestation clears exactly the scenario it names
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a two-scenario spec corpus under a policy that requires evidence for each
    When evidence naming one scenario and its project is added and promoted
    Then the gate stops reporting that scenario
    And it still reports the other one, because an attestation covers its own key and no neighbours

  @subject:evidence-attestation
  @case:failure
  @evidence.add.04
  Scenario: A record keyed to neither a task nor a scenario is refused
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an add naming a kind and a reference but no coverage subject at all
    When it is invoked
    Then it exits two as a fixable invocation and names the keys it will accept
    And nothing is staged, because a record no gate can read back is an attestation nobody can check

  @subject:evidence-attestation
  @case:failure
  @evidence.add.05
  Scenario: A scenario id with no project id cannot close the join and is refused
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an add naming a scenario id and omitting the project id
    When it is invoked
    Then it exits two and names the missing project id
    And the record is refused rather than written for the gate to silently ignore

  @subject:evidence-attestation
  @case:failure
  @evidence.add.06
  Scenario: An empty evidence kind or reference is refused rather than written
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an add whose kind is only whitespace, and another whose reference is empty
    When each is invoked
    Then both exit two
    And nothing is staged, because a companion naming no class and no reference tells a reader nothing

  @subject:evidence-attestation
  @case:failure
  @evidence.add.07
  Scenario: A fabrication marker in the summary is refused at authoring, before anything is staged
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an add whose summary claims the work would pass once the suite is wired up
    When it is invoked
    Then it is graded the same way the gate grades fabricated evidence
    And staging stays empty, so the note never reaches the append-only ledger where it could not be withdrawn

  @subject:evidence-attestation
  @case:failure
  @evidence.add.08
  Scenario: A line separator in a document-bound field is refused, and the plan is untouched
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an add whose summary carries a newline followed by a checked row of its own
    When it is invoked
    Then it exits two as a fixable invocation, not as gate-red evidence
    And nothing is staged
    And the plan document is byte-identical, so the forged row never existed to clean up

  @subject:evidence-attestation
  @case:edge
  @evidence.add.09
  Scenario: A multi-line captured command result stays authorable
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an add carrying a summary and a command result of several lines of real captured output
    When it is invoked
    Then it succeeds and the record is staged
    And the refusal of line separators is scoped to the fields that reach a document, never to pasted output

  @subject:evidence-attestation
  @case:failure
  @evidence.add.10
  Scenario: Approval metadata without authentication is refused before staging
  # canon: {"schema":1,"at":"2026-10-01T00:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an evidence add with approval metadata but no detached SSH signature
    When the add is invoked
    Then it is refused before staging, even when --approval-by and --approval-role are both supplied
    And CANON_ACTOR does not substitute for authenticated approval

  # canon: {"schema":1,"at":"2026-10-08T15:13:25Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.11
  Scenario: A report binds the matched case and refuses a faithful claim over a failed case
    Given a JUnit report whose case names the scenario id and failed
    When faithful evidence is added with that report
    Then the command exits 1 naming the failed case and stages nothing
    And once the case passes, the record carries the report path, digest, matched case and passed outcome

  # canon: {"schema":1,"at":"2026-10-08T15:13:25Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.12
  Scenario: Any repository file binds by digest without being run
    Given a trace file inside the repository
    When evidence is added with it as an artifact
    Then the record carries its repository-relative path and sha256
    And a file outside the repository is refused because no reviewer could find it

  # canon: {"schema":1,"at":"2026-10-10T15:43:50Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.13
  Scenario: Bound files are copied into the artifact store, so a later rewrite cannot orphan the record
    Given a scenario and a smoke report inside the repository
    When evidence is added with the report bound as an artifact and promoted
    Then the report's bytes are stored under .canon/artifacts/sha256/<digest> and the add names the store
    And after the report is rewritten the gate is still clean, proven from the stored bytes

  # canon: {"schema":1,"at":"2026-10-10T15:43:50Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.14
  Scenario: A bound file over the store's size limit is refused, naming the override flag
    Given a bound file larger than the store's size limit
    When evidence is added with a one-MiB limit
    Then the command exits 2 naming the --max-artifact-mib override, and stages and stores nothing
    And raising the limit above the file's size lets the same add succeed and store the bytes

  # canon: {"schema":1,"at":"2026-10-10T15:43:50Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.15
  Scenario: A summary is kept on scenario-only evidence and still scanned for fabrication
    Given a scenario and no plan task for it
    When evidence naming only the scenario is added with a summary
    Then the committed record carries the summary in its evidence note
    And a summary carrying a fabrication marker is refused with exit 1, naming the scenario

  # canon: {"schema":1,"at":"2026-10-10T15:43:50Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.16
  Scenario: Every report case carrying the scenario id is bound, and any failed one refuses a faithful verdict
    Given a test report with two cases carrying the scenario id and one helper case without it
    When evidence is added with the report and a report-case naming the helper
    Then the record binds all three cases, each with its own name and outcome
    And once one id-carrying case fails, a faithful claim is refused with exit 1 naming it, even though the named case passed

  # canon: {"schema":1,"at":"2026-10-10T15:43:50Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.17
  Scenario: The vault stores the bytes of records written before the store existed
    Given a committed record whose bound report has no stored blob but is unchanged in the working tree
    When the vault runs
    Then it stores the report's bytes under its digest and says it stored one file
    And the gate stops advising the vault, and a second run stores nothing new

  # canon: {"schema":1,"at":"2026-10-10T15:43:50Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:edge
  @evidence.add.18
  Scenario: A fresh init never ignores the artifact store
    Given a fresh repository after git init and canon init
    When git is asked whether a path under .canon/artifacts is ignored
    Then it is not, so stored evidence bytes are committed with the ledger

  # canon: {"schema":1,"at":"2026-10-10T16:02:33Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.19
  Scenario: Evidence records the attesting session
    Given a scenario
    When evidence is added with --actor-id and --session-id and promoted
    Then the committed record's actor carries the agent id, the role and the session_id

  # canon: {"schema":1,"at":"2026-10-10T16:02:33Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.20
  Scenario: A session id outside the SessionId grammar is refused and nothing is written
    Given a session id with a control character, surrounding whitespace, or nothing at all
    When evidence add or review add is run with it
    Then the command exits 2 naming --session-id
    And nothing is staged or committed

  # canon: {"schema":1,"at":"2026-10-10T16:02:33Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.21
  Scenario: Every write command says whether it staged or wrote directly
    Given the write commands evidence add, finding add and close, review add, divergence stage and resolve, subject new, adopt and status, scenario new and feature new
    When each succeeds
    Then a staged write ends with the promote command that commits it
    And a direct write ends with written directly; nothing to promote
