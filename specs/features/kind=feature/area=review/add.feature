Feature: review add
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:review-attestation
  @case:happy
  @review.add.01
  Scenario: A review is written attributed to the invoking actor with its provenance ref intact
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a project, a scenario and one upstream ref
    When a review is added
    Then one review record is on the committed ledger
    And it carries the invoking agent and its role, not the reviewer string
    And the upstream ref is stored exactly as given, never rewritten into the other ref variant

  @subject:review-attestation
  @case:failure
  @review.add.02
  Scenario: A review carrying no provenance ref is refused and writes nothing
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given neither an upstream ref nor an original spec ref
    When a review is added
    Then the command refuses as a usage error
    And no review record exists, because an empty or synthesized ref is never written in place of one

  @subject:review-attestation
  @case:failure
  @review.add.03
  Scenario: A review naming both provenance refs is refused as ambiguous
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given both an upstream ref and an original spec ref on one invocation
    When a review is added
    Then the command refuses, naming the two flags as mutually exclusive
    And nothing is written, so exactly one ref is required rather than one being preferred

  @subject:review-attestation
  @case:edge
  @review.add.04
  Scenario: The pinned commit is part of a review's identity
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a review of one scenario in one project at one pin
    When its storage coordinate is resolved
    Then the natural key is the project, the scenario and the pin joined
    And a review of that same scenario at a different pin therefore resolves a different key
    And two attestations at two commits are two records, never two versions of one

  @subject:review-attestation
  @case:edge
  @review.add.05
  Scenario: A review's area comes from its scenario id and never from a plausible directory
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a review whose scenario id disagrees with the directory the feature was found under
    When its storage coordinate is resolved
    Then the area is taken from the scenario id
    And the caller's directory guess decides nothing

  @subject:review-attestation
  @case:failure
  @review.add.06
  Scenario: A review that names no project is malformed rather than project-inferred
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a review body carrying a scenario id and a pin but no project
    When its storage coordinate is resolved
    Then the record is reported malformed
    And no project is inferred for it, so it never becomes a legitimately identified review of a guessed project

  @subject:review-attestation
  @case:failure
  @review.add.07
  Scenario: A review of one project never satisfies another project's evidence for the same scenario id
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a review attesting one scenario id under one project
    And evidence promoted as reviewed for that same scenario id under a different project
    When the trust ladder is checked
    Then that evidence is an unreviewed promotion
    And the attestation is scoped to the project it names, so a shared scenario id borrows no trust

  # canon: {"schema":1,"at":"2026-10-10T16:02:33Z","actor":{"agent_id":"canon"}}
  @subject:review-attestation
  @case:happy
  @review.add.08
  Scenario: A review records the reviewing session and says it was written directly
    Given evidence attested for a scenario
    When a review is added with --session-id
    Then the review's actor carries that session_id
    And the command says it was written directly, with nothing to promote

  # canon: {"schema":1,"at":"2026-10-10T16:02:33Z","actor":{"agent_id":"canon"}}
  @subject:review-attestation
  @case:failure
  @review.add.09
  Scenario: A review from a session that attested the scenario's evidence does not count under distinct_actor
    Given evidence attested from session sess-impl and require_review.distinct_actor on
    When another actor id adds a review from the same session
    Then the subject cannot enter verifying, and unreviewed-promotion names the session and the session rule
    And a review by that reviewer from its own session lets the subject through

  # canon: {"schema":1,"at":"2026-10-10T16:02:33Z","actor":{"agent_id":"canon"}}
  @subject:review-attestation
  @case:edge
  @review.add.10
  Scenario: A session on only one side leaves the actor rule in charge
    Given a session on the evidence or on the review, but not on both
    When the subject moves into require_review scope
    Then a review by another actor counts, as it did before sessions were recorded

  # canon: {"schema":1,"at":"2026-10-10T19:30:45Z","actor":{"agent_id":"canon"}}
  @subject:review-attestation
  @case:happy
  @review.add.20
  Scenario: A review pins the scenario's digest and its latest evidence, computed rather than typed
    Given a scenario with a record and one promoted evidence record
    When a review is added without naming any digest
    Then the review carries the scenario's digest from its record: its own scenario digest, or the file's source digest when the record has none
    And it carries the evidence record's natural key and ledger file digest
    And a scenario with no evidence yet is pinned with an empty evidence list, never left unpinned

  # canon: {"schema":1,"at":"2026-10-10T19:30:45Z","actor":{"agent_id":"canon"}}
  @subject:review-attestation
  @case:failure
  @review.add.21
  Scenario: A review of a scenario with no record is refused and writes nothing
    Given a scenario id that no scenario record carries
    When a review of it is added
    Then the command refuses with exit 2 and names `canon inventory sync`
    And no review record exists, because there is nothing to pin

  # canon: {"schema":1,"at":"2026-10-10T19:30:45Z","actor":{"agent_id":"canon"}}
  @subject:review-attestation
  @case:failure
  @review.add.22
  Scenario: A pinned review turns stale when its scenario's text changes
    Given a pinned review that lets its subject enter verifying with the gate clean
    When the scenario's text changes and `canon inventory sync` records its new digest
    Then `canon gate check` exits 1 with unreviewed-promotion
    And the detail says the review is stale because the spec changed

  # canon: {"schema":1,"at":"2026-10-10T19:30:45Z","actor":{"agent_id":"canon"}}
  @subject:review-attestation
  @case:failure
  @review.add.23
  Scenario: A pinned review turns stale when its scenario is re-attested
    Given a pinned review that lets its subject enter verifying
    When the scenario is attested again and promoted
    Then `canon gate check` exits 1 with unreviewed-promotion saying the review is stale because the evidence changed
    And the move to shipped is refused on the same stale review

  # canon: {"schema":1,"at":"2026-10-10T19:30:45Z","actor":{"agent_id":"canon"}}
  @subject:review-attestation
  @case:failure
  @review.add.24
  Scenario: An indeterminate or unavailable review is a gap, never an approval
    Given a review recorded with --outcome indeterminate or unavailable
    When the gate and the status guard check the scenario
    Then it is unreviewed-promotion with a detail naming the reviewer and the outcome
    And it counts as no approval even without distinct_actor, and as no rejection either

  # canon: {"schema":1,"at":"2026-10-10T19:30:45Z","actor":{"agent_id":"canon"}}
  @subject:review-attestation
  @case:edge
  @review.add.25
  Scenario: The latest review per reviewer wins, so a later indeterminate run withdraws that reviewer's signature
    Given a reviewer who signed a scenario and later recorded an indeterminate review of it
    When the gate folds the reviews to the latest per reviewer
    Then that reviewer's signature no longer counts
    And a later signed review by the same reviewer counts again
    And another reviewer's current signed review still counts

  # canon: {"schema":1,"at":"2026-10-10T19:30:46Z","actor":{"agent_id":"canon"}}
  @subject:review-attestation
  @case:happy
  @review.add.26
  Scenario: A review records the reviewer runtime that produced it
    Given --reviewer-provider, --reviewer-model and --reviewer-harness on review add
    When the review is written
    Then the record carries each value exactly as given
    And an empty or padded label is a usage error

  # canon: {"schema":1,"at":"2026-10-10T19:30:46Z","actor":{"agent_id":"canon"}}
  @subject:review-attestation
  @case:edge
  @review.add.27
  Scenario: canon status counts stale and unsigned reviews and names them in its review step
    Given one scenario whose review went stale and one whose only review is indeterminate
    When `canon status` runs
    Then the subject line counts one stale and one unsigned review, and neither as reviewed
    And the review step names how many scenarios have each kind of gap

  # canon: {"schema":1,"at":"2026-10-10T20:10:17Z","actor":{"agent_id":"canon"}}
  @subject:review-attestation
  @case:edge
  @review.add.28
  Scenario: A review pins its scenario's own digest, so a sibling's edit never stales it
    Given require_review over a .feature file with a Background and two scenarios, each attested and reviewed with its scenario digest pinned
    When a third scenario is appended to the file and synced
    Then the first two reviews stay current and only the new scenario lacks a review
    And editing one scenario's steps stales only that scenario's review, naming the scenario's digest
    And editing the Background stales the review of every scenario that inherits it
    And a review pinned to the file's source digest keeps being compared with the file's source digest
