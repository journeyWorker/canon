Feature: atip codestored
  # canon: {"schema":1,"at":"2026-11-04T06:09:18Z","actor":{"agent_id":"impl-codes-stored","role":"implementer","session_id":"s-0-36-codes-stored"}}

  # canon: {"schema":1,"at":"2026-11-04T06:09:18Z","actor":{"agent_id":"impl-codes-stored","role":"implementer","session_id":"s-0-36-codes-stored"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-stored.F1
  @atip.codestored.01
  Scenario: A brief that cannot be read is BRIEF_DENIED, BRIEF_INVALID or BRIEF_NOT_FOUND by its cause, and a directory in its place is INVALID
    Given a repository with a change whose brief is then made unreadable, filled with bytes that are not UTF-8, replaced by a directory, and a change name that has no brief
    When brief show and brief check run on each
    Then they exit 2 with BRIEF_DENIED, BRIEF_INVALID, BRIEF_INVALID and BRIEF_NOT_FOUND, none is retryable, nothing is written, and once the brief is back the same call passes (a test run as root skips the denied case and says so)

  # canon: {"schema":1,"at":"2026-11-04T06:09:25Z","actor":{"agent_id":"impl-codes-stored","role":"implementer","session_id":"s-0-36-codes-stored"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-stored.F1
  @atip.codestored.02
  Scenario: A canon.yaml, a policy, a signature or a spec file that cannot be read is DENIED, INVALID or NOT_FOUND by its cause, never UNREADABLE
    Given a repository whose canon.yaml does not parse, one whose canon.yaml and policy this user may not read
    When inventory sync, plugin sync, export, purge and policy approve run
    Then a file that does not parse is CONFIG_INVALID (RETENTION_TIERS_INVALID for export and purge), a file the system refuses is CONFIG_DENIED (RETENTION_TIERS_DENIED, POLICY_DENIED), a signature or spec file is told the same way by its cause, each exits 2 and writes nothing, and no error is called UNREADABLE

  # canon: {"schema":1,"at":"2026-11-04T06:09:29Z","actor":{"agent_id":"impl-codes-stored","role":"implementer","session_id":"s-0-36-codes-stored"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-stored.F2
  @atip.codestored.03
  Scenario: A broken dispatch manifest is DISPATCH_MANIFEST_CORRUPT at exit 2 and mending it lets the next call through
    Given a repository with a begun dispatch run whose manifest is made invalid JSON, filed under another run id, and unreadable
    When dispatch end runs for the run, and dispatch diff lists the runs
    Then dispatch end exits 2 with DISPATCH_MANIFEST_CORRUPT (DISPATCH_MANIFEST_DENIED for the unreadable one) and writes nothing, dispatch diff warns under the name DISPATCH_MANIFEST_UNREADABLE and exits 0, and with the manifest mended the same dispatch end succeeds

  # canon: {"schema":1,"at":"2026-11-04T06:09:33Z","actor":{"agent_id":"impl-codes-stored","role":"implementer","session_id":"s-0-36-codes-stored"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-stored.F2
  @atip.codestored.04
  Scenario: A broken policy approval refuses the ship with POLICY_APPROVAL_CORRUPT at exit 2 and no override passes it
    Given a repository with an approved policy whose approval file is then made invalid JSON, and made unreadable
    When subject move to shipped runs with and without --override-reason, and gate check runs
    Then the ship exits 2 with POLICY_APPROVAL_CORRUPT (POLICY_APPROVAL_DENIED for the unreadable file) and the override passes neither, gate check keeps its own exit 1, and the subject stays verifying

  # canon: {"schema":1,"at":"2026-11-04T06:09:37Z","actor":{"agent_id":"impl-codes-stored","role":"implementer","session_id":"s-0-36-codes-stored"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-stored.F2
  @atip.codestored.05
  Scenario: A repository that cannot be read is REPO_INVALID or REPO_DENIED and writes nothing
    Given a repository, a --repo path that does not exist, and a ledger directory this user may not read
    When dashboard and subject show run
    Then they exit 2 with REPO_INVALID and REPO_DENIED, neither is retryable, nothing is created or written, and a ledger record that does not parse is LEDGER_CORRUPT (proved by the classification test)

  # canon: {"schema":1,"at":"2026-11-04T06:09:42Z","actor":{"agent_id":"impl-codes-stored","role":"implementer","session_id":"s-0-36-codes-stored"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-stored.F3
  @atip.codestored.06
  Scenario: A write that waits in vain for the repository write lock is REPO_LOCK_UNAVAILABLE, retryable, and the same call passes once the lock is free
    Given a repository whose repository write lock is held by another process
    When a write command runs and waits to the end of its wait
    Then it exits 2 with REPO_LOCK_UNAVAILABLE and retryable true, nothing is written, and when the lock is released the same call writes

  # canon: {"schema":1,"at":"2026-11-04T06:09:46Z","actor":{"agent_id":"impl-codes-stored","role":"implementer","session_id":"s-0-36-codes-stored"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-stored.F3
  @atip.codestored.07
  Scenario: An artifact store that cannot be used is told by its cause: REQUIRED, UNAVAILABLE, INVALID or CONFIG_INVALID, and a blob that cannot be read is not CORRUPT
    Given a repository with an object-store tier whose credentials are unset, one whose endpoint does not answer, one whose artifact_store section names no object-store tier, one whose store directory is a symlink, and a local blob that is unreadable or broken
    When artifacts verify, evidence add and artifacts move run
    Then they exit 2 with ARTIFACTS_STORE_REQUIRED (not retryable), ARTIFACTS_STORE_UNAVAILABLE (retryable), CONFIG_INVALID with the reason ARTIFACT_STORE, ARTIFACTS_STORE_INVALID, ARTIFACTS_BLOB_DENIED and ARTIFACTS_BLOB_CORRUPT, and nothing is written or moved

  # canon: {"schema":1,"at":"2026-11-04T06:09:50Z","actor":{"agent_id":"impl-codes-stored","role":"implementer","session_id":"s-0-36-codes-stored"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-stored.F4
  @atip.codestored.08
  Scenario: A target that cannot be written is WRITE_DENIED or WRITE_INVALID and a ledger root without an archive is LEDGER_ARCHIVE_INVALID
    Given a repository, an output path whose directory is missing or is a file, a read-only output directory, and a ledger root that is not named ledger
    When export and ledger compact run
    Then export exits 2 with WRITE_INVALID and WRITE_DENIED, ledger compact with LEDGER_ARCHIVE_INVALID, none is retryable and nothing is changed

  # canon: {"schema":1,"at":"2026-11-04T06:09:54Z","actor":{"agent_id":"impl-codes-stored","role":"implementer","session_id":"s-0-36-codes-stored"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-stored.F5
  @atip.codestored.09
  Scenario: The old names are gone from the registry and the new codes carry the exit and retryable value of the table
    Given the code registry
    When the registry and canon schema --error are read
    Then none of the old names is an error code and canon schema --error refuses each, the warnings keep their names, every new code has the exit and retryable value of the table, and only an UNAVAILABLE code is retryable

  # canon: {"schema":1,"at":"2026-11-04T06:09:58Z","actor":{"agent_id":"impl-codes-stored","role":"implementer","session_id":"s-0-36-codes-stored"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-stored.F5
  @atip.codestored.10
  Scenario: The CHANGELOG replaces every old name with its new codes
    Given the CHANGELOG of the release
    When its replacement table is read
    Then every old name has a row whose second column names its new codes
