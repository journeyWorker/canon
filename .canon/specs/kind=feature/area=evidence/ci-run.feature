Feature: evidence ci-run
  # canon: {"schema":1,"at":"2026-11-09T10:19:54Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}

  # canon: {"schema":1,"at":"2026-11-09T10:19:54Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-ci-evidence.F1
  @evidence.ci-run.01
  Scenario: A batch recorded with --from-ci gives every record the same ci block: provider, run, attempt, url, repository, sha and each report's path, digest, format and case counts
    Given a repository with a report whose cases belong to several scenarios and the GitHub Actions environment variables
    When evidence add --all-scenarios --from-ci --ref <suite command> --report junit:<path> runs
    Then every record written has the same ci block, and its provider, run, attempt, url, repository, sha and each report's path, sha256, format, passed, failed and skipped equal the environment and the report

  # canon: {"schema":1,"at":"2026-11-09T10:20:39Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-ci-evidence.F1
  @evidence.ci-run.02
  Scenario: A report no scenario owns a case of is still an entry of ci.reports, and every case counts, with or without a scenario id
    Given two reports, one with a case of a scenario and one with no case any scenario owns, and some cases with no scenario id
    When the batch is recorded with --from-ci
    Then both reports are entries of ci.reports, and the counts include every case, with or without a scenario id

  # canon: {"schema":1,"at":"2026-11-09T10:20:52Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-ci-evidence.F1
  @evidence.ci-run.03
  Scenario: The same batch recorded again with the same GITHUB_RUN_ATTEMPT is unchanged, and another attempt writes new records
    Given a batch already recorded with --from-ci at one GITHUB_RUN_ATTEMPT
    When the same batch is recorded again with the same attempt, then with another attempt
    Then the same attempt reports unchanged and writes nothing, and the other attempt writes new records

  # canon: {"schema":1,"at":"2026-11-09T10:21:02Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-ci-evidence.F1
  @evidence.ci-run.04
  Scenario: A review of a scenario stays current when CI records the same suite again
    Given a scenario with a review that holds, and a CI record of its suite
    When CI records the same suite again with --from-ci
    Then the review is still current for the scenario

  # canon: {"schema":1,"at":"2026-11-09T10:21:14Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F1
  @evidence.ci-run.05
  Scenario: --from-ci without --all-scenarios, or without an explicit --ref, is refused (exit 2) and nothing is written
    Given a repository and the CI environment
    When evidence add --from-ci runs without --all-scenarios, and again with --all-scenarios but without an explicit --ref
    Then each call exits 2 and writes no record

  # canon: {"schema":1,"at":"2026-11-09T10:21:24Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F2
  @evidence.ci-run.06
  Scenario: --from-ci outside GitHub Actions (GITHUB_ACTIONS not true, or an empty GITHUB_RUN_ID) is refused as CI_ENV_REQUIRED naming the variable, and nothing is written
    Given a repository where GITHUB_ACTIONS is not true, or GITHUB_RUN_ID is empty
    When evidence add --all-scenarios --from-ci runs
    Then it exits 2 with CI_ENV_REQUIRED naming the variable, and nothing is written

  # canon: {"schema":1,"at":"2026-11-09T10:21:40Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F2
  @evidence.ci-run.07
  Scenario: --from-ci on a work tree that differs from HEAD (an untracked junit.xml) is refused as CI_WORKTREE_CONFLICT, and nothing is written
    Given a work tree with an untracked junit.xml, so its code differs from HEAD's
    When evidence add --all-scenarios --from-ci runs with the CI environment
    Then it exits 2 with CI_WORKTREE_CONFLICT and a hint to use a gitignored path, .canon/ or code.exclude, and nothing is written

  # canon: {"schema":1,"at":"2026-11-09T10:21:59Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-ci-evidence.F2
  @evidence.ci-run.08
  Scenario: A report in a gitignored directory passes --from-ci, and the same batch without --from-ci is recorded on a dirty work tree as in 0.39
    Given a dirty work tree, and a report in a gitignored directory
    When the batch is recorded with --from-ci, and the same batch is recorded without it on the dirty tree
    Then the first passes, and the second is recorded as in 0.39

  # canon: {"schema":1,"at":"2026-11-09T10:22:14Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F2
  @evidence.ci-run.09
  Scenario: --from-ci with --rev or --verdict is refused as EVIDENCE_ARGUMENT_CONFLICT and nothing is written
    Given a repository and the CI environment
    When evidence add --all-scenarios --from-ci runs with --rev <commit>, and again with --verdict faithful
    Then each call exits 2 with EVIDENCE_ARGUMENT_CONFLICT and writes no record

  # canon: {"schema":1,"at":"2026-11-09T10:22:26Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:happy
  @flow:canon-0-40-ci-evidence.F3
  @evidence.ci-run.10
  Scenario: Every --report batch prints one line per report with its passed, failed and skipped counts, and data.reports[] says the same, with or without --from-ci
    Given a batch of two reports
    When evidence add --all-scenarios --report runs, with and without --from-ci
    Then one line per report says <path>: <passed> passed, <failed> failed, <skipped> skipped, and data.reports[] says the same numbers

  # canon: {"schema":1,"at":"2026-11-09T10:22:37Z","actor":{"agent_id":"spec-ci-evidence","role":"planner"}}
  @subject:evidence-attestation
  @lane:behavior
  @case:failure
  @flow:canon-0-40-ci-evidence.F3
  @evidence.ci-run.11
  Scenario: A report that cannot be parsed is refused as before, without counts, and nothing is written
    Given a batch with a report that cannot be parsed
    When evidence add --all-scenarios --report runs
    Then it is refused as before, no count is told, and nothing is written
