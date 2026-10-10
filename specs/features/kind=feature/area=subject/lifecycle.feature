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

  # canon: {"schema":1,"at":"2026-10-10T16:01:46Z","actor":{"agent_id":"canon"}}
  @subject:subject-lifecycle
  @case:edge
  @subject.lifecycle.20
  Scenario: Adopting a change that is already linked writes nothing
    Given a change and a subject that already carry the adoption link on both sides
    When the change is adopted into the subject again
    Then it exits 0 saying the change is already linked and nothing was written
    And no record is added to the ledger

  # canon: {"schema":1,"at":"2026-10-10T16:01:46Z","actor":{"agent_id":"canon"}}
  @subject:subject-lifecycle
  @case:failure
  @subject.lifecycle.21
  Scenario: An adoption that fails between its two writes names what was written and the command that completes it
    Given a change record that cannot be written while the subject record can
    When the change is adopted into the subject
    Then the subject record, which the gate reads adopted changes from, is written first
    And the command exits 2 naming the subject that now lists the change and the exact command that completes the link
    And running that command completes the link, and running it again writes nothing
