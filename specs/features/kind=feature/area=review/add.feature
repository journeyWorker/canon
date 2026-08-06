Feature: review add
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:review-attestation
  @review.add.01
  Scenario: A review is written attributed to the invoking actor with its provenance ref intact
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a project, a scenario and one upstream ref
    When a review is added
    Then one review record is on the committed ledger
    And it carries the invoking agent and its role, not the reviewer string
    And the upstream ref is stored exactly as given, never rewritten into the other ref variant

  @subject:review-attestation
  @review.add.02
  Scenario: A review carrying no provenance ref is refused and writes nothing
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given neither an upstream ref nor an original spec ref
    When a review is added
    Then the command refuses as a usage error
    And no review record exists, because an empty or synthesized ref is never written in place of one

  @subject:review-attestation
  @review.add.03
  Scenario: A review naming both provenance refs is refused as ambiguous
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given both an upstream ref and an original spec ref on one invocation
    When a review is added
    Then the command refuses, naming the two flags as mutually exclusive
    And nothing is written, so exactly one ref is required rather than one being preferred

  @subject:review-attestation
  @review.add.04
  Scenario: The pinned commit is part of a review's identity
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a review of one scenario in one project at one pin
    When its storage coordinate is resolved
    Then the natural key is the project, the scenario and the pin joined
    And a review of that same scenario at a different pin therefore resolves a different key
    And two attestations at two commits are two records, never two versions of one

  @subject:review-attestation
  @review.add.05
  Scenario: A review's area comes from its scenario id and never from a plausible directory
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a review whose scenario id disagrees with the directory the feature was found under
    When its storage coordinate is resolved
    Then the area is taken from the scenario id
    And the caller's directory guess decides nothing

  @subject:review-attestation
  @review.add.06
  Scenario: A review that names no project is malformed rather than project-inferred
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a review body carrying a scenario id and a pin but no project
    When its storage coordinate is resolved
    Then the record is reported malformed
    And no project is inferred for it, so it never becomes a legitimately identified review of a guessed project

  @subject:review-attestation
  @review.add.07
  Scenario: A review of one project never satisfies another project's evidence for the same scenario id
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a review attesting one scenario id under one project
    And evidence promoted as reviewed for that same scenario id under a different project
    When the trust ladder is checked
    Then that evidence is an unreviewed promotion
    And the attestation is scoped to the project it names, so a shared scenario id borrows no trust
