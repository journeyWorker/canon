Feature: divergence lifecycle
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}

  @subject:divergence-lifecycle
  @case:happy
  @divergence.lifecycle.01
  Scenario: Promotion assigns each staged divergence a monotonic order and a refusal consumes none
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given two open candidates staged against two scenarios
    When they are promoted
    Then both are committed carrying the first two run seq values, one each
    And the staging area is empty afterwards

  @subject:divergence-lifecycle
  @case:failure
  @divergence.lifecycle.02
  Scenario: A candidate promote refuses consumes no run seq
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given one staged candidate whose actor carries no role and one well-formed candidate beside it
    When they are promoted
    Then the run reports a refusal without grading it a usage error
    And only the well-formed candidate is committed
    And it holds the first run seq, so the refused one burned no position in the order

  @subject:divergence-lifecycle
  @case:happy
  @divergence.lifecycle.03
  Scenario: Resolving and deferring stage and promote their own record and leave other staged work alone
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given an unrelated candidate a reviewer is still staging
    When one scenario is resolved and another is deferred with a reason and an expiry
    Then both are committed in their own run, each promoted from staging like any other staged record
    And the staged candidate is still sitting there untouched, so a routine resolve never promotes somebody else's work

  @subject:divergence-lifecycle
  @case:happy
  @divergence.lifecycle.04
  Scenario: A resolution is what the divergence list reports
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a scenario resolved
    When canon divergence list is read
    Then the read exits zero, because it is a capability query and never a gate
    And the current state of that scenario is resolved

  @subject:divergence-lifecycle
  @case:edge
  @divergence.lifecycle.05
  Scenario: Run seq alone decides which record is current
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a still-divergent record at a low run seq raised in a high round
    And an open record at a higher run seq raised in a lower round
    When the scenario is folded to its current state
    Then it is open
    And the round the record was raised in moved nothing, because it is not an ordering axis

  @subject:divergence-lifecycle
  @case:edge
  @divergence.lifecycle.06
  Scenario: Round breaks a tie only between records sharing a run seq
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a still-divergent record and an open record at the same run seq
    When the scenario is folded to its current state
    Then the higher round wins and the scenario is open

  @subject:divergence-lifecycle
  @case:edge
  @divergence.lifecycle.07
  Scenario: A deferral lapses back into still-divergent at its expiry
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a divergence deferred with a reason and an expiry
    When it is read before the expiry
    Then it is deferred, carrying that reason and that expiry
    And read after the expiry it is still-divergent, because time alone resolves nothing

  @subject:divergence-lifecycle
  @case:failure
  @divergence.lifecycle.08
  Scenario: A resolution downgrades to resolved-invalid once the app sha moves off what it resolved against
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a divergence whose winning record is resolved against one app sha
    When the scenario's live binding names a different app sha
    Then the folded state is resolved-invalid
    And the record on disk is never rewritten, so the downgrade is derived at read time and not persisted

  @subject:divergence-lifecycle
  @case:happy
  @divergence.lifecycle.09
  Scenario: A resolution whose app sha still matches stays resolved
  # canon: {"schema":1,"at":"2026-08-06T07:00:00.000000Z","actor":{"agent_id":"canon"}}
    Given a divergence whose winning record is resolved against one app sha
    When the scenario's live binding still names that sha
    Then the folded state is resolved
    And the downgrade needs actual evidence of a mismatch, never merely the passage of a read

  # canon: {"schema":1,"at":"2026-10-21T16:35:35Z","actor":{"agent_id":"canon-impl-026"}}
  @subject:divergence-lifecycle
  @case:failure
  @divergence.lifecycle.10
  Scenario: A divergence command whose project id names no configured spec root is refused
    Given a repo whose canon.yaml configures one spec root, game
    When divergence add, resolve, defer and accept each name a --project that is no configured root, with no agent role declared
    Then each exits 2 with UNKNOWN_SPEC_ROOT and stages or commits no divergence record
    And divergence list, which only reads, still lists what the ledger holds

  # canon: {"schema":1,"at":"2026-10-23T12:06:16Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:divergence-lifecycle
  @case:failure
  @divergence.lifecycle.11
  Scenario: An acceptance is a human act and every agent role is refused
    Given an acceptance naming a reason and a tracking reference
    When canon divergence accept runs as planner, implementer, reviewer or orchestrator, declared by --as or by the environment
    Then each run exits 2 with HUMAN_ONLY, retryable false, and a hint addressed to the human naming canon divergence accept
    And the refusal is the same whether the role is declared by --as or by the environment

  # canon: {"schema":1,"at":"2026-10-23T12:06:21Z","actor":{"agent_id":"canon-impl-028","role":"implementer"}}
  @subject:divergence-lifecycle
  @case:edge
  @divergence.lifecycle.12
  Scenario: With no agent role declared the acceptance reaches its own checks
    Given the same acceptance and no agent role declared
    When canon divergence accept runs
    Then it is not refused as HUMAN_ONLY and reaches its own checks instead

  # canon: {"schema":1,"at":"2026-10-26T11:42:45Z","actor":{"agent_id":"impl-030-sb-f4","role":"implementer","session_id":"sb-f4-2026-10-26"}}
  @subject:divergence-lifecycle
  @case:failure
  @flow:canon-0-30-spec-backfill.F4
  @divergence.lifecycle.13
  Scenario: Divergence list refuses an invalid timestamp, a zero limit or a cursor canon did not issue with exit 2
    Given a repo with no divergences
    When canon divergence list runs with --as-of bogus, with --limit 0, with --cursor zzz, and with both --limit and --all
    Then --as-of bogus, --limit 0 and the combined flags each exit 2 with USAGE naming the option, and --cursor zzz exits 2 with CURSOR_INVALID
    And no file in the repo changed

  # canon: {"schema":1,"at":"2026-10-30T11:54:39Z","actor":{"agent_id":"impl-relay","role":"implementer","session_id":"s-0-32-relay-impl"}}
  @subject:divergence-lifecycle
  @case:happy
  @flow:canon-0-32-relay-approval.F3
  @divergence.lifecycle.14
  Scenario: A relayed acceptance records the relay, the approver and the answer on the divergence
    Given a scenario whose latest evidence is divergent, and a reviewer-session-independent agent role with an actor
    When canon divergence accept runs with --relayed-via, --approver, --note and --actor
    Then it exits 0 and the acceptance record holds relay with role, actor, tool, approver and answer, and the same call by a human has no relay

  # canon: {"schema":1,"at":"2026-10-30T11:54:44Z","actor":{"agent_id":"impl-relay","role":"implementer","session_id":"s-0-32-relay-impl"}}
  @subject:divergence-lifecycle
  @case:failure
  @flow:canon-0-32-relay-approval.F3
  @divergence.lifecycle.15
  Scenario: A relayed acceptance without the human's name or answer, or with relay denied, is refused and writes nothing
    Given the same scenario and an agent role
    When canon divergence accept runs relayed without --approver, without --note, and with approval.relay denied in canon.yaml
    Then each exits 2 (USAGE for the missing option, HUMAN_ONLY naming approval.relay for the denial) and no divergence record is written
