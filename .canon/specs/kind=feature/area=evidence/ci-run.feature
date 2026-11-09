Feature: evidence ci-run
  # canon: {"schema":1,"at":"2026-11-09T10:19:54Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}

  # canon: {"schema":1,"at":"2026-11-09T10:19:54Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-ci-evidence.F1
  @evidence.ci-run.01
  Scenario: A batch recorded with --from-ci gives every record the same ci block: provider, run, attempt, url, repository, sha and each report's path, digest, format and case counts
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:20:39Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-ci-evidence.F1
  @evidence.ci-run.02
  Scenario: A report no scenario owns a case of is still an entry of ci.reports, and every case counts, with or without a scenario id
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:20:52Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-ci-evidence.F1
  @evidence.ci-run.03
  Scenario: The same batch recorded again with the same GITHUB_RUN_ATTEMPT is unchanged, and another attempt writes new records
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:21:02Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-ci-evidence.F1
  @evidence.ci-run.04
  Scenario: A review of a scenario stays current when CI records the same suite again
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:21:14Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F1
  @evidence.ci-run.05
  Scenario: --from-ci without --all-scenarios, or without an explicit --ref, is refused (exit 2) and nothing is written
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:21:24Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F2
  @evidence.ci-run.06
  Scenario: --from-ci outside GitHub Actions (GITHUB_ACTIONS not true, or an empty GITHUB_RUN_ID) is refused as CI_ENV_REQUIRED naming the variable, and nothing is written
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:21:40Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F2
  @evidence.ci-run.07
  Scenario: --from-ci on a work tree that differs from HEAD (an untracked junit.xml) is refused as CI_WORKTREE_CONFLICT, and nothing is written
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:21:59Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-ci-evidence.F2
  @evidence.ci-run.08
  Scenario: A report in a gitignored directory passes --from-ci, and the same batch without --from-ci is recorded on a dirty work tree as in 0.39
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:22:14Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F2
  @evidence.ci-run.09
  Scenario: --from-ci with --rev or --verdict is refused as EVIDENCE_ARGUMENT_CONFLICT and nothing is written
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:22:26Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-ci-evidence.F3
  @evidence.ci-run.10
  Scenario: Every --report batch prints one line per report with its passed, failed and skipped counts, and data.reports[] says the same, with or without --from-ci
    Given a step

  # canon: {"schema":1,"at":"2026-11-09T10:22:37Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F3
  @evidence.ci-run.11
  Scenario: A report that cannot be parsed is refused as before, without counts, and nothing is written
    Given a step
