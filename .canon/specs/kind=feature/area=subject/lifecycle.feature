Feature: subject lifecycle
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:subject-lifecycle
  @case:happy
  @subject.lifecycle.01
  Scenario: A new subject is born proposed and reads back through the routed tier
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repo with no subjects
    When one is authored with a domain and a title
    Then querying subjects returns exactly that one
    And its status is proposed, the only state a subject can start in

  @subject:subject-lifecycle
  @case:happy
  @subject.lifecycle.02
  Scenario: Adopting a change links it to the subject on both sides
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a change on the ledger and a subject
    When the change is adopted into the subject
    Then the subject lists that change
    And the change names that subject, so neither side has to be inferred from the other

  @subject:subject-lifecycle
  @case:happy
  @subject.lifecycle.03
  Scenario: The forward chain advances one rung at a time and every write folds to one row
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a proposed subject
    When it is moved to specced, then building, then verifying
    Then every transition is admitted
    And the four writes read back as one row whose status is verifying
    And a re-write appends a version rather than mutating one, and the reader folds to the latest

  @subject:subject-lifecycle
  @case:failure
  @subject.lifecycle.04
  Scenario: A transition off the chain is refused and the subject is unchanged
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a proposed subject
    When it is moved straight to shipped
    Then the command refuses as a usage error naming the transition as invalid
    And the subject still reads proposed

  @subject:subject-lifecycle
  @case:edge
  @subject.lifecycle.05
  Scenario: Any state but retired may retire and retired is terminal
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given each of proposed, specced, building, verifying and shipped
    When retiring from that state is considered
    Then it is allowed from every one of them, so retirement is not a rung on the chain
    And retiring from retired is refused, because there is nowhere left to go

  @subject:subject-lifecycle
  @case:failure
  @subject.lifecycle.06
  Scenario: Shipping fails closed while a linked scenario carries no verdict at all
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a verifying subject linked to one scenario with no evidence
    When it is moved to shipped
    Then the command fails, reporting the uncovered cell by failure class and naming the scenario
    And the subject still reads verifying, so absent evidence is never read as passing evidence

  @subject:subject-lifecycle
  @case:failure
  @subject.lifecycle.07
  Scenario: Shipping is blocked while a linked scenario's latest verdict is divergent
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a verifying subject linked to one scenario whose latest verdict is divergent
    When it is moved to shipped
    Then the command fails and names that scenario
    And the subject still reads verifying

  @subject:subject-lifecycle
  @case:happy
  @subject.lifecycle.08
  Scenario: Shipping is admitted once every linked scenario has a faithful latest verdict
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a verifying subject linked to one scenario whose latest verdict is faithful
    When it is moved to shipped
    Then the transition succeeds and the subject reads shipped
    And this one rung of the chain is the only one evidence gates

  @subject:subject-lifecycle
  @case:failure
  @subject.lifecycle.09
  Scenario: A domain outside the vocabulary this repo declares is refused naming the legal set
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repo whose activated vocabulary declares its own domain members
    When a subject is authored under a near miss of one of them
    Then the command refuses, naming both the offending value and the members it expected
    And no subject was written, so a typo can never mint a new domain

  # canon: {"schema":1,"at":"2026-10-07T03:21:21Z","actor":{"agent_id":"canon"}}
  @subject:subject-lifecycle
  @case:failure
  @subject.lifecycle.10
  Scenario: Shipping is refused when no scenario is tagged to the subject
    Given a verifying subject that no scenario names in an @subject tag
    When it is moved to shipped
    Then the command fails, reporting an uncovered cell that names the missing tag
    And the subject still reads verifying, so an empty scenario set is never read as passing evidence

  # canon: {"schema":1,"at":"2026-10-08T14:02:46Z","actor":{"agent_id":"canon"}}
  @subject:subject-lifecycle
  @case:failure
  @subject.lifecycle.11
  Scenario: Shipping is refused until each owned surface specifies a required case
    Given a verifying subject whose only scenario is attested but specifies no failure path
    And a spec_coverage policy that requires the failure case
    When it is moved to shipped
    Then the command exits 1 reporting the surface and the missing case, and the subject stays verifying
    And once an attested failure scenario is added on that surface the move succeeds

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:subject-lifecycle
  @case:failure
  @subject.lifecycle.12
  Scenario: With require_review, entering its scope is refused while an owned scenario has no review
    Given a building subject whose only scenario is attested by its implementer and never reviewed
    And a spec_coverage policy whose require_review section takes its defaults
    When it is moved to verifying
    Then the command exits 1 reporting unreviewed-promotion for that scenario and naming --override-reason
    And it prints which review checks it ran, and the subject stays building

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:subject-lifecycle
  @case:failure
  @subject.lifecycle.13
  Scenario: A review authored by the evidence actor does not count under distinct_actor
    Given a building subject whose scenario's only review was authored by the actor who attested its evidence
    When it is moved to verifying
    Then the command exits 1 naming that actor and the distinct_actor rule
    And the subject stays building

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:subject-lifecycle
  @case:happy
  @subject.lifecycle.14
  Scenario: A review by a distinct actor admits the move with no waiver recorded
    Given a building subject whose scenario was reviewed by someone other than its evidence actor
    When it is moved to verifying
    Then the move succeeds and the subject carries no status_override
    And the gate checks the repository green

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:subject-lifecycle
  @case:failure
  @subject.lifecycle.15
  Scenario: An open blocker finding on an adopted change refuses the move until it is closed as fixed
    Given a reviewed building subject whose adopted change carries an open blocker finding
    When it is moved to verifying
    Then the command exits 1 reporting open-blocker for that finding
    And once the latest version of the same finding is fixed the move succeeds

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:subject-lifecycle
  @case:edge
  @subject.lifecycle.16
  Scenario: An override reason moves the subject, records the waiver and keeps the gap visible
    Given a building subject with an unreviewed scenario under require_review
    When it is moved to verifying with an override reason and an actor
    Then the move succeeds and the subject records the reason, the waived classes and the actor
    And the gate stays green but lists the gap as an advisory naming the waiver
    And the next status write drops the waiver

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:subject-lifecycle
  @case:failure
  @subject.lifecycle.17
  Scenario: An override never waives the ship gate and a blank reason is refused
    Given a verifying subject whose scenario has no verdict and no review
    When it is moved to shipped with an override reason
    Then the command exits 1, because the override waives only the review checks
    And a blank override reason is refused as a usage error

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:subject-lifecycle
  @case:edge
  @subject.lifecycle.18
  Scenario: Without require_review the guard is silent and review records change nothing
    Given a spec_coverage policy with no require_review section
    When a subject with unreviewed scenarios and an open blocker finding is moved to verifying
    Then the move succeeds with exactly the pre-0.12 output and no status_override
    And the gate output is byte-identical to the same repository without any review records

  # canon: {"schema":1,"at":"2026-10-09T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:subject-lifecycle
  @case:edge
  @subject.lifecycle.19
  Scenario: The review guard says which checks it skipped outside its scope
    Given a spec_coverage policy whose require_review section takes its defaults
    When a proposed subject is moved to specced
    Then the move succeeds
    And the guard reports both review checks as skipped because specced is not in its scope

  # canon: {"schema":1,"at":"2026-10-10T15:43:50Z","actor":{"agent_id":"canon"}}
  @subject:subject-lifecycle
  @case:edge
  @subject.lifecycle.20
  Scenario: One record keyed by a task and a scenario ships the scenario and still flips the task
    Given a verifying subject owning one scenario, and a plan task
    When one record keyed by both the task and the scenario is added and promoted
    Then the subject ships, because the scenario cell has that record's faithful verdict
    And the same record still flips the task's checkbox

  # canon: {"schema":1,"at":"2026-10-10T16:01:46Z","actor":{"agent_id":"canon"}}
  @subject:subject-lifecycle
  @case:edge
  @subject.lifecycle.22
  Scenario: Adopting a change that is already linked writes nothing
    Given a change and a subject that already carry the adoption link on both sides
    When the change is adopted into the subject again
    Then it exits 0 saying the change is already linked and nothing was written
    And no record is added to the ledger

  # canon: {"schema":1,"at":"2026-10-10T16:01:46Z","actor":{"agent_id":"canon"}}
  @subject:subject-lifecycle
  @case:failure
  @subject.lifecycle.23
  Scenario: An adoption that fails between its two writes names what was written and the command that completes it
    Given a change record that cannot be written while the subject record can
    When the change is adopted into the subject
    Then the subject record, which the gate reads adopted changes from, is written first
    And the command exits 2 naming the subject that now lists the change and the exact command that completes the link
    And running that command completes the link, and running it again writes nothing

  # canon: {"schema":1,"at":"2026-10-11T17:15:04Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:failure
  @subject.lifecycle.24
  Scenario: Shipping is refused while the brief has no Approved line, even with every technical gate passing
    Given a verifying subject whose every scenario is evidenced and reviewed, with no blocker or case gap
    And its newest adopted change's .canon/changes brief.md carries no Approved line
    When the subject is moved to shipped
    Then the command exits 1 with an unreviewed-promotion naming the subject and that brief.md
    And the refusal says a human must answer the brief and the agent records the Approved line only from that answer
    And the subject stays verifying, and status reports the same brief as not approved
    And once the line is recorded the same move ships the subject

  # canon: {"schema":1,"at":"2026-10-11T17:15:05Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:failure
  @subject.lifecycle.25
  Scenario: An Approved line inside an HTML comment or a code fence does not let the subject ship
    Given a verifying subject that passes every technical gate
    And its brief.md carries an Approved line only inside an HTML comment or a fenced code block
    When the subject is moved to shipped
    Then the command exits 1 naming the brief.md as having no Approved line
    And status reports the brief as not approved, so status and the guard agree

  # canon: {"schema":1,"at":"2026-10-11T17:15:05Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:failure
  @subject.lifecycle.26
  Scenario: An override ships a subject with an unapproved brief and records the waiver
    Given a verifying subject that passes every technical gate but whose brief has no Approved line
    When the subject is moved to shipped with an override reason
    Then the subject ships and the command names the waived brief check
    And the subject record carries the reason, the actor and the waived unreviewed-promotion naming the subject

  # canon: {"schema":1,"at":"2026-10-12T03:00:00Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:edge
  @subject.lifecycle.27
  Scenario: A subject with no adopted change ships with an advisory that its brief is not approved
    Given a verifying subject that passes every technical gate and adopts no change
    When the subject is moved to shipped
    Then it ships, and the command prints one advisory that the brief is not approved because there is no brief to answer
    And no waiver is recorded
    And status shows the brief as not approved and not enforced, with no human step

  # canon: {"schema":1,"at":"2026-10-12T12:00:00Z","actor":{"agent_id":"017-p2-impl"}}
  @subject:subject-lifecycle
  @case:failure
  @subject.lifecycle.28
  Scenario: Shipping a change from an external plan needs its canon brief
    Given a verifying subject that passes every technical gate, whose newest adopted change comes from an external openspec plan with no canon brief
    When the subject is moved to shipped
    Then the command exits 1 with an unreviewed-promotion naming .canon/changes/<change>/brief.md to create, with a "Source:" line naming the external proposal.md, and the approval to record there
    And the "Approved:" line inside the external proposal.md is not read, and status reports the same brief as enforced and not approved
    And once the canon brief.md holds the "Source:" and "Approved:" lines the same move ships the subject, and the external proposal.md is never edited

  # canon: {"schema":1,"at":"2026-10-12T18:00:00Z","actor":{"agent_id":"018-n2-impl"}}
  @subject:subject-lifecycle
  @case:happy
  @subject.lifecycle.29
  Scenario: A shipped subject reopens inside its approved scope and records why
    Given a shipped subject whose newest adopted change's brief.md carries an Approved line (with a digest that still matches the brief, or a pre-0.19 line with none)
    When it is moved to verifying with a reason and an actor
    Then it moves with no human, and the review guard says it skipped its checks because a reopen moves the subject back to fix its gaps
    And the new record carries the reason, the actor and the approved change the follow-up stays in
    And the next status write clears that note, so it belongs to the reopen that wrote it

  # canon: {"schema":1,"at":"2026-10-12T18:00:01Z","actor":{"agent_id":"018-n2-impl"}}
  @subject:subject-lifecycle
  @case:failure
  @subject.lifecycle.30
  Scenario: Reopening outside an approved scope is refused naming the brief to approve
    Given a shipped subject that adopted a newer change whose brief.md has no Approved line and that one of its scenarios is tagged @flow:<change>.* for
    When it is moved to verifying with a reason
    Then the command exits 1 with an unreviewed-promotion naming that brief.md and saying a human must answer it
    And an override reason does not waive it, and the subject stays shipped
    And once the brief records the human's approval the same reopen goes through

  # canon: {"schema":1,"at":"2026-10-12T18:00:02Z","actor":{"agent_id":"018-n2-impl"}}
  @subject:subject-lifecycle
  @case:failure
  @subject.lifecycle.31
  Scenario: A reopen needs a reason and a reason needs a reopen
    Given a shipped subject
    When it is moved to verifying with no reason, or with a blank one
    Then the command refuses as a usage error saying the reopen needs one line of reason
    And a reason given to any other transition is refused the same way
    And the subject stays shipped

  # canon: {"schema":1,"at":"2026-10-12T18:00:03Z","actor":{"agent_id":"018-n2-impl"}}
  @subject:subject-lifecycle
  @case:failure
  @subject.lifecycle.32
  Scenario: Shipping a reopened subject runs every guard again
    Given a reopened subject whose follow-up added an owned scenario with no evidence and found an open blocker on its change
    When it is moved to shipped
    Then the command exits 1 naming the uncovered scenario, the unreviewed scenario and the open blocker, and the subject stays verifying
    And once the blocker is fixed and the scenario evidenced and reviewed, the same move ships it

  # canon: {"schema":1,"at":"2026-10-12T18:00:04Z","actor":{"agent_id":"018-n2-impl"}}
  @subject:subject-lifecycle
  @case:happy
  @subject.lifecycle.33
  Scenario: A change adopted by a second subject belongs to both and its open blocker gates both
    Given a change written by canon change new under one subject
    When it is adopted under a second subject
    Then the command says the change now belongs to both subjects, naming each
    And both subjects list the change, and the change lists both, never re-pointed
    And status counts an open blocker on the change against both, and it refuses each subject's move into review scope

  # canon: {"schema":1,"at":"2026-10-12T18:00:05Z","actor":{"agent_id":"018-n2-impl"}}
  @subject:subject-lifecycle
  @case:edge
  @subject.lifecycle.34
  Scenario: A change written before 0.18 with one subject reads as a one-element list
    Given a stored change written before 0.18 that names its one subject in subject_id
    When the changes are queried
    Then the change shows that subject as a one-element subject_ids
    And adopting it under a second subject appends that subject and writes the change as the list at the current schema generation

  # canon: {"schema":1,"at":"2026-10-21T11:58:47Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:subject-lifecycle
  @case:happy
  @subject.lifecycle.35
  Scenario: Adopting an unapproved change into a building subject warns and says how to undo it
    Given a proposed, a specced and a building subject, and a change with a draft brief.md
    When the change is created under, or adopted by, each subject
    Then the building, verifying or shipped subject (the shipped one adopting into a shipped subject too) gets one warning on stderr and in JSON meta.warnings naming canon subject unadopt, and the proposed and specced ones none
    And an approved brief adopted into a building subject gets none

  # canon: {"schema":1,"at":"2026-10-21T11:58:51Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:subject-lifecycle
  @case:happy
  @subject.lifecycle.36
  Scenario: Unadopting a change removes the link on both sides
    Given a verifying subject adopting an approved change and a draft change, and a holding subject
    When the draft change is unadopted from the subject
    Then the subject's change list and the change's subject list both drop the link, canon status no longer asks about that brief for the subject, and the holding subject can adopt the change
    And unadopting an unlinked pair writes nothing and exits 0, and the result carries meta.next

  # canon: {"schema":1,"at":"2026-10-21T11:58:56Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:subject-lifecycle
  @case:failure
  @subject.lifecycle.37
  Scenario: A change with an approved brief is not unadopted
    Given a building subject adopting a change whose brief.md carries an "Approved:" line
    When the change is unadopted from that subject
    Then the command exits 2 with SUBJECT_UNADOPT_REFUSED, its hint naming the approved scope, and the link stays
    And a draft change a scenario of the subject is tagged @flow:<change>.* for is refused the same way, its hint naming the scenarios and saying to remove or retag them first

  # canon: {"schema":1,"at":"2026-10-21T12:38:43Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:subject-lifecycle
  @case:happy
  @subject.lifecycle.38
  Scenario: A pending draft change does not freeze a shipped subject's reopen, evidence, review and ship
    Given a shipped subject adopting an approved change and a newer draft change that none of its scenarios is tagged @flow:<change>.* for
    When the subject is reopened, a scenario is re-verified and reviewed, and the subject ships again
    Then each step is accepted, the reopen and ship output say in one line that the draft is a pending draft not part of the transition, and canon status still lists the draft awaiting approval, marked as not yet part of the subject's scope
    And once a scenario of the subject is tagged @flow:<draft>.F1, shipping is refused on the draft's brief
