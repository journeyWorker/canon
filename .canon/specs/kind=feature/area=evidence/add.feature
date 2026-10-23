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
  Scenario: A scenario id with no project id is inferred from the one spec root and refused when several are configured
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an add naming a scenario id and omitting the project id, once in a repository with one spec root and once in a repository with two
    When it is invoked in each
    Then the repository with one root takes that root as the project id and writes the record
    And the repository with two roots exits two and writes nothing, rather than writing a record for the gate to silently ignore

  @subject:evidence-attestation
  @case:failure
  @evidence.add.06
  Scenario: An empty evidence class or reference is refused rather than written
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an add whose class is only whitespace, and another whose reference is empty
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
    Then it is refused before staging, even when --approver and --approval-role are both supplied
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
  Scenario: The artifact store keeps the bytes of records written before the store existed
    Given a committed record whose bound report has no stored blob but is unchanged in the working tree
    When `canon artifacts store` runs
    Then it stores the report's bytes under its digest and says it stored one file
    And the gate stops advising `canon artifacts store`, and a second run stores nothing new

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
    When evidence add or review scenario is run with it
    Then the command exits 2 naming --session-id
    And nothing is staged or committed

  # canon: {"schema":1,"at":"2026-10-10T16:02:33Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.21
  Scenario: Every write command either stages its record or writes directly, as its mode says
    Given the write commands evidence add, finding add and close, review scenario, divergence add and resolve, subject new, adopt and move, change new, scenario new and feature new
    When each succeeds
    Then a direct write leaves nothing staged: feature new, scenario new, change new, subject new, adopt and move, and review scenario
    And a staged write run with --no-promote leaves its record staged for `canon gate promote`
    And without --no-promote the staged writers promote their own record in the same run, so nothing is left to promote

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
  @case:happy
  @evidence.add.40
  Scenario: A record binds exactly the files its own write names
    Given a scenario whose latest evidence record binds two files
    When evidence is added for it binding only one of them
    Then the add is accepted and the new record binds that one file
    And nothing from the earlier record is carried into it, and no drop is recorded or listed by canon status

  # canon: {"schema":1,"at":"2026-10-10T19:35:32Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.41
  Scenario: Binding a file again with new bytes binds the new digest
    Given a scenario whose latest evidence record binds a screenshot and a log
    When evidence is added binding the screenshot again with new bytes
    Then the new record binds the screenshot at its new digest and nothing else

  # canon: {"schema":1,"at":"2026-10-10T19:35:32Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.42
  Scenario: A batch binds only what it names and prints no carried list
    Given a scenario whose latest evidence binds screenshots and a test report that names the scenario
    When the suite is recorded with --all-scenarios and that report
    Then the new record binds the report alone and the batch output has no carried list

  # canon: {"schema":1,"at":"2026-10-10T19:35:33Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.43
  Scenario: The drop flags no longer exist
    Given a scenario with bound evidence
    When evidence is added with --drop-attachment or --drop-reason
    Then each add exits 2 as an unknown flag and nothing is staged

  # canon: {"schema":1,"at":"2026-10-10T19:35:33Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.44
  Scenario: Audit options are required on evidence audit and refused on evidence add
    Given a scenario
    When an audit is recorded with `canon evidence audit` without the evaluator options, or with a task id, a verdict, an unknown result, or a digest that is not a sha256, or `canon evidence add` is given the evaluator options or the audit class
    Then each invocation exits 2 with nothing on stdout, and the evidence add refusals name `canon evidence audit`
    And nothing is staged

  # canon: {"schema":1,"at":"2026-10-10T19:35:33Z","actor":{"agent_id":"canon"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.45
  Scenario: An audit is recorded beside the declared verdict and never replaces it
    Given a scenario with a faithful declared record
    When an external evaluator's refuted audit with a counterexample report is recorded with `canon evidence audit`
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
    When evidence add, finding add, finding close and divergence add run
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
    And init --check reports the write section as failed
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
    When evidence add and divergence add run under write promote auto
    Then each run promotes and names its own record only
    And the other author's records are still staged
    And gate promote commits them afterwards

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
    When evidence add, finding add, finding close and divergence add run
    Then each prints one success line, naming the record's committed file under the ledger, which exists
    And no line names the staged file promotion moved, and no separate promote line is printed
    And with --no-promote, evidence add, finding add and divergence add print the staged file and canon gate promote

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
    And canon review scenario and canon finding add for the same scenario and change are not refused

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
    And the external proposal.md is never edited, and canon task done flips the external task row

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
    And --all-scenarios with --scenario-id, --task-id, --report-case or an approval, or without --report, is refused with exit 2

  # canon: {"schema":1,"at":"2026-10-12T15:00:00Z","actor":{"agent_id":"018-n1-impl"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.84
  Scenario: A task's report with no scenario id binds every case in it
    Given a task row and a report whose cases carry no scenario id
    When evidence is added with --task-id and --report and no --scenario-id
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
    And a suite whose end reports a failure no test inside it recorded fails every test inside it, while a suite whose end only reports a failed test leaves that test's siblings as they ended
    And a stream that is not JSON lines, or in which no test ran, is refused with exit 2

  # canon: {"schema":1,"at":"2026-10-12T15:00:00Z","actor":{"agent_id":"018-n1-impl"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.86
  Scenario: A write that makes a pinned review stale names the review and its re-pin command
    Given a scenario with a current signed review that pinned what its latest evidence said
    When new evidence for the scenario with a different verdict or different checks is added, alone or in a batch
    Then the add prints one line per review now stale naming the reviewer, the scenario and the `canon review scenario` command that re-pins it
    And that command keeps the review's session when it recorded one
    And running that command makes the review current again
    And evidence that says the same, with the same verdict from the same checks, leaves the review current and prints no such line
    And evidence for a scenario with no current pinned review prints no such line

  # canon: {"schema":1,"at":"2026-10-12T18:00:00Z","actor":{"agent_id":"018-n3-impl"}}
  @subject:evidence-attestation
  @case:edge
  @evidence.add.90
  Scenario: A marker inside a path, a file name, a URL or a longer word is not a fabrication marker
    Given a scenario and a summary naming "pack.json/atlas JSON", "tbd_notes.md" and "https://example.com/tbd"
    When evidence is added for the scenario with that summary
    Then the add succeeds and the committed record carries the summary as given
    And a summary where "n/a" stands on its own is still refused with exit 1 and nothing is written

  # canon: {"schema":1,"at":"2026-10-12T18:00:00Z","actor":{"agent_id":"018-n3-impl"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.91
  Scenario: A fabrication marker is refused the way every other gate-red refusal of evidence add is
    Given a summary holding "TBD" and a command result reading "n/a"
    When evidence is added with them
    Then the command exits 1 with the code EVIDENCE_FABRICATED and one line per finding, each naming fabricated-evidence, the scenario, the field and the marker
    And nothing is staged or committed

  # canon: {"schema":1,"at":"2026-10-18T09:00:00Z","actor":{"agent_id":"attachments-impl"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.93
  Scenario: A batch binds each test's own attachments to that test's scenario and no other
    Given a JUnit report whose test cases each list their own screenshot in a system-out attachment line, with paths relative to the report's directory
    When evidence is added with --report and --all-scenarios
    Then each scenario's record binds the report and the files its own cases name, and no file named by another scenario's case
    And a file two of one scenario's cases name is bound once, and every file is stored by digest

  # canon: {"schema":1,"at":"2026-10-18T09:00:00Z","actor":{"agent_id":"attachments-impl"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.94
  Scenario: A Cucumber embedding is stored by digest and bound to the scenario it belongs to
    Given a Cucumber JSON report whose scenario steps embed base64 screenshots
    When evidence is added with --report cucumber:<path> and --all-scenarios
    Then each scenario's record binds a named attachment for each of its own embeddings, whose digest is the decoded bytes
    And the decoded bytes are in the artifact store

  # canon: {"schema":1,"at":"2026-10-18T09:00:00Z","actor":{"agent_id":"attachments-impl"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.95
  Scenario: A bare artifact is refused in a batch because it would bind to every scenario
    Given indexed scenarios and a report that owns a case for several of them
    When evidence is added with --all-scenarios and --artifact <path>
    Then the add exits 2 with the code EVIDENCE_INVALID_ARGUMENT and a hint naming per-test report attachments and --artifact <scenario-id>=<path>
    And nothing is staged, committed or stored

  # canon: {"schema":1,"at":"2026-10-18T09:00:00Z","actor":{"agent_id":"attachments-impl"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.96
  Scenario: An artifact scoped to a scenario binds to that scenario's record alone
    Given indexed scenarios and a report that owns a case for several of them
    When evidence is added with --all-scenarios and --artifact <scenario-id>=<path>
    Then only that scenario's record binds the file
    And a scenario that has no record in the batch is refused with exit 2 and nothing is written
    And a single-scenario add accepts --artifact naming its own scenario and refuses one naming another

  # canon: {"schema":1,"at":"2026-10-18T09:00:00Z","actor":{"agent_id":"attachments-impl"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.97
  Scenario: A report attachment that is missing, outside the repository or over the size limit refuses the bind
    Given a report whose case names an attachment that is not on disk, one outside the repository, or one larger than --max-artifact-mib
    When evidence is added from that report, in a batch or for one scenario
    Then the add exits 2 naming the case and the attachment path
    And nothing is staged, committed or stored

  # canon: {"schema":1,"at":"2026-10-18T09:00:00Z","actor":{"agent_id":"attachments-impl"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.98
  Scenario: A single scenario's report binds the attachments of the cases matched to it
    Given a report with attachments on the cases of two scenarios
    When evidence is added for one scenario with --scenario-id and --report
    Then the record binds the report and the files of that scenario's cases only

  # canon: {"schema":1,"at":"2026-10-18T09:00:00Z","actor":{"agent_id":"attachments-impl"}}
  @subject:evidence-attestation
  @case:edge
  @evidence.add.99
  Scenario: A re-run with new screenshot bytes leaves a pinned review current
    Given a scenario with a current signed review that pinned its report evidence, the cases having screenshots
    When the suite runs again and evidence is added with the same cases and verdicts but new screenshot bytes
    Then the new record binds the new bytes, prints no stale-review line, and the review stays current

  # canon: {"schema":1,"at":"2026-10-21T11:58:32Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.100
  Scenario: A draft change adopted into a building subject freezes only the scenarios tagged with it
    Given a verifying subject with an approved change A, a scenario tagged @flow:A.F1, an untagged scenario, and a task of A, then a draft change B adopted
    When evidence is added for each of them, and for a new scenario tagged @flow:B.F1
    Then the adoption warns, and the untagged scenario, the A-tagged scenario and A's task are accepted
    And the B-tagged scenario is refused BRIEF_UNAPPROVED naming B's brief.md, and moving the subject to shipped still refuses on B
    And once B is approved the B-tagged scenario is accepted

  # canon: {"schema":1,"at":"2026-10-21T11:58:37Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.101
  Scenario: A subject whose only change is unapproved, or whose approval was edited, still refuses evidence
    Given a verifying subject whose only change has a brief.md with no "Approved:" line, or an approved one edited since
    When evidence is added for an untagged scenario and for one tagged @flow: with that change
    Then each add exits 1 with BRIEF_UNAPPROVED, or BRIEF_CHANGED_SINCE_APPROVAL for the edited approval, naming that brief.md

  # canon: {"schema":1,"at":"2026-10-21T11:58:42Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:evidence-attestation
  @case:edge
  @evidence.add.102
  Scenario: A subject built before briefs keeps re-verifying its evidenced scenarios under a draft change
    Given a verifying subject with scenarios already evidenced and no change, which then adopts its first change, a draft
    When evidence is added again for an evidenced untagged scenario, and for one that was never evidenced
    Then the evidenced one is accepted, being existing behavior attested before any approved scope and before the subject adopted its first change
    And evidence recorded only after that adoption, for example while the subject was specced, does not exempt the scenario
    And the never-evidenced one is refused BRIEF_UNAPPROVED naming the draft change's brief.md

  # canon: {"schema":1,"at":"2026-10-21T16:35:00Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.103
  Scenario: A project id that names no configured spec root is refused before anything is written
    Given a repo whose canon.yaml configures one spec root, game, and a scenario indexed under it
    When evidence add names a --project-id that is no configured root, for one scenario and with --all-scenarios
    Then each add exits 2 with UNKNOWN_SPEC_ROOT before anything is staged, bound or written
    And the hint names the one root to use, `--project-id game`, and the same add under game is accepted

  # canon: {"schema":1,"at":"2026-10-21T16:35:10Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:evidence-attestation
  @case:edge
  @evidence.add.104
  Scenario: The refusal of an unknown project id lists every configured spec root
    Given a repo whose canon.yaml configures two spec roots
    When evidence add names a --project-id that matches neither
    Then it exits 2 with UNKNOWN_SPEC_ROOT, and the message, the hint and the JSON expected list name both roots
    And a root id that is configured is accepted whichever order the roots are listed in

  # canon: {"schema":1,"at":"2026-10-21T16:35:18Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.105
  Scenario: An approval payload is not issued for a project id that names no configured spec root
    Given a repo whose canon.yaml configures one spec root, game
    When evidence approval-payload names a --project-id that is no configured root
    Then it exits 2 with UNKNOWN_SPEC_ROOT and prints no payload bytes
    And the payload for the configured root is still printed

  # canon: {"schema":1,"at":"2026-10-23T12:02:58Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.106
  Scenario: One evidence add document records one attestation and an array or NDJSON records one per element
    Given a git work tree holding two indexed scenarios
    When evidence add is given its arguments as one JSON object with --input, then as a JSON array, then as NDJSON from a file
    Then the object writes one record and returns the ordinary report
    And the array and the NDJSON each return a batch report with one item per element, in order, each written
    And an array of one element is still a batch

  # canon: {"schema":1,"at":"2026-10-23T12:03:02Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.107
  Scenario: A batch with one invalid element writes nothing and points at the element
    Given a batch of two evidence documents whose second carries a value outside its closed set, or lacks a required key
    When evidence add is run on it with --input
    Then it exits 2 and the error's field points at that element and key, such as /items/1/verdict
    And nothing is written, not even the valid first element

  # canon: {"schema":1,"at":"2026-10-23T12:03:06Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:evidence-attestation
  @case:edge
  @evidence.add.108
  Scenario: Repeating an evidence add that already landed writes nothing and is reported unchanged
    Given an evidence add that already landed
    When the same add is repeated, alone or as a batch
    Then it exits 0 and the report says unchanged, each batch element being unchanged
    And nothing is written, and the ledger still holds the one record

  # canon: {"schema":1,"at":"2026-10-23T12:03:11Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.109
  Scenario: A record names the commit it is about, HEAD by default, with the code tree it saw
    Given a scenario in a git work tree and an evidence add that names no revision
    When it is invoked
    Then the record names the full sha that HEAD resolves to
    And it carries the code tree of the code it saw and a worktree-dirty flag of false

  # canon: {"schema":1,"at":"2026-10-23T12:03:15Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:evidence-attestation
  @case:edge
  @evidence.add.110
  Scenario: Uncommitted code is recorded as dirty and never refused
    Given a git work tree whose code differs from HEAD, with a new untracked file
    When evidence is added for a scenario
    Then the add succeeds and the record names HEAD and carries a worktree-dirty flag of true
    And a review made on the same working tree records the code tree of what it saw, which differs from HEAD's
    And once that working tree is committed the commit's code tree equals the one the review recorded, so the review covers it

  # canon: {"schema":1,"at":"2026-10-23T12:03:19Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.111
  Scenario: Outside git, or before the first commit, an evidence add is refused with NO_COMMIT
    Given a directory with no git repository, and a git repository with no commit, each holding an indexed scenario
    When evidence add runs in each
    Then it exits 2 with NO_COMMIT and a hint to run git init and commit
    And nothing is written

  # canon: {"schema":1,"at":"2026-10-23T12:03:23Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:evidence-attestation
  @case:happy
  @evidence.add.112
  Scenario: A bound report decides the verdict of a single-scenario add
    Given a scenario and three JUnit reports that name it: one where every case passed, one with a skipped case and none failed, one with a failed case
    When evidence is added for the scenario with each report and no --verdict
    Then the passing report records faithful, the report with a skipped case and none failed records not-applicable, and the report with a failed case records divergent
    And each add succeeds, because the report decided the verdict and nothing contradicts it

  # canon: {"schema":1,"at":"2026-10-23T12:03:28Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.113
  Scenario: An explicit faithful verdict over a failed or skipped case is refused and nothing is written
    Given a scenario and a report whose case for it is skipped, and another whose case failed
    When evidence is added for the scenario with each report and --verdict faithful
    Then each add exits 1 and nothing is written
    And the same adds without --verdict succeed and record the verdict the report decides

  # canon: {"schema":1,"at":"2026-10-23T12:03:34Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:evidence-attestation
  @case:edge
  @evidence.add.114
  Scenario: An override is recorded, echoed in the add's overrides and listed by status
    Given a subject owning a scenario whose change brief has no answer from the human
    When evidence is added for the scenario, then again with --override-reason
    Then the first add is refused with exit 1, and the second succeeds
    And the JSON report lists the waiver in its overrides, with the reason and the code of the refusal it waived
    And canon status lists the override with its reason

  # canon: {"schema":1,"at":"2026-10-23T13:17:29Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:evidence-attestation
  @case:failure
  @evidence.add.115
  Scenario: An evidence batch whose second record fails to stage or promote leaves nothing behind
    Given a batch of two records and one artifact, whose second record fails to stage in one run and whose second promotion is refused in another
    When each batch is landed
    Then no record is left staged or committed and no new blob is left, the error naming the second record
    And a read-only destination is refused before anything is written
    And the same batch with nothing failing lands whole, both records committed and the blob stored
