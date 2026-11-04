Feature: atip codesplit
  # canon: {"schema":1,"at":"2026-11-04T04:05:44Z","actor":{"agent_id":"impl-codes-refusals","role":"implementer","session_id":"s-0-36-codes-refusals"}}

  # canon: {"schema":1,"at":"2026-11-04T04:05:44Z","actor":{"agent_id":"impl-codes-refusals","role":"implementer","session_id":"s-0-36-codes-refusals"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-refusals.F1
  @atip.codesplit.01
  Scenario: A call that gives no task or scenario, two arguments that contradict each other and an empty value each answer with their own condition
    Given an empty directory
    When evidence add runs with neither --task nor --scenario, with --all-scenarios beside --scenario, and with an empty --ref, and evidence add and evidence approval-payload run with a --task that names no task
    Then the three exit 2 with EVIDENCE_REQUIRED, EVIDENCE_ARGUMENT_CONFLICT and EVIDENCE_ARGUMENT_INVALID, none carries data, and nothing is written; the missing task is TASK_NOT_FOUND from both commands, with the hint of the task, a --surface-ref that gets past the argument validator is EVIDENCE_ARGUMENT_INVALID naming it, and EVIDENCE_INVALID is no code

  # canon: {"schema":1,"at":"2026-11-04T04:06:32Z","actor":{"agent_id":"impl-codes-refusals","role":"implementer","session_id":"s-0-36-codes-refusals"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-refusals.F1
  @atip.codesplit.02
  Scenario: A file to bind that changed, lies outside the repository, is missing or is a directory is refused by its condition and writes nothing
    Given a repository with an indexed scenario, a bound file that changed after it was bound, one outside the repository, one that does not exist and one that is a directory
    When evidence add and review scenario bind each of them
    Then the outside one is DENIED, the missing one NOT_FOUND and the directory INVALID, each exits 2 with no data, and no record or blob is written; a file that changed while it was read is EVIDENCE_BINDING_STALE (REVIEW_ARTIFACT_STALE), exit 2 and not retryable, shown where the store reports it

  # canon: {"schema":1,"at":"2026-11-04T04:06:38Z","actor":{"agent_id":"impl-codes-refusals","role":"implementer","session_id":"s-0-36-codes-refusals"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-refusals.F2
  @atip.codesplit.03
  Scenario: Ending a run that was already ended is DISPATCH_CONFLICT with the reason ALREADY_ENDED and the manifest keeps its bytes
    Given a repository with a dispatch run that was begun and ended
    When dispatch end runs for the same run again
    Then it exits 2 with DISPATCH_CONFLICT and the reason ALREADY_ENDED, and the manifest keeps its bytes; a run whose recorded status is not running gets the reason NOT_RUNNING

  # canon: {"schema":1,"at":"2026-11-04T04:06:47Z","actor":{"agent_id":"impl-codes-refusals","role":"implementer","session_id":"s-0-36-codes-refusals"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-refusals.F3
  @atip.codesplit.04
  Scenario: A broken section of canon.yaml and a bad provider answer with CONFIG_INVALID and its reason, or with the argument's own code
    Given a repository whose canon.yaml has a malformed skills section and one whose specs section is malformed
    When skills install, inventory sync and plugin sync run, and skills install --providers bogus runs
    Then the malformed sections exit 2 with CONFIG_INVALID and the reason SKILLS or SPECS, the unknown provider with SKILLS_ARGUMENT_INVALID, and nothing is written; with the section mended plugin sync goes on to its own check and is not refused for a scenario index another process holds

  # canon: {"schema":1,"at":"2026-11-04T04:06:52Z","actor":{"agent_id":"impl-codes-refusals","role":"implementer","session_id":"s-0-36-codes-refusals"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-refusals.F4
  @atip.codesplit.05
  Scenario: A repository with no brief, a change with no brief and a scenario the index does not hold each answer with their own code, and the fix lets the same call through
    Given a repository with no change that has a brief, a change with no brief, and a scenario the index does not hold
    When brief show, brief show with the change name and evidence approval-payload for the scenario run, then canon change new, canon scenario new and canon inventory sync run
    Then they exit 2 with BRIEF_REQUIRED, BRIEF_NOT_FOUND and SCENARIO_INDEX_REQUIRED, and after the fix the same calls no longer refuse

  # canon: {"schema":1,"at":"2026-11-04T09:36:05Z","actor":{"agent_id":"impl-refusals-fix","role":"implementer","session_id":"s-0-36-refusals-fix"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-refusals.F3
  @atip.codesplit.06
  Scenario: A rollback the signature check turns away says which condition failed, and only a signature that does not verify is STRATEGY_SIGNATURE_INVALID
    Given a quarantined strategy and a policy whose allowed_signers file lists the actor alice
    When strategy rollback runs with a blank reason, with an actor the file does not list, with an empty signature file, with a signature that does not verify, with an allowed_signers file that is unreadable or missing, with a policy that does not parse and with a policy that names no verifier
    Then they exit 2 with STRATEGY_ARGUMENT_INVALID naming --reason, STRATEGY_SIGNER_DENIED, STRATEGY_SIGNATURE_REQUIRED, STRATEGY_SIGNATURE_INVALID, CONFIG_DENIED or CONFIG_INVALID with the reason POLICY, CONFIG_INVALID with the reason POLICY and STRATEGY_VERIFIER_REQUIRED, none carries data, and nothing is recorded

  # canon: {"schema":1,"at":"2026-11-04T09:36:37Z","actor":{"agent_id":"impl-refusals-fix","role":"implementer","session_id":"s-0-36-refusals-fix"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-refusals.F3
  @atip.codesplit.07
  Scenario: A file or lock the system refuses is told by the code of its own subject
    Given an adapter response and a hook file this user may not read, a policy that is a directory, a canon.yaml the system refuses or that does not parse, an artifact tier that is not an object store, a scenario index another process holds and a migrate preview whose temp directory cannot be written
    When adapter validate, gate install-hooks, policy approve, export, purge, artifacts move, inventory sync and migrate run
    Then they exit 2 with ADAPTER_FILE_DENIED, HOOKS_FILE_DENIED, POLICY_INVALID, RETENTION_TIERS_DENIED or RETENTION_TIERS_INVALID, ARTIFACTS_TIER_INVALID, INVENTORY_INDEX_UNAVAILABLE (retryable, the hint says to wait) and MIGRATION_DENIED, nothing is written, and plugin sync, which takes no index lock, is not refused for a lock another process holds

  # canon: {"schema":1,"at":"2026-11-04T19:21:05Z","actor":{"agent_id":"impl-flowfix-036","role":"implementer","session_id":"s-0-36-flowfix-3"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-refusals.F5
  @atip.codesplit.08
  Scenario: A batch approval whose ids are partly refused answers with the first refused id's own code and reason, one details entry for each refused id and no data, and writes nothing
    Given three changes, one with a clean brief, one with the skeleton's blockers and one with a brief that ends inside an open code fence, and an id that has no change
    When brief approve runs with the clean id, the blocked and the unclosed ones, then with the missing id first, then with the blocked one alone
    Then the answer is exit 2 with the code and reason of the first refused id (BRIEF_INVALID with BLOCKERS, or BRIEF_NOT_FOUND), two details in the order given (BLOCKERS then UNAPPROVABLE), no data and no data.refused, no brief changes, not even the one that could be approved, and one refused id has no details

  # canon: {"schema":1,"at":"2026-11-04T19:21:10Z","actor":{"agent_id":"impl-flowfix-036","role":"implementer","session_id":"s-0-36-flowfix-3"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-refusals.F5
  @atip.codesplit.09
  Scenario: The refused ids fixed one at a time let the same call approve them all with one entry each and no refusal
    Given the three changes of the refused approval
    When the blocked brief is fixed and the call runs again, then the unclosed one is fixed and the call runs again
    Then after the first fix the first refused id is the unclosed brief with its own reason and no details, and after the second all three are written in the order given, each carries the approval, and the answer is data without a refusal and without data.refused

  # canon: {"schema":1,"at":"2026-11-04T19:21:15Z","actor":{"agent_id":"impl-flowfix-036","role":"implementer","session_id":"s-0-36-flowfix-3"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-36-codes-refusals.F6
  @atip.codesplit.10
  Scenario: The CHANGELOG has a row for each refusal code the change split that says what it became, the registry has those codes with the row's exit and retryability, and the old names are in no registry view
    Given the 28 codes of the change's table, the replacement table of the CHANGELOG, the registry, the errors of every command document and the guides
    When each row is read against the registry, canon schema --errors and canon schema --error <old name> run, and the guides are scanned for the old names
    Then every code has one row naming the new codes of the brief's table, each new code is registered with the row's exit and retryability, each reason of a row is registered under one of its new codes, the old names that no new code keeps are in no registry view and no errors list and are refused by schema --error as USAGE at --error, the BRIEF_APPROVE_REFUSED row says where the refusals of a batch are, and no guide spells an old name

  # canon: {"schema":1,"at":"2026-11-04T19:21:19Z","actor":{"agent_id":"impl-flowfix-036","role":"implementer","session_id":"s-0-36-flowfix-3"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-36-codes-refusals.F6
  @atip.codesplit.11
  Scenario: A row left out, a new code dropped or misspelled, an exit that is not the registry's, a reason no new code registers or one the row leaves out, and an old name brought back into a guide are each found
    Given the CHANGELOG's replacement table with one row left out, one new code dropped, one misspelled, one exit changed, one reason misspelled and one reason removed, and guide text that says an old name
    When each changed table is checked against the registry and the guide text is scanned for the old names
    Then the real table has no problem, each changed table has a problem that names its row and what is wrong, and the guide text that says MIGRATE_REFUSED or OLD_LAYOUT is named while one that says only names that stay is not
