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
    And the command exits 1 with CHANGE_ADOPTION_INCOMPLETE naming the subject that now lists the change and the exact command that completes the link
    And running that command completes the link, and running it again writes nothing

  # canon: {"schema":1,"at":"2026-10-11T17:15:04Z","actor":{"agent_id":"canon-scaffold"}}
  @subject:subject-lifecycle
  @case:failure
  @subject.lifecycle.24
  Scenario: Shipping is refused while the brief has no Approved line, even with every technical gate passing
    Given a verifying subject whose every scenario is evidenced and reviewed, with no blocker or case gap
    And its adopted change's .canon/changes brief.md carries no Approved line, and the subject answers for it: a scenario is tagged for it, it is a chore, or no other change of the subject is approved
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
    Given a shipped subject whose every change it answers for (tagged by a scenario, with an Approved line, or a chore) has a brief.md with an Approved line (with a digest that still matches the brief, or a pre-0.19 line with none)
    When it is moved to verifying with a reason and an actor
    Then it moves with no human, and the review guard says it skipped its checks because a reopen moves the subject back to fix its gaps
    And the new record carries the reason, the actor and the newest approved change the follow-up stays in
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
  Scenario: A reopen needs a reason and any other transition may carry one
    Given a shipped subject
    When it is moved to verifying with no reason, or with a blank one
    Then the command refuses as a usage error saying the reopen needs one line of reason
    And the subject stays shipped
    And a reason given to any other move is accepted and recorded on the subject with the actor, and the next move without one clears it

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
    Given a shipped subject adopting an approved change and a newer draft change that was never approved, is no chore, and that none of its scenarios is tagged @flow:<change>.* for
    When the subject is reopened, a scenario is re-verified and reviewed, and the subject ships again
    Then each step is accepted, the reopen and ship output say in one line that the draft is a pending draft the transition does not read, and canon status lists the draft in pendingDrafts, not in briefs, marked as not yet part of the subject's scope, without saying evidence add refuses the subject's scenarios
    And once a scenario of the subject is tagged @flow:<draft>.F1, shipping is refused on the draft's brief, and so it is once the draft's brief is approved and then edited

  # canon: {"schema":1,"at":"2026-10-21T13:15:05Z","actor":{"agent_id":"canon-impl-025"}}
  @subject:subject-lifecycle
  @case:failure
  @subject.lifecycle.39
  Scenario: A tagged unapproved change still blocks reopen and ship under a newer untagged draft
    Given a shipped subject adopting an approved change a, a draft change b that one of its scenarios is tagged @flow:b.F1 for, and a newer draft change c no scenario is tagged for
    When the subject is reopened and, with b's scenario attested, shipped again
    Then both are refused naming b's brief, and the output names every pending draft, here c, as not read by the transition
    And without c the refusal is the same, and only a change never approved, no chore and tagged by no scenario is ever a pending draft
    And canon status does not advise the refused reopen: it says reopening waits for the brief's approval

  # canon: {"schema":1,"at":"2026-10-26T11:42:43Z","actor":{"agent_id":"impl-030-sb-f4","role":"implementer","session_id":"sb-f4-2026-10-26"}}
  @subject:subject-lifecycle
  @case:happy
  @flow:canon-0-30-spec-backfill.F4
  @subject.lifecycle.40
  Scenario: Show prints one subject's record, status and owner and writes nothing
    Given a repo holding a proposed subject demo with domain dev, title Demo and owner role implementer
    When canon subject show demo runs, once with --json and once without
    Then both exit 0, and the --json data carries subject_id, title, domain, status proposed and owner_role
    And the human output names the subject with its domain and status, its title and its owner
    And no file in the repo changed

  # canon: {"schema":1,"at":"2026-10-26T11:42:43Z","actor":{"agent_id":"impl-030-sb-f4","role":"implementer","session_id":"sb-f4-2026-10-26"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-30-spec-backfill.F4
  @subject.lifecycle.41
  Scenario: Show of an unknown or missing subject id is refused with exit 2 and writes nothing
    Given a repo holding a subject demo and no subject nope
    When canon subject show runs with the id nope, and again with no id
    Then the first exits 2 with SUBJECT_NOT_FOUND naming nope, and the second exits 2 with USAGE
    And no file in the repo changed

  # canon: {"schema":1,"at":"2026-10-27T10:24:18Z","actor":{"agent_id":"impl-031-loop","role":"implementer","session_id":"loop-2026-10-27"}}
  @subject:subject-lifecycle
  @case:happy
  @flow:canon-0-31-loop.F3
  @subject.lifecycle.42
  Scenario: Reopen and ship judge every tagged change and pass once all are approved, in either order
    Given a shipped subject adopting two changes a and b whose briefs have no Approved line and each tagged @flow:<change>.F1 by one of its scenarios
    When a is approved and the subject is reopened, and then shipped again with b's scenario attested
    Then each is refused with an unreviewed-promotion naming b's brief.md as not approved, and a is not named
    And once both are approved, in either order, the same reopen and ship go through

  # canon: {"schema":1,"at":"2026-10-27T10:24:20Z","actor":{"agent_id":"impl-031-loop","role":"implementer","session_id":"loop-2026-10-27"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-31-loop.F3
  @subject.lifecycle.43
  Scenario: An approved tagged change edited afterwards, or one whose tags were removed, still refuses reopen and ship
    Given a shipped subject adopting two approved tagged changes
    When the brief of one is edited after its approval, and separately its scenarios' tags are removed
    Then reopen and ship refuse naming that change's brief.md as changed since its approval, BRIEF_CHANGED_SINCE_APPROVAL, and say it differs from a brief never approved
    And the other approved change is not named, and approving the edited brief again lets both go through

  # canon: {"schema":1,"at":"2026-10-27T10:24:21Z","actor":{"agent_id":"impl-031-loop","role":"implementer","session_id":"loop-2026-10-27"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-31-loop.F3
  @subject.lifecycle.44
  Scenario: An approved feature brief no scenario tags, edited afterwards, refuses ship and is named
    Given a verifying subject adopting an approved feature change no scenario is tagged for and an approved tagged change
    When the untagged change's brief is edited after its approval and the subject ships
    Then shipping refuses naming that change's brief.md as changed since its approval
    And the brief-structure check also runs over that change, and approving it again lets the subject ship

  # canon: {"schema":1,"at":"2026-10-27T10:24:22Z","actor":{"agent_id":"impl-031-loop","role":"implementer","session_id":"loop-2026-10-27"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-31-loop.F3
  @subject.lifecycle.45
  Scenario: An unapproved or edited chore refuses reopen and ship
    Given a shipped subject adopting a chore change whose brief has no Approved line, and another subject adopting an approved chore edited afterwards
    When each is reopened and, when verifying, shipped
    Then each is refused naming the chore's brief.md, the first as not approved and the second as changed since its approval
    And a reopen is not waived by --override-reason, shipping's brief refusal is, and the chore's after-approval checks are unchanged

  # canon: {"schema":1,"at":"2026-10-27T10:24:23Z","actor":{"agent_id":"impl-031-loop","role":"implementer","session_id":"loop-2026-10-27"}}
  @subject:subject-lifecycle
  @case:edge
  @flow:canon-0-31-loop.F3
  @subject.lifecycle.46
  Scenario: A never-approved untagged feature brief blocks neither reopen nor ship, and two unapproved governing changes are both named in one approve call
    Given a shipped subject adopting a never-approved feature change that no scenario is tagged for, and a second subject whose two tagged changes are both unapproved
    When the first is reopened, re-verified and shipped, and the second is reopened
    Then the first goes through every step and canon status lists that change as a pending draft
    And the second's refusal names both briefs and its hint is one canon brief approve line holding both ids

  # canon: {"schema":1,"at":"2026-10-27T12:05:01Z","actor":{"agent_id":"impl-031-bf","role":"implementer","session_id":"bf-2026-10-27"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-31-brief-format.F4
  @subject.lifecycle.47
  Scenario: Reopen and ship refuse a subject that answers for an old-format brief, and a change it does not answer for never blocks
    Given a verifying subject answering for an old-format brief, and another whose only old-format brief is a change it does not answer for
    When each is moved to shipped, and a shipped one is reopened
    Then the first is refused with OLD_BRIEF_FORMAT and stays where it is
    And the second moves, because the command does not judge that change

  # canon: {"schema":1,"at":"2026-10-30T11:53:55Z","actor":{"agent_id":"impl-relay","role":"implementer","session_id":"s-0-32-relay-impl"}}
  @subject:subject-lifecycle
  @case:happy
  @flow:canon-0-32-relay-approval.F1
  @subject.lifecycle.48
  Scenario: Brief show --body prints the brief's bytes and the digest an approval records
    Given a change whose brief file ends without an Approved line
    When canon brief show <change> --body runs, once without and once with --json, and then canon brief approve records the human's answer
    Then the plain output is the brief file's bytes followed by one last line digest: sha256:<hex>, and nothing is written
    And the digest is the one the Approved line records after the approval, and --json carries the brief text as body and the same value as digest

  # canon: {"schema":1,"at":"2026-10-30T11:53:59Z","actor":{"agent_id":"impl-relay","role":"implementer","session_id":"s-0-32-relay-impl"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-32-relay-approval.F1
  @subject.lifecycle.49
  Scenario: Brief show --body of an unknown change or an unreadable brief is refused with exit 2 and writes nothing
    Given a repo with no change named missing and a change whose brief.md is not UTF-8 text
    When canon brief show missing --body and canon brief show <broken> --body run
    Then each exits 2 with the code the same call without --body gives, and prints no brief text
    And no file in the repo changed

  # canon: {"schema":1,"at":"2026-10-30T11:54:02Z","actor":{"agent_id":"impl-relay","role":"implementer","session_id":"s-0-32-relay-impl"}}
  @subject:subject-lifecycle
  @case:happy
  @flow:canon-0-32-relay-approval.F2
  @subject.lifecycle.50
  Scenario: The planner guidance shows the brief, asks with the harness question tool and counts only an explicit approval
    Given the planner guide of the skill, its source and the reference the install projects from it
    When their approval stop is read
    Then it says to show the brief with canon brief show <change> --body to the human's view without retyping it, naming omp out:// and the visible tool output of harnesses without that channel
    And it says to ask with the harness question tool, naming ask for omp and AskUserQuestion for Claude Code, or in the chat when there is none, and to record with --relayed-via
    And divergence accept and the migrate commit get the same procedure

  # canon: {"schema":1,"at":"2026-10-30T11:54:07Z","actor":{"agent_id":"impl-relay","role":"implementer","session_id":"s-0-32-relay-impl"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-32-relay-approval.F2
  @subject.lifecycle.51
  Scenario: The guidance treats a conditional answer as a revision request and no longer says to report the command and wait
    Given the same guides
    When they say what counts as the human's answer
    Then only an explicit approval counts, a question or a condition is a revision request, and no answer records nothing
    And no sentence tells the planner to report the command to the human and wait, and canon skills check exits 0 once the projected stubs are installed

  # canon: {"schema":1,"at":"2026-10-30T11:54:12Z","actor":{"agent_id":"impl-relay","role":"implementer","session_id":"s-0-32-relay-impl"}}
  @subject:subject-lifecycle
  @case:happy
  @flow:canon-0-32-relay-approval.F3
  @subject.lifecycle.52
  Scenario: A planner's relayed approval of two briefs marks both Approved lines, and status, brief show and the dashboard show it
    Given two changes whose briefs pass the checks and a planner role declared with an actor
    When canon brief approve <a> <b> runs with --relayed-via ask, the human's name and answer, --actor and each change's shown digest
    Then both briefs carry an Approved line with (relayed by planner <actor> via ask) before the digest, and the call exits 0
    And canon status lists both approvals as relayed, canon brief show names the relay, and the dashboard briefs list carries them in relayed
    And the same name and answer approved by a human without the option differ in the line only by that marker

  # canon: {"schema":1,"at":"2026-10-30T11:54:15Z","actor":{"agent_id":"impl-relay","role":"implementer","session_id":"s-0-32-relay-impl"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-32-relay-approval.F3
  @subject.lifecycle.53
  Scenario: A relayed brief approval whose shown digest differs or is missing is refused with exit 2 and nothing is written
    Given two changes with approvable briefs, the first shown with a stale digest and the second shown with none
    When canon brief approve runs relayed for the first alone, for the second alone, and for both
    Then each exits 2, the first with RELAY_DIGEST_MISMATCH and the second with USAGE, and neither brief file nor the policy approval changed
    And an id whose digest is right is refused too when another id of the call is not

  # canon: {"schema":1,"at":"2026-10-30T11:54:17Z","actor":{"agent_id":"impl-relay","role":"implementer","session_id":"s-0-32-relay-impl"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-32-relay-approval.F3
  @subject.lifecycle.54
  Scenario: A relayed approval without the human's name, answer or an actor, or with no role declared, is refused with exit 2
    Given a planner role and a brief that passes the checks
    When canon brief approve is relayed with an empty --approver, an empty --note, no --actor and no CANON_ACTOR, and when --relayed-via is given with no role declared
    Then each exits 2 with USAGE naming the missing option, or APPROVAL_INCOMPLETE for an empty name or answer, and nothing is written

  # canon: {"schema":1,"at":"2026-10-30T11:54:20Z","actor":{"agent_id":"impl-relay","role":"implementer","session_id":"s-0-32-relay-impl"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-32-relay-approval.F3
  @subject.lifecycle.55
  Scenario: An agent role without the relay option is refused HUMAN_ONLY and the hint names the relay procedure
    Given each agent role in turn declared with --as
    When canon brief approve, canon policy approve and canon divergence accept run without --relayed-via
    Then each exits 2 with HUMAN_ONLY, and the hint names the relay procedure (canon brief show --body, the question tool, --relayed-via) before the human's own call
    And nothing is written

  # canon: {"schema":1,"at":"2026-10-30T11:54:23Z","actor":{"agent_id":"impl-relay","role":"implementer","session_id":"s-0-32-relay-impl"}}
  @subject:subject-lifecycle
  @case:happy
  @flow:canon-0-32-relay-approval.F3
  @subject.lifecycle.56
  Scenario: A relayed policy approval records its relay fields in the approval file and refuses the same way
    Given a repo with a policy and a planner role with an actor
    When canon policy approve runs with --relayed-via, --approver, --note and --actor, and without --note, and with a role but no --relayed-via
    Then the first exits 0 and .canon/policy.approval.json holds relay with role, actor and tool, and canon policy show --json reports it
    And the second exits 2 with USAGE and the third with HUMAN_ONLY, and the file is unchanged by both

  # canon: {"schema":1,"at":"2026-10-30T11:54:27Z","actor":{"agent_id":"impl-relay","role":"implementer","session_id":"s-0-32-relay-impl"}}
  @subject:subject-lifecycle
  @case:happy
  @flow:canon-0-32-relay-approval.F3
  @subject.lifecycle.57
  Scenario: A relayed migrate commit that re-binds Approved lines records its relay in the backup journal
    Given a repo whose migrate plan re-binds an Approved line, a planner role and the plan digest
    When canon migrate --commit --plan-digest <d> runs without the relay option, and then with --relayed-via, --approver, --note and --actor
    Then the first exits 2 with HUMAN_ONLY and rewrites nothing, and the second migrates and the backup journal holds relay with role, actor, tool, approver and answer

  # canon: {"schema":1,"at":"2026-10-30T11:54:30Z","actor":{"agent_id":"impl-relay","role":"implementer","session_id":"s-0-32-relay-impl"}}
  @subject:subject-lifecycle
  @case:edge
  @flow:canon-0-32-relay-approval.F3
  @subject.lifecycle.58
  Scenario: An Approved line without a relay marker reads as before and the marker is outside the digest
    Given a brief with an Approved line written before 0.32 and one with a relay marker
    When both are read for approval, status and the approval time, and the marker is edited by hand
    Then the old line reads as approved as before, with no relay, and the marked line reads as approved and relayed
    And editing the marker does not change the brief's digest, so the approval still binds

  # canon: {"schema":1,"at":"2026-10-30T11:54:33Z","actor":{"agent_id":"impl-relay","role":"implementer","session_id":"s-0-32-relay-impl"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-32-relay-approval.F4
  @subject.lifecycle.59
  Scenario: With approval.relay denied in canon.yaml a relayed approval is refused HUMAN_ONLY naming the key
    Given canon.yaml with approval: { relay: deny } and a planner role
    When canon brief approve, canon policy approve and canon migrate --commit run with --relayed-via and everything else a relay needs
    Then each exits 2 with HUMAN_ONLY naming approval.relay in its message or hint, and the hint gives the call the human runs
    And nothing is written

  # canon: {"schema":1,"at":"2026-10-30T11:54:35Z","actor":{"agent_id":"impl-relay","role":"implementer","session_id":"s-0-32-relay-impl"}}
  @subject:subject-lifecycle
  @case:happy
  @flow:canon-0-32-relay-approval.F4
  @subject.lifecycle.60
  Scenario: Without the key relaying is allowed, and canon init shows the key commented
    Given a repo whose canon.yaml has no approval key, and a fresh canon init
    When a relayed brief approval runs, and the written canon.yaml is read
    Then the approval is recorded, and the init template carries approval: and relay: allow | deny as comments
    And a malformed approval section is refused CONFIG_UNREADABLE rather than read as allow
