Feature: gate staleness-scope
  # canon: {"schema":1,"at":"2026-11-08T06:19:28Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}

  # canon: {"schema":1,"at":"2026-11-08T06:19:28Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-39-staleness-scope.F1
  @gate.staleness-scope.01
  Scenario: A directory and a glob declared as the surface go stale when a file under them changes
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:19:57Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:failure
  @flow:canon-0-39-staleness-scope.F1
  @gate.staleness-scope.02
  Scenario: A glob that stops at a slash and a name at the root are not stale for a file further down
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:20:58Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-39-staleness-scope.F1
  @gate.staleness-scope.03
  Scenario: code.exclude, code scopes and surface refs read a pattern with a slash the same way
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:22:12Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-39-staleness-scope.F2
  @gate.staleness-scope.04
  Scenario: A surface that matches no file is saved with SURFACE_REF_UNMATCHED and exits 0
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:22:48Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:failure
  @flow:canon-0-39-staleness-scope.F2
  @gate.staleness-scope.05
  Scenario: A surface outside the repository is still refused as USAGE
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:23:34Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-39-staleness-scope.F3
  @gate.staleness-scope.06
  Scenario: A shallow clone is counted as base_missing with the unshallow instruction, and the verdict is unchanged
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:23:59Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:failure
  @flow:canon-0-39-staleness-scope.F3
  @gate.staleness-scope.07
  Scenario: A full clone has every base and gate check says nothing about unread cells
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:24:49Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-39-staleness-scope.F3
  @gate.staleness-scope.08
  Scenario: A base commit removed by gc is base_missing and there is no unshallow instruction
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:25:57Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-39-staleness-scope.F3
  @gate.staleness-scope.09
  Scenario: A ledger moved from another branch is base_not_ancestor and judged from the range
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:27:41Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-39-staleness-scope.F3
  @gate.staleness-scope.10
  Scenario: With no git on the PATH the cells are git_unavailable
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:27:49Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-39-staleness-scope.F4
  @gate.staleness-scope.11
  Scenario: status and gate check say the same count and reasons for the cells they could not read
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:27:55Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:failure
  @flow:canon-0-39-staleness-scope.F4
  @gate.staleness-scope.12
  Scenario: When every base is read, neither gate check nor status says anything
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:28:02Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:happy
  @flow:canon-0-39-staleness-scope.F5
  @gate.staleness-scope.13
  Scenario: The CHANGELOG says the new rule, the advisory and the warnings
    Given a step

  # canon: {"schema":1,"at":"2026-11-08T06:28:10Z","actor":{"agent_id":"spec-gitstale","role":"planner"}}
  @subject:gate-trust-spine
  @lane:behavior
  @case:failure
  @flow:canon-0-39-staleness-scope.F5
  @gate.staleness-scope.14
  Scenario: A CHANGELOG without the entry or a piece of it is named by what it lacks
    Given a step
