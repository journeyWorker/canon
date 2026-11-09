Feature: gate ci-run
  # canon: {"schema":1,"at":"2026-11-09T10:22:50Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}

  # canon: {"schema":1,"at":"2026-11-09T10:22:50Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-ci-evidence.F4
  @gate.ci-run.01
  Scenario: With require_ci_run: {} a scenario whose newest evidence is a complete CI record of the current code passes gate check and the ship guard
    Given require_ci_run: {} and a scenario whose newest evidence is a complete CI record of the current code tree
    When gate check runs and the subject is moved to shipped
    Then neither reports a violation

  # canon: {"schema":1,"at":"2026-11-09T10:22:59Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F4
  @gate.ci-run.02
  Scenario: A scenario whose newest evidence a person recorded after the CI's is no_ci_run in gate check, the ship guard and status
    Given require_ci_run: {} and a scenario whose newest evidence a person recorded after the CI record
    When gate check, the ship guard and status read it
    Then all three name no_ci_run for the scenario

  # canon: {"schema":1,"at":"2026-11-09T10:23:18Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F4
  @gate.ci-run.03
  Scenario: A CI record of another code tree is other_code_tree, class stale-evidence, in gate check, the ship guard and status
    Given require_ci_run: {} and a CI record of another code tree
    When gate check, the ship guard and status read it
    Then all three name other_code_tree, and the class is stale-evidence

  # canon: {"schema":1,"at":"2026-11-09T10:23:45Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F4
  @gate.ci-run.04
  Scenario: A CI record that says its work tree was dirty is dirty_worktree
    Given require_ci_run: {} and a CI record whose worktree_dirty is true
    When gate check, the ship guard and status read it
    Then all three name dirty_worktree

  # canon: {"schema":1,"at":"2026-11-09T10:24:09Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F4
  @gate.ci-run.05
  Scenario: min_cases 100 refuses a run of 99 cases as short_run and passes a run of 100
    Given require_ci_run with min_cases 100 and CI records of 99 and of 100 cases run
    When gate check runs on each
    Then 99 is short_run naming 99 and 100, and 100 passes

  # canon: {"schema":1,"at":"2026-11-09T10:25:03Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F4
  @gate.ci-run.06
  Scenario: When git gives no code tree now, a CI record is code_tree_unknown, never passed unknown
    Given require_ci_run: {} and a repository where git gives no code tree
    When gate check, the ship guard and status read a CI record
    Then all three name code_tree_unknown, and none passes the record as unknown

  # canon: {"schema":1,"at":"2026-11-09T10:25:46Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F4
  @gate.ci-run.07
  Scenario: A malformed require_ci_run (min_cases 0, scope [done], an unknown key, null) makes spec_coverage unusable and gate check fails
    Given a policy whose require_ci_run has min_cases 0, scope [done], an unknown key, or null
    When gate check runs on each
    Then spec_coverage is unusable and gate check reports a violation

  # canon: {"schema":1,"at":"2026-11-09T10:26:07Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F4
  @gate.ci-run.08
  Scenario: --override-reason does not waive a require_ci_run violation of the ship guard
    Given require_ci_run: {} and a violation at the ship guard
    When subject move --to shipped runs with --override-reason
    Then the guard still refuses with the violation

  # canon: {"schema":1,"at":"2026-11-09T10:27:07Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-ci-evidence.F5
  @gate.ci-run.09
  Scenario: canon context names require_ci_run and canon status reads the policy level as custom
    Given a policy with require_ci_run
    When canon context and canon status run
    Then context names require_ci_run in one line and status reads the policy level as custom

  # canon: {"schema":1,"at":"2026-11-09T10:28:15Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F5
  @gate.ci-run.10
  Scenario: Without the policy, canon context and canon status have no require_ci_run line
    Given a policy without require_ci_run
    When canon context and canon status run
    Then neither has a require_ci_run line

  # canon: {"schema":1,"at":"2026-11-09T10:29:37Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-ci-evidence.F6
  @gate.ci-run.11
  Scenario: Without the policy, gate check and the ship guard answer as in 0.39, and a record written without --from-ci has no ci key
    Given a policy without require_ci_run and a record written without --from-ci
    When gate check and the ship guard run and the record is read
    Then their answers are those of 0.39, and the record has no ci key

  # canon: {"schema":1,"at":"2026-11-09T10:30:19Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F6
  @gate.ci-run.12
  Scenario: A record whose ci is malformed (attempt a string) is unreadable and reported as malformed evidence, while a ci with an unknown key is read
    Given a record whose ci has an attempt that is a string, and one whose ci has an unknown key
    When the ledger is read
    Then the first is unreadable and reported as malformed evidence, and the second is read

  # canon: {"schema":1,"at":"2026-11-09T10:31:06Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-ci-evidence.F7
  @gate.ci-run.13
  Scenario: The gate skill carries the GitHub Actions recipe, the policy and subject skills name the key and the check, and the CHANGELOG lists the additions
    Given the skills, the policy and subject guides and the CHANGELOG
    When they are read
    Then the gate skill has the GitHub Actions recipe, the policy and subject skills name the key and the check, and the Unreleased section lists the additions

  # canon: {"schema":1,"at":"2026-11-09T10:31:33Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F7
  @gate.ci-run.14
  Scenario: A generated block that no longer matches the spec fails the drift test
    Given a generated block that no longer matches its spec
    When the drift test runs
    Then it fails
