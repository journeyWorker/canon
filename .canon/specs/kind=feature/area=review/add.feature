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
  Scenario: A review pins the scenario's digest and what its latest evidence says, computed rather than typed
    Given a scenario with a record and one promoted evidence record
    When a review is added without naming any digest
    Then the review carries the scenario's digest from its record: its own scenario digest, or the file's source digest when the record has none
    And it carries, per role, the latest evidence's verdict and the identity of the checks it bound, never the record's own id
    And a scenario with no evidence yet is pinned with an empty list of checks, never left unpinned

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
  Scenario: A pinned review turns stale when its scenario's evidence says something different
    Given a pinned review that lets its subject enter verifying
    When the scenario is attested again with a different verdict, or from a different set of cases, and promoted
    Then `canon gate check` exits 1 with unreviewed-promotion saying the review is stale because the evidence changed
    And the move to shipped is refused on the same stale review
    And attesting again with the same verdict from the same cases leaves the review current

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
    And re-aligning a table, reordering tags or moving a whole doc string never stales a review, while a changed cell, tag or doc string line does

  # canon: {"schema":1,"at":"2026-10-11T14:44:56Z","actor":{"agent_id":"canon"}}
  @subject:review-attestation
  @case:happy
  @review.add.40
  Scenario: A review stores the files the reviewer looked at and records their path and digest
    Given a screenshot and a recording inside the repository
    When a review is added with --artifact for each, one of them given twice
    Then each file's bytes are stored in .canon/artifacts/sha256/ under their digest and review add says so
    And the review records each file once as its path and sha256, in the order given
    And a review without --artifact carries no artifacts field
    And `canon gate check` stays clean after the working-tree files change

  # canon: {"schema":1,"at":"2026-10-11T14:44:56Z","actor":{"agent_id":"canon"}}
  @subject:review-attestation
  @case:failure
  @review.add.41
  Scenario: A missing or altered review artifact is stale evidence naming the review
    Given a review bound to a screenshot and a recording
    When one stored blob is deleted and its working-tree file changed, and the other stored blob is altered
    Then `canon gate check` exits 1 with stale-evidence on the review's scenario for each file
    And the detail names the reviewer, the review's actor and time, the path, the recorded digest and what the store and the working tree hold
    And an altered blob is stale even while the working-tree file still matches

  # canon: {"schema":1,"at":"2026-10-11T14:44:56Z","actor":{"agent_id":"canon"}}
  @subject:review-attestation
  @case:failure
  @review.add.42
  Scenario: A review artifact outside the repository, missing, or over the size limit is refused and nothing is written
    Given a file outside the repository, a path that names no file, and a file over --max-artifact-mib
    When a review is added with each as --artifact
    Then review add exits 2 with a refusal naming the file and why
    And no review is written and no bytes are stored

  # canon: {"schema":1,"at":"2026-10-11T14:44:56Z","actor":{"agent_id":"canon"}}
  @subject:review-attestation
  @case:edge
  @review.add.43
  Scenario: Only the in-effect review's artifacts are checked
    Given a review whose stored artifact blob was lost
    When the same reviewer records a later review of the scenario
    Then the earlier review's artifacts are no longer checked and `canon gate check` is clean

  # canon: {"schema":1,"at":"2026-10-12T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:review-attestation
  @case:happy
  @review.add.60
  Scenario: A new review of a scenario whose review from before pinning went stale clears it
    Given a review written before 0.15 that is stale because its scenario changed after the commit it pinned
    When the same reviewer reviews the scenario again with `canon review add`
    Then the new review pins what it saw and supersedes the old one
    And `canon gate check` exits 0

  # canon: {"schema":1,"at":"2026-10-12T00:00:00Z","actor":{"agent_id":"canon"}}
  @subject:review-attestation
  @case:failure
  @review.add.61
  Scenario: The subject status guard and status judge a review that predates 0.23 as the gate does
    Given a building subject whose scenario's review predates 0.23 pinning
    When the subject is moved to verifying
    Then the guard refuses with the gate's detail naming `canon migrate` and prints the advisory for the subject's own scenarios
    And `canon status` counts the scenario as a stale review
    And once the scenario is reviewed again the move goes through

  # canon: {"schema":1,"at":"2026-10-12T15:00:00Z","actor":{"agent_id":"018-n1-impl"}}
  @subject:review-attestation
  @case:happy
  @review.add.80
  Scenario: One review add signs off several scenarios with one reviewer, pin and set of artifacts
    Given three indexed scenarios
    When a review is added with --scenario-id repeated and --scenario-ids-from a file of ids with comments
    Then one review is written per scenario, each pinning its own scenario and evidence, with the same reviewer, pin and artifacts
    And the artifacts are stored once

  # canon: {"schema":1,"at":"2026-10-12T15:00:00Z","actor":{"agent_id":"018-n1-impl"}}
  @subject:review-attestation
  @case:failure
  @review.add.81
  Scenario: A batch review with any scenario that has no record writes nothing
    Given two indexed scenarios and one id with no scenario record
    When a review is added for all three
    Then the add exits 2 naming the unindexed id, and no review is written for any of them
    And an ids file with a line that is not a scenario id is refused with exit 2, naming the line
    And when writing the second review fails, the first review and every blob the call stored are removed, and a read-only review directory is refused before anything is written

  # canon: {"schema":1,"at":"2026-10-12T18:00:00Z","actor":{"agent_id":"018-n3-impl"}}
  @subject:review-attestation
  @case:edge
  @review.add.90
  Scenario: A lost review blob whose working-tree file still matches is a vault advisory, as for evidence
    Given a review bound to a screenshot whose stored blob was deleted while the working-tree file is unchanged
    When `canon gate check` runs
    Then it exits 0 with no stale-evidence, and the evidence-artifacts advisory names the scenario, the path, the digest and the reviewer and says to run `canon evidence vault`
    And `canon evidence vault` stores the blob back from the working tree, and the advisory goes away
    And once the blob is lost again and the working-tree file changes, the gate exits 1 with stale-evidence naming the review

  # canon: {"schema":1,"at":"2026-10-12T18:00:00Z","actor":{"agent_id":"018-n3-impl"}}
  @subject:review-attestation
  @case:edge
  @review.add.91
  Scenario: A review that went stale and was reviewed again by another reviewer is superseded, so its files are neither checked nor vaulted
    Given a review bound to a screenshot, then new evidence for its scenario, then another reviewer's review bound to a recording
    When the first review's stored blob is lost while its working-tree file still matches
    Then `canon gate check` exits 0 with no advisory for the file, and `canon evidence vault` stores nothing for it, counting only the newer review's file
    And once its working-tree file changes too, `canon gate check` still exits 0 and names neither the file nor the first review
    And when the newer review's blob is lost and its working-tree file changes, the gate exits 1 with stale-evidence naming the newer review only

  # canon: {"schema":1,"at":"2026-10-21T16:35:27Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:review-attestation
  @case:failure
  @review.add.92
  Scenario: A review for a project id that names no configured spec root is refused
    Given a repo whose canon.yaml configures one spec root, game, and a scenario indexed under it
    When review add names a --project-id that is no configured root
    Then it exits 2 with UNKNOWN_SPEC_ROOT, writes no review record, and the hint names the configured root
