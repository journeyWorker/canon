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

  # canon: {"schema":1,"at":"2026-10-10T18:33:15Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.22
  Scenario: A report case binds only to its own leading scenario id, never to one it mentions
    Given a report with a passing case led by the scenario id and a failing case led by another id that mentions it
    When faithful evidence is added for the scenario with that report
    Then the record binds only the case the scenario id leads, and the faithful claim is accepted
    And naming the mentioning case with --report-case binds it, so its failure refuses the faithful claim with exit 1

  # canon: {"schema":1,"at":"2026-10-10T19:35:32Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.40
  Scenario: Re-attesting a scenario with fewer bound files is refused, naming the dropped paths
    Given a scenario whose latest evidence record binds two files
    When evidence is added for it binding only one of them
    Then the add exits 2 naming the left-out path
    And the refusal names both ways out: re-bind it, or --drop-attachment with --drop-reason
    And nothing is staged

  # canon: {"schema":1,"at":"2026-10-10T19:35:32Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.41
  Scenario: A deliberate drop is recorded with its reason and listed by canon status
    Given a scenario whose latest evidence record binds a screenshot and a log
    When evidence is added binding the screenshot again with new bytes, then binding only the log with --drop-attachment for the screenshot and a --drop-reason
    Then both adds are accepted and the second record carries attachment_drop with that path and reason
    And canon status lists the drop with its reason, actor and time

  # canon: {"schema":1,"at":"2026-10-10T19:35:32Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:edge
  @evidence.add.42
  Scenario: A staged, unpromoted record is the latest record the drop guard compares with
    Given a staged, unpromoted record for a scenario that binds two files
    When evidence is added for the scenario binding only one of them
    Then the add exits 2 naming the other file

  # canon: {"schema":1,"at":"2026-10-10T19:35:33Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.43
  Scenario: Malformed drop flags are refused before anything is staged
    Given a scenario with bound evidence
    When evidence is added with --drop-attachment but no --drop-reason, a reason but no path, a path that is not being dropped, a multi-line reason, or a drop on task-only evidence
    Then each add exits 2 naming what is wrong and nothing is staged

  # canon: {"schema":1,"at":"2026-10-10T19:35:33Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.44
  Scenario: Audit flags are required with --kind audit and refused on every other kind
    Given a scenario
    When an audit is added without the evaluator flags, or evaluator flags are given on a test-run, or an audit carries --verdict or --task, an unknown result, or a digest that is not a sha256
    Then each add exits 2 naming the flag and nothing is staged

  # canon: {"schema":1,"at":"2026-10-10T19:35:33Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.45
  Scenario: An audit is recorded beside the declared verdict and never replaces it
    Given a scenario with a faithful declared record
    When an external evaluator's refuted audit with a counterexample report is added with --kind audit
    Then the record carries the evaluator id, version, digest and result, and a divergent verdict
    And the counterexample report is bound and stored like any report
    And the scenario's declared verdict is still the faithful one

  # canon: {"schema":1,"at":"2026-10-11T15:01:53Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.60
  Scenario: Promote on the command line commits the record in the same run
    Given a repository whose canon.yaml has no write section
    When evidence add runs with --promote
    Then the same run promotes the record and prints one success line naming the committed record
    And the success line ends "staged and promoted in this run; nothing to promote"
    And a later gate promote finds nothing staged and exits 0

  # canon: {"schema":1,"at":"2026-10-11T15:01:53Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:edge
  @evidence.add.61
  Scenario: Without a write key a staged record waits for gate promote
    Given a repository whose canon.yaml has no write section
    When evidence add runs without a promote flag
    Then the record stays staged and the line names canon gate promote
    And gate promote commits it

  # canon: {"schema":1,"at":"2026-10-11T15:01:53Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.62
  Scenario: Auto promotion commits every staged write and no-promote keeps one staged
    Given a repository whose canon.yaml says write promote auto, as canon init writes it
    When evidence add, finding add, finding close and divergence stage run
    Then nothing is left staged
    And an evidence add with --no-promote stays staged until gate promote

  # canon: {"schema":1,"at":"2026-10-11T15:01:53Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.63
  Scenario: A malformed write section or both promote flags are refused before anything is staged
    Given a canon.yaml whose write section says promote always
    When a staged write runs
    Then it is refused with exit 2 naming the write section and nothing is staged
    And init --check-config reports the write section as failed
    And --promote together with --no-promote is a usage error with exit 2

  # canon: {"schema":1,"at":"2026-10-11T15:01:53Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.64
  Scenario: A promotion that refuses the record just staged leaves it staged and exits 1
    Given a staged write whose own record promotion refuses
    When the write runs with --promote
    Then the record is still staged
    And the success line names canon gate promote
    And the command exits 1

  # canon: {"schema":1,"at":"2026-10-11T15:45:12Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.65
  Scenario: A promote on write commits only the record its own run staged
    Given another author staged an evidence record and a divergence candidate with --no-promote
    When evidence add and divergence stage run under write promote auto
    Then each run promotes and names its own record only
    And the other author's records are still staged
    And gate promote and divergence promote commit them afterwards

  # canon: {"schema":1,"at":"2026-10-11T17:14:21Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.66
  Scenario: A store rollback never removes a blob another writer published
    Given two files staged into the artifact store under its lock
    And another writer has already stored the first file's blob intact
    When publishing the second file fails and the store rolls back
    Then the first file's blob is still stored and hashes to its name
    And no temp file is left in the store
    And a second writer of the same repo waits for the lock and is refused naming the lock file

  # canon: {"schema":1,"at":"2026-10-12T03:41:11Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.67
  Scenario: A promoted write prints one success line that names the committed record
    Given a repository whose canon.yaml says write promote auto, as canon init writes it
    When evidence add, finding add, finding close and divergence stage run
    Then each prints one success line, naming the record's committed file under the ledger, which exists
    And no line names the staged file promotion moved, and no separate promote line is printed
    And with --no-promote, evidence add and finding add print the staged file and canon gate promote, and divergence stage prints the staged file and canon divergence promote

  # canon: {"schema":1,"at":"2026-10-12T07:30:40Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.68
  Scenario: Evidence for a subject whose brief is not approved is refused, and nothing is staged
    Given a building subject owning a scenario and adopting a change whose .canon/changes brief.md has no "Approved:" line, with a task row in its tasks.md
    When evidence is added for the scenario, and then for the task
    Then each add exits 1 with an unreviewed-promotion line naming the subject and that brief.md
    And the refusal tells the agent to stop, report the brief and wait for the human's answer, and names --override-reason
    And nothing is staged or committed
    And canon review add and canon finding add for the same scenario and change are not refused

  # canon: {"schema":1,"at":"2026-10-12T07:30:41Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:edge
  @evidence.add.69
  Scenario: A subject with no brief to answer is attested as before
    Given a building subject that adopts no change, one whose newest change has no brief, and a scenario no subject owns
    When evidence is added for each scenario
    Then each add succeeds with no brief refusal
    And the record carries no brief override and canon status lists none

  # canon: {"schema":1,"at":"2026-10-12T07:30:41Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:edge
  @evidence.add.70
  Scenario: An override attests anyway and records its reason and actor where a reader sees it
    Given a building subject owning a scenario whose newest change's brief.md has no "Approved:" line
    When evidence is added for the scenario with a blank --override-reason, then with a one-line reason and an actor id
    Then the blank reason exits 2 and nothing is written
    And the reasoned add succeeds, printing the waived unreviewed-promotion line and who recorded the override
    And the record carries brief_override with the reason, the waived unreviewed-promotion naming the subject, and the actor
    And canon status lists it as a warning and canon gate check as an advisory, without failing
    And once the brief is approved, an override nothing needed is not recorded

  # canon: {"schema":1,"at":"2026-10-12T07:30:41Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.71
  Scenario: Recording the human's approval lifts the refusal
    Given a building subject whose scenario and task attestations are refused because its brief has no "Approved:" line
    When the human's answer is recorded as the "Approved:" line in its proposal.md
    Then the same scenario and task attestations succeed with no override

  # canon: {"schema":1,"at":"2026-10-12T12:00:00Z","actor":{"agent_id":"017-p2-impl"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.72
  Scenario: A change from an external plan is answered in a canon brief, and its task rows still flip
    Given a building subject owning a scenario and adopting a change from an external openspec plan, whose proposal.md carries an "Approved:" line and whose tasks.md has a task row, and no canon brief
    When evidence is added for the scenario, and then for the task
    Then each add exits 1 with an unreviewed-promotion line naming .canon/changes/<change>/brief.md as not existing yet, the external proposal.md it must point at with a "Source:" line, and the approval to record there
    And nothing is staged or committed
    And once that canon brief.md holds the "Source:" line and the "Approved:" line, both adds succeed
    And the external proposal.md is never edited, and canon gate task flips the external task row

  # canon: {"schema":1,"at":"2026-10-12T15:00:00Z","actor":{"agent_id":"018-n1-impl"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.80
  Scenario: One report attests every indexed scenario that owns a case in it, one record each
    Given three indexed scenarios and a JUnit report whose cases lead with their ids, one of them failing
    When evidence is added with --report and --all-scenarios and no --verdict
    Then three records are written, each binding only its own cases, with the failing scenario's verdict divergent and the others faithful
    And every record pins the same stored report blob, which is stored once
    And the output lists each scenario with its verdict on its own line

  # canon: {"schema":1,"at":"2026-10-12T15:00:00Z","actor":{"agent_id":"018-n1-impl"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.81
  Scenario: A faithful claim over a batch with any failed case refuses the whole batch
    Given indexed scenarios and a report where one scenario's case failed and the others passed
    When evidence is added with --all-scenarios --verdict faithful
    Then the add exits 1 naming the failed case and its scenario
    And no record is staged or committed and no blob is stored

  # canon: {"schema":1,"at":"2026-10-12T15:00:00Z","actor":{"agent_id":"018-n1-impl"}}
  @subject:evidence-attestation
  @case:edge
  @evidence.add.82
  Scenario: A case whose scenario id is not indexed is named and skipped
    Given a report with a case leading with an id no indexed scenario carries, and a case with no id at all
    When evidence is added with --all-scenarios
    Then the indexed scenarios get their records and the output names the unindexed id and counts the case with no id as skipped
    And a report in which no case belongs to an indexed scenario is refused with exit 2 and nothing is written

  # canon: {"schema":1,"at":"2026-10-12T15:00:00Z","actor":{"agent_id":"018-n1-impl"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.83
  Scenario: Any refusal one record of a batch would meet refuses all of them
    Given a batch of scenarios where one belongs to a subject whose brief is not approved
    When evidence is added with --all-scenarios
    Then the add exits 1 with the brief refusal and nothing is staged for any scenario
    And a summary carrying a fabrication marker refuses the batch the same way
    And a scenario whose latest record binds a file the batch does not refuses it with exit 2, nothing written
    And --all-scenarios with --scenario-id, --task, --report-case, a drop or an approval, or without --report or --project-id, is refused with exit 2

  # canon: {"schema":1,"at":"2026-10-12T15:00:00Z","actor":{"agent_id":"018-n1-impl"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.84
  Scenario: A task's report with no scenario id binds every case in it
    Given a task row and a report whose cases carry no scenario id
    When evidence is added with --task and --report and no --scenario-id
    Then the record binds every case in the report
    And when any case failed, a faithful verdict is refused with exit 1 and nothing is staged

  # canon: {"schema":1,"at":"2026-10-12T15:00:00Z","actor":{"agent_id":"018-n1-impl"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.85
  Scenario: A Swift Testing event stream binds by display name, then function name
    Given the event stream a real `swift test --experimental-event-stream-output` run wrote, with passing, failing, skipped and parameterized tests
    When it is bound with --report swift-testing:<path>
    Then each test's case is named by its display name, or its function name when it has none
    And a test with a failing issue is failed, a skipped test skipped, and a test that started and ended without one passed
    And a stream that is not JSON lines, or in which no test ran, is refused with exit 2

  # canon: {"schema":1,"at":"2026-10-12T15:00:00Z","actor":{"agent_id":"018-n1-impl"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.86
  Scenario: A write that makes a pinned review stale names the review and its re-pin command
    Given a scenario with a current signed review that pinned its latest evidence
    When new evidence for the scenario is added, alone or in a batch
    Then the add prints one line per review now stale naming the reviewer, the scenario and the `canon review add ... --pin <sha>` command that re-pins it
    And running that command makes the review current again
    And evidence for a scenario with no current pinned review prints no such line
