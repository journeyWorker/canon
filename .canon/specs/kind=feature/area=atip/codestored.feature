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
  Scenario: A canon.yaml, a policy, a signature, a spec root or a plan document that cannot be read is DENIED, INVALID or NOT_FOUND by its cause, never UNREADABLE
    Given a repository whose canon.yaml does not parse, one whose canon.yaml and policy this user may not read, a rollback signature that is missing, a directory, not UTF-8 text or unreadable, a building change whose spec root directory is unreadable or is a file, a malformed specs section, and a plan document that is unreadable or has no row for the task
    When inventory sync, plugin sync, export, purge, policy approve, strategy rollback, brief check and task done run
    Then a file that does not parse is CONFIG_INVALID (RETENTION_TIERS_INVALID for export and purge), a file the system refuses is CONFIG_DENIED (RETENTION_TIERS_DENIED, POLICY_DENIED, STRATEGY_SIGNATURE_DENIED, PLAN_CORPUS_DENIED), a missing signature is STRATEGY_SIGNATURE_NOT_FOUND and a signature that is a directory or not text STRATEGY_SIGNATURE_INVALID, a spec root directory is SPEC_ROOT_DENIED or SPEC_ROOT_INVALID, a malformed specs section is CONFIG_INVALID with the reason SPECS from brief check and inventory sync alike, a plan document without the row is PLAN_DOCUMENT_INVALID, each exits 2 and writes nothing, and no error is called UNREADABLE

  # canon: {"schema":1,"at":"2026-11-04T06:09:29Z","actor":{"agent_id":"impl-codes-stored","role":"implementer","session_id":"s-0-36-codes-stored"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-stored.F2
  @atip.codestored.03
  Scenario: A broken dispatch manifest is DISPATCH_MANIFEST_CORRUPT at exit 2 and mending it lets the next call through
    Given a repository with a begun dispatch run whose manifest is made invalid JSON, not UTF-8 text, a directory, filed under another run id, unreadable at mode 000 or 200, or sits in a dispatch directory this user may not enter
    When dispatch end runs for the run, and dispatch diff lists the runs
    Then dispatch end exits 2 with DISPATCH_MANIFEST_CORRUPT (DISPATCH_MANIFEST_DENIED for the unreadable one and the unreadable directory, never WRITE_DENIED) and writes nothing, dispatch diff warns under the name DISPATCH_MANIFEST_UNREADABLE and exits 0, and with the manifest mended the same dispatch end succeeds

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
    Then they exit 2 with REPO_INVALID and REPO_DENIED, neither is retryable, nothing is created or written, and the registry holds no LEDGER_CORRUPT because every reader skips a ledger record that does not parse and warns

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
    Given a repository with an object-store tier whose credentials are unset, one whose endpoint does not answer, one whose artifact_store section names no object-store tier, one whose store directory is a symlink, a regular file or unreadable, a local blob that is unreadable or broken, and a commit during which a blob is rewritten after blobs were uploaded or before any
    When artifacts verify, evidence add and artifacts move run
    Then they exit 2 with ARTIFACTS_STORE_REQUIRED (not retryable), ARTIFACTS_STORE_UNAVAILABLE (retryable), CONFIG_INVALID with the reason ARTIFACT_STORE, ARTIFACTS_STORE_INVALID for the symlink and the file, ARTIFACTS_STORE_DENIED for the unreadable directory, ARTIFACTS_BLOB_DENIED and ARTIFACTS_BLOB_CORRUPT, and nothing is written or moved; the rewrite after uploads is STOPPED_MIDWAY at exit 1 (not retryable, reason ARTIFACTS_BLOB_CORRUPT, data the uploads, meta.next the read of the state, nothing removed, the same plan digest stale) and the rewrite before any upload is ARTIFACTS_BLOB_CORRUPT at exit 2

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

  # canon: {"schema":1,"at":"2026-11-04T07:11:03Z","actor":{"agent_id":"impl-refusals-fix","role":"implementer","session_id":"s-0-36-refusals-fix"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-stored.F3
  @atip.codestored.11
  Scenario: A read the system fails in a way a repeat can pass is the subject's UNAVAILABLE code, retryable, at exit 2
    Given a read of canon.yaml, a repository file, a brief, a signature, a scenario file, a policy, a policy approval, a dispatch manifest, a plan document, an adapter response, a hook file, a bound file or a review artifact that the system fails with EIO (or an interrupted or timed-out call), injected through the cause helper of each
    When the command that reads it refuses
    Then the answer is the subject's own UNAVAILABLE code (CONFIG_, REPO_, BRIEF_, STRATEGY_SIGNATURE_, SPEC_ROOT_, POLICY_, POLICY_APPROVAL_, DISPATCH_MANIFEST_, PLAN_CORPUS_, ADAPTER_FILE_, HOOKS_FILE_, EVIDENCE_BINDING_, REVIEW_ARTIFACT_, MIGRATION_) at exit 2 with retryable true, a refused read keeps its DENIED code, a directory in the file's place keeps its INVALID code, and nothing is written

  # canon: {"schema":1,"at":"2026-11-04T07:54:40Z","actor":{"agent_id":"impl-refusals-fix","role":"implementer","session_id":"s-0-36-refusals-fix"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-stored.F1
  @atip.codestored.12
  Scenario: A fault in canon.yaml is the config's own code, and a path in the wrong state is told as what it is
    Given a repository whose canon.yaml lacks the routing entry of a record kind, a directory where canon.yaml belongs, and a change directory this user may not enter
    When change new, subject show, status, brief show and brief check run
    Then the missing routing entry is CONFIG_INVALID with the reason TIERS, the directory is CONFIG_INVALID, and the change directory is BRIEF_DENIED (never BRIEF_NOT_FOUND), each exits 2, is not retryable and writes nothing

  # canon: {"schema":1,"at":"2026-11-04T07:54:45Z","actor":{"agent_id":"impl-refusals-fix","role":"implementer","session_id":"s-0-36-refusals-fix"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-stored.F3
  @atip.codestored.13
  Scenario: A tier a purge or export needs that is not attached is RETENTION_TIERS_REQUIRED and a stored object that does not decode is RETENTION_TIERS_CORRUPT
    Given a repository whose hot tier names a Postgres server with its DSN variable unset, and a trajectory file that is not a Parquet file
    When export and purge run
    Then export exits 2 with RETENTION_TIERS_REQUIRED and purge with RETENTION_TIERS_CORRUPT, neither is retryable, each hint says what to do, and nothing is written; only a backend that does not answer or an error a repeat can pass is RETENTION_TIERS_UNAVAILABLE
