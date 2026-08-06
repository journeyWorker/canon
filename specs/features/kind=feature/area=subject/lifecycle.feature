Feature: subject lifecycle
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:subject-lifecycle
  @subject.lifecycle.01
  Scenario: A new subject is born proposed and reads back through the routed tier
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repo with no subjects
    When one is authored with a domain and a title
    Then querying subjects returns exactly that one
    And its status is proposed, the only state a subject can start in

  @subject:subject-lifecycle
  @subject.lifecycle.02
  Scenario: Adopting a change links it to the subject on both sides
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a change on the ledger and a subject
    When the change is adopted into the subject
    Then the subject lists that change
    And the change names that subject, so neither side has to be inferred from the other

  @subject:subject-lifecycle
  @subject.lifecycle.03
  Scenario: The forward chain advances one rung at a time and every write folds to one row
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a proposed subject
    When it is moved to specced, then building, then verifying
    Then every transition is admitted
    And the four writes read back as one row whose status is verifying
    And a re-write appends a version rather than mutating one, and the reader folds to the latest

  @subject:subject-lifecycle
  @subject.lifecycle.04
  Scenario: A transition off the chain is refused and the subject is unchanged
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a proposed subject
    When it is moved straight to shipped
    Then the command refuses as a usage error naming the transition as invalid
    And the subject still reads proposed

  @subject:subject-lifecycle
  @subject.lifecycle.05
  Scenario: Any state but retired may retire and retired is terminal
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given each of proposed, specced, building, verifying and shipped
    When retiring from that state is considered
    Then it is allowed from every one of them, so retirement is not a rung on the chain
    And retiring from retired is refused, because there is nowhere left to go

  @subject:subject-lifecycle
  @subject.lifecycle.06
  Scenario: Shipping fails closed while a linked scenario carries no verdict at all
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a verifying subject linked to one scenario with no evidence
    When it is moved to shipped
    Then the command fails, reporting the uncovered cell by failure class and naming the scenario
    And the subject still reads verifying, so absent evidence is never read as passing evidence

  @subject:subject-lifecycle
  @subject.lifecycle.07
  Scenario: Shipping is blocked while a linked scenario's latest verdict is divergent
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a verifying subject linked to one scenario whose latest verdict is divergent
    When it is moved to shipped
    Then the command fails and names that scenario
    And the subject still reads verifying

  @subject:subject-lifecycle
  @subject.lifecycle.08
  Scenario: Shipping is admitted once every linked scenario has a faithful latest verdict
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a verifying subject linked to one scenario whose latest verdict is faithful
    When it is moved to shipped
    Then the transition succeeds and the subject reads shipped
    And this one rung of the chain is the only one evidence gates

  @subject:subject-lifecycle
  @subject.lifecycle.09
  Scenario: A domain outside the vocabulary this repo declares is refused naming the legal set
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repo whose activated vocabulary declares its own domain members
    When a subject is authored under a near miss of one of them
    Then the command refuses, naming both the offending value and the members it expected
    And no subject was written, so a typo can never mint a new domain
