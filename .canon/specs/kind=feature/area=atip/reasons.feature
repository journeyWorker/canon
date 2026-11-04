Feature: atip reasons
  # canon: {"schema":1,"at":"2026-11-04T11:58:07Z","actor":{"agent_id":"impl-code-reasons","role":"implementer","session_id":"s-0-36-code-reasons"}}

  # canon: {"schema":1,"at":"2026-11-04T11:58:07Z","actor":{"agent_id":"impl-code-reasons","role":"implementer","session_id":"s-0-36-code-reasons"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-code-reasons.F1
  @atip.reasons.01
  Scenario: A reason is registered under its code with a name, a meaning and a fix of its own, and a reason its code does not register is refused
    Given the registry of error codes and a debug build that answers a reason only when its code registers it
    When every code's reasons are read, and an error is answered with a reason its code does not register, and with a reason of another code
    Then each reason is upper-case words, unique inside its code, in the order of the names and has a meaning and a fix of its own, and each of the two answers is refused

  # canon: {"schema":1,"at":"2026-11-04T11:58:22Z","actor":{"agent_id":"impl-code-reasons","role":"implementer","session_id":"s-0-36-code-reasons"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-code-reasons.F1
  @atip.reasons.02
  Scenario: Every reason a source raises is registered under its code, and every registered reason is raised by a site
    Given the sources that raise errors and the registry of reasons
    When the reasons the sources name are matched against the registry, and the registry against the sources
    Then every reason a source names is registered under some code, and a registered reason that no site raises fails the test

  # canon: {"schema":1,"at":"2026-11-04T11:58:29Z","actor":{"agent_id":"impl-code-reasons","role":"implementer","session_id":"s-0-36-code-reasons"}}
  @subject:atip-contract
  @case:edge
  @flow:canon-0-36-code-reasons.F1
  @atip.reasons.03
  Scenario: The same reason name may stand under two codes, and the names the contract resolved by namespace keep their names
    Given the reasons of every code
    When the owners of BLOCKERS, UNREADABLE, EXPIRED and STALE are listed
    Then the same name stands under two codes with a fix of its own each, and EXPIRED, STALE and UNREADABLE keep the names the contract resolved by namespace

  # canon: {"schema":1,"at":"2026-11-04T11:58:38Z","actor":{"agent_id":"impl-code-reasons","role":"implementer","session_id":"s-0-36-code-reasons"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-code-reasons.F2
  @atip.reasons.04
  Scenario: The audit table lists the reasons the registry holds for each code, and it fails on a code with several next actions and no reasons or one next action under two reasons
    Given the audit table of the change, with the situations of each code, the reason that names each and the next action of the caller
    When the table is compared with the registry and with `canon schema --error <CODE>`, and given a table with a code of several next actions and no reasons, and one with one next action under two reasons
    Then the table and the registry list the same reasons for every code, the schema lists them with a fix for each, and the two defective tables are refused

  # canon: {"schema":1,"at":"2026-11-04T11:58:44Z","actor":{"agent_id":"impl-code-reasons","role":"implementer","session_id":"s-0-36-code-reasons"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-code-reasons.F3
  @atip.reasons.05
  Scenario: A wrong specs section, a wrong policy and a wrong canon.yaml are CONFIG_INVALID with three different reasons, each the same every time
    Given a repository whose canon.yaml does not parse, one whose specs section is wrong, one whose skills section is wrong, one whose artifact_store section is wrong and one whose policy file is wrong
    When a command that reads each runs twice
    Then each answers CONFIG_INVALID with its own reason (CANON_YAML, SPECS, SKILLS, ARTIFACT_STORE, POLICY), the same reason both times, exit 2 and not retryable

  # canon: {"schema":1,"at":"2026-11-04T11:58:50Z","actor":{"agent_id":"impl-code-reasons","role":"implementer","session_id":"s-0-36-code-reasons"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-code-reasons.F3
  @atip.reasons.06
  Scenario: A brief.md that cannot be read is BRIEF_INVALID with the reason UNREADABLE and a brief with blockers is BRIEF_INVALID with BLOCKERS
    Given a repository whose brief has blocker findings, one whose brief.md is not UTF-8 and one whose brief.md is a directory
    When brief approve, brief show and brief check run
    Then each answers BRIEF_INVALID with BLOCKERS or UNREADABLE as the cause says, exit 2, and the registry row's retryable

  # canon: {"schema":1,"at":"2026-11-04T11:58:57Z","actor":{"agent_id":"impl-code-reasons","role":"implementer","session_id":"s-0-36-code-reasons"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-code-reasons.F4
  @atip.reasons.07
  Scenario: A transition the gate refuses names the kind that asks for a different fix, and every entry of details has its own code and a registered reason
    Given a subject with an open blocker finding, then one at verifying with a refuted audit and a new blocker, and the 11 violation kinds of the gate
    When the subject moves to verifying and to shipped, and the kinds are mapped to their codes and reasons
    Then the first move is refused with SUBJECT_INVALID and the reason OPEN_BLOCKER, the ship with FALSE_SUCCESS and OPEN_BLOCKER as the entries of details, no data, every entry has its own registered code and reason (a debug build checks each entry), and only the kinds that ask for a different fix under one code are reasons

  # canon: {"schema":1,"at":"2026-11-04T11:59:03Z","actor":{"agent_id":"impl-code-reasons","role":"implementer","session_id":"s-0-36-code-reasons"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-code-reasons.F5
  @atip.reasons.08
  Scenario: A reason never changes the exit or the retryable value of its code, and a different value is a different code
    Given every code with reasons and the pairs of codes that differ in retryable or exit
    When an error is built for each code and each of its reasons
    Then its exit and retryable are the row's, no source sets either beside the row, ARTIFACTS_STORE_UNAVAILABLE and ARTIFACTS_STORE_REQUIRED are two codes and PROMOTION_INVALID and PROMOTION_FAILED differ in exit

  # canon: {"schema":1,"at":"2026-11-04T11:59:09Z","actor":{"agent_id":"impl-code-reasons","role":"implementer","session_id":"s-0-36-code-reasons"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-code-reasons.F6
  @atip.reasons.09
  Scenario: The CHANGELOG table of reasons is the registry and STOPPED_MIDWAY and its reason are gone
    Given the CHANGELOG table of reasons and the registry
    When the table is compared with the registry
    Then each code of the table has the reasons the registry lists, the codes of the brief's table are in it, and neither STOPPED_MIDWAY nor its reason SKILLS_INVALID_ARGUMENT is registered or printed

  # canon: {"schema":1,"at":"2026-11-04T11:59:15Z","actor":{"agent_id":"impl-code-reasons","role":"implementer","session_id":"s-0-36-code-reasons"}}
  @subject:atip-contract
  @case:edge
  @flow:canon-0-36-code-reasons.F5
  @atip.reasons.10
  Scenario: The kind of an I/O error is read before its raw number, so an access-denied error with raw number 5 is not an EIO
    Given an I/O error of the kind PermissionDenied with raw number 5, as Windows reports ERROR_ACCESS_DENIED, and one of no kind with raw number 5 on a unix system
    When read_cause classifies each
    Then the first is a refused read, the second an EIO a repeat can pass, and the raw number is read only where 5 is EIO

  # canon: {"schema":1,"at":"2026-11-04T11:59:21Z","actor":{"agent_id":"impl-code-reasons","role":"implementer","session_id":"s-0-36-code-reasons"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-code-reasons.F5
  @atip.reasons.11
  Scenario: A read the system fails for a repeat to pass before any work began is the subject's UNAVAILABLE code, for canon query and for a migrate
    Given a repository with a ledger record and a debug-build seam that makes reads of matching paths fail with EIO
    When canon query reads the ledger, and canon migrate reads canon.yaml and the files of its plan with the seam on
    Then query answers REPO_UNAVAILABLE (exit 2, retryable) and not QUERY_FAILED, migrate answers CONFIG_UNAVAILABLE and MIGRATION_UNAVAILABLE by the file, and a canon.yaml that is not there is still CONFIG_REQUIRED

  # canon: {"schema":1,"at":"2026-11-04T11:59:27Z","actor":{"agent_id":"impl-code-reasons","role":"implementer","session_id":"s-0-36-code-reasons"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-code-reasons.F5
  @atip.reasons.12
  Scenario: Every call site that reads answers its subject's UNAVAILABLE code for an EIO, shown through the real binary with the debug seam
    Given a repository with a brief, a scenario, evidence with an artifact, a dispatch run, a strategy and a ledger in the old layout, and the seam
    When each command that reads a file or a directory runs with its path matched by the seam, and each reader of the plan corpus (dispatch begin, evidence add, evidence approval-payload, task done, ingest plans) runs with a canon.yaml that is refused, a directory or not UTF-8
    Then the readers of the plan corpus answer PLAN_CORPUS_UNAVAILABLE, CONFIG_DENIED or CONFIG_INVALID by the cause, and each its subject's UNAVAILABLE code (exit 2, retryable, no data, the system's error in the message), the code is in canon schema for the command, and every UNAVAILABLE code of the registry is driven or is no read

  # canon: {"schema":1,"at":"2026-11-04T11:59:34Z","actor":{"agent_id":"impl-code-reasons","role":"implementer","session_id":"s-0-36-code-reasons"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-code-reasons.F2
  @atip.reasons.13
  Scenario: Every code a command can emit is in canon schema for that command, TASK_NOT_FOUND and PLAN_SOURCES_REQUIRED of evidence add and approval-payload included
    Given a repository with no task and one with no plan source
    When evidence add and evidence approval-payload run with --task
    Then they answer TASK_NOT_FOUND and PLAN_SOURCES_REQUIRED, and canon schema lists both codes and the PLAN_CORPUS codes for both commands

  # canon: {"schema":1,"at":"2026-11-04T11:59:40Z","actor":{"agent_id":"impl-code-reasons","role":"implementer","session_id":"s-0-36-code-reasons"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-code-reasons.F5
  @atip.reasons.14
  Scenario: A Postgres DSN that is set and cannot be reached is RETENTION_TIERS_UNAVAILABLE, an unset one RETENTION_TIERS_REQUIRED
    Given a canon.yaml whose hot tier is Postgres, and a DSN that is unset, then empty, then malformed, then one that names a host that does not resolve
    When export, purge, query and tier age run
    Then the unset DSN is RETENTION_TIERS_REQUIRED and not retryable, the unreachable one is RETENTION_TIERS_UNAVAILABLE and retryable (REPO_UNAVAILABLE for query, TIER_UNAVAILABLE for tier age, the message saying the tier did not answer), and an empty or malformed DSN is RETENTION_TIERS_INVALID (CONFIG_INVALID with reason TIERS for query and tier age) and not retryable, with no data and nothing written
