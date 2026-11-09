Feature: gate ci-run
  # canon: {"schema":1,"at":"2026-11-09T10:22:50Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}

  # canon: {"schema":1,"at":"2026-11-09T10:22:50Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-ci-evidence.F4
  @gate.ci-run.01
  Scenario: With require_ci_run: {} a scenario whose newest evidence is a complete CI record of the current code passes gate check and the ship guard
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:22:59Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F4
  @gate.ci-run.02
  Scenario: A scenario whose newest evidence a person recorded after the CI's is no_ci_run in gate check, the ship guard and status
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:23:18Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F4
  @gate.ci-run.03
  Scenario: A CI record of another code tree is other_code_tree, class stale-evidence, in gate check, the ship guard and status
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:23:45Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F4
  @gate.ci-run.04
  Scenario: A CI record that says its work tree was dirty is dirty_worktree
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:24:09Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F4
  @gate.ci-run.05
  Scenario: min_cases 100 refuses a run of 99 cases as short_run and passes a run of 100
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:25:03Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F4
  @gate.ci-run.06
  Scenario: When git gives no code tree now, a CI record is code_tree_unknown, never passed unknown
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:25:46Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F4
  @gate.ci-run.07
  Scenario: A malformed require_ci_run (min_cases 0, scope [done], an unknown key, null) makes spec_coverage unusable and gate check fails
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:26:07Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F4
  @gate.ci-run.08
  Scenario: --override-reason does not waive a require_ci_run violation of the ship guard
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:27:07Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-ci-evidence.F5
  @gate.ci-run.09
  Scenario: canon context names require_ci_run and canon status reads the policy level as custom
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:28:15Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F5
  @gate.ci-run.10
  Scenario: Without the policy, canon context and canon status have no require_ci_run line
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:29:37Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-ci-evidence.F6
  @gate.ci-run.11
  Scenario: Without the policy, gate check and the ship guard answer as in 0.39, and a record written without --from-ci has no ci key
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:30:19Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F6
  @gate.ci-run.12
  Scenario: A record whose ci is malformed (attempt a string) is unreadable and reported as malformed evidence, while a ci with an unknown key is read
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:31:06Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-ci-evidence.F7
  @gate.ci-run.13
  Scenario: The gate skill carries the GitHub Actions recipe, the policy and subject skills name the key and the check, and the CHANGELOG lists the additions
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:31:33Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F7
  @gate.ci-run.14
  Scenario: A generated block that no longer matches the spec fails the drift test
    Given a step
