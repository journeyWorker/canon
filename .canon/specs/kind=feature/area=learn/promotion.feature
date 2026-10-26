Feature: learn promotion
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:learn-promotion
  @case:happy
  @learn.promotion.01
  Scenario: A proven strategy with a verified human approval graduates into a git-tracked file
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a quarantined distilled strategy and a regime carrying enough corroborating successes to clear its gate
    And a valid paired evaluation of it and a human approval signed by a policy-pinned signer
    When canon strategy promote runs with the evaluation and the approval
    Then the command succeeds and a file appears in the git-tracked strategies tier
    And the strategy has moved from derived memory into something a person can review in a diff
    And the same promotion without the evaluation and the approval is refused and writes nothing

  @subject:learn-promotion
  @case:happy
  @learn.promotion.02
  Scenario: The promoted file opens active and carries the strategy's own content
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a distilled strategy authored under a role
    When it is promoted into a git-tier root
    Then the written path is scoped by that role and named by the strategy id
    And the file opens with front matter whose status is active
    And the body carries the strategy's content, not a summary of it

  @subject:learn-promotion
  @case:happy
  @learn.promotion.03
  Scenario: A dry run previews the promotion and writes nothing
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a strategy that would otherwise pass its gate
    When canon strategy promote is run with --dry-run
    Then the command succeeds and shows what it would write
    And no file is created in the git-tracked tier

  @subject:learn-promotion
  @case:failure
  @learn.promotion.04
  Scenario: A strategy with no corroborating trajectories is blocked by the gate
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a distilled strategy whose regime holds no resolved trajectories at all
    When canon strategy promote is run
    Then the command refuses
    And the strategy stays out of the git-tracked tier, because nothing corroborates it

  @subject:learn-promotion
  @case:failure
  @learn.promotion.05
  Scenario: Unresolved trajectories never count toward a promotion
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a regime carrying more pending trajectories than the gate's minimum
    When canon strategy promote is run on the strategy distilled from them
    Then the command still refuses
    And volume of unresolved evidence is not evidence

  @subject:learn-promotion
  @case:failure
  @learn.promotion.06
  Scenario: A later contradiction resets the streak and blocks the promotion
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a regime with enough successes to clear the gate, followed by one failure
    When canon strategy promote is run
    Then the command refuses
    And the gate counts the streak up to the contradiction, not the total number of successes

  @subject:learn-promotion
  @case:edge
  @learn.promotion.07
  Scenario: A role that configures no promotion gate gets the conservative occurrence default
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a repository whose configuration names no promotion entry for a role
    When that role's promotion gate is resolved
    Then the mode is occurrence
    And the minimum sample count and the window come from the conservative defaults, never left unset

  @subject:learn-promotion
  @case:happy
  @learn.promotion.08
  Scenario: Gate mode is chosen per role between crn and occurrence
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a configuration setting one role to occurrence with its own minimum and window, and another role to crn
    When both roles' gates are resolved
    Then the first role's mode is occurrence and carries exactly the configured minimum and window
    And the second role's mode is crn
    And the crn role's occurrence fields still hold well-formed defaults, because a mode change never leaves a field uninitialized

  @subject:learn-promotion
  @case:happy
  @learn.promotion.09
  Scenario: Demoting soft-flags the promoted file and leaves the rest of it intact
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a strategy already promoted into the git-tracked tier
    When it is demoted under the default policy
    Then its front matter carries a demoted status
    And the front matter it already carried survives unchanged
    And the body survives too, because a demotion records a judgement rather than erasing the strategy

  # canon: {"schema":1,"at":"2026-10-26T11:42:43Z","actor":{"agent_id":"impl-030-sb-f4","role":"implementer","session_id":"sb-f4-2026-10-26"}}
  @subject:learn-promotion
  @case:happy
  @flow:canon-0-30-spec-backfill.F4
  @learn.promotion.10
  Scenario: The approval payload of a seeded strategy is the same bytes on every call, carrying the approver, time and subject
    Given a quarantined strategy candidate and a passing paired evaluation file
    When canon strategy approval-payload runs twice with the same approver and approval time, and once more with --json
    Then every run exits 0 and the two plain runs print identical bytes
    And the --json data carries the payload as hex equal to those bytes, the approver, the approval time and the subject strategy:<id>
    And no file in the repo changed

  # canon: {"schema":1,"at":"2026-10-26T11:42:44Z","actor":{"agent_id":"impl-030-sb-f4","role":"implementer","session_id":"sb-f4-2026-10-26"}}
  @subject:learn-promotion
  @case:failure
  @flow:canon-0-30-spec-backfill.F4
  @learn.promotion.11
  Scenario: The approval payload of an unknown strategy or a malformed id is refused and writes nothing
    Given a repo with no strategies and an evaluation file
    When canon strategy approval-payload runs for a well-formed id no strategy has, for the malformed id nope, and for a well-formed id without --evaluation
    Then the unknown strategy exits 2 with LEARN_STRATEGY_NOT_FOUND, and the malformed id and the missing --evaluation each exit 2 with USAGE
    And no file in the repo changed

  # canon: {"schema":1,"at":"2026-10-26T11:42:44Z","actor":{"agent_id":"impl-030-sb-f4","role":"implementer","session_id":"sb-f4-2026-10-26"}}
  @subject:learn-promotion
  @case:happy
  @flow:canon-0-30-spec-backfill.F4
  @learn.promotion.12
  Scenario: A rollback of a promoted strategy signed by the pinned approver disables it and prints the rollback record
    Given a strategy promoted with a paired evaluation and an approval signed by the pinned approver alice, and a rollback signature over the strategy, the reason and the time
    When canon strategy rollback runs with that reason, --actor-id alice, the signature file and the approval time
    Then it exits 0 and prints the record: the strategy id, the actor alice, the reason, an approval digest and the rollback time
    And the promoted file's status changes from active to rolled_back and gains the rollback reason and actor
    And the rest of its front matter and its body survive unchanged

  # canon: {"schema":1,"at":"2026-10-26T11:42:44Z","actor":{"agent_id":"impl-030-sb-f4","role":"implementer","session_id":"sb-f4-2026-10-26"}}
  @subject:learn-promotion
  @case:failure
  @flow:canon-0-30-spec-backfill.F4
  @learn.promotion.13
  Scenario: A rollback with an unreadable or invalid signature or a malformed timestamp is refused and writes nothing
    Given a repo with a strategy candidate, an approver pinned in the policy, and a signature file whose content is not a signature
    When canon strategy rollback runs for the candidate with a signature file that does not exist, with the file that is not a signature, with --approval-at yesterday, and with no --reason
    Then the missing signature file exits 2 with LEARN_SIGNATURE_UNREADABLE
    And the file that is not a signature exits 1 with LEARN_ROLLBACK_FAILED, its message saying the ssh signature verification failed
    And the malformed timestamp and the missing reason each exit 2 with USAGE
    And no file in the repo changed
