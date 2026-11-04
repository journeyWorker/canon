Feature: atip: the INCOMPLETE codes of a command that stopped after its earlier steps wrote
  # canon: {"schema":1,"at":"2026-11-02T09:39:12Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}

  # canon: {"schema":1,"at":"2026-11-02T09:39:28Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-canon-stopped-midway.F1
  @atip.midway.01
  Scenario: The reserved midway code is gone, each INCOMPLETE code is registered at exit 1 with its retryability and reasons, and both schema views agree
    Given canon's error registry
    When canon schema --errors --json and canon schema --error <code> --json are read
    Then the reserved midway code is neither registered nor listed and is no code to look up; each INCOMPLETE code of the table is one row with exit 1, its retryability (retryable only for ARTIFACTS_STORE_INCOMPLETE) and its reasons, which for a code that says a refusal are the names of registered exit-2 codes with a meaning and a fix, sorted

  # canon: {"schema":1,"at":"2026-11-02T09:39:56Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-canon-stopped-midway.F1
  @atip.midway.02
  Scenario: A registry that registers the reserved code again, or breaks the row of an INCOMPLETE code, fails the registry check
    Given the registry with a reason that names no code, an exit-1 code or unsorted names, with a row at exit 2 or with another retryability, with the reserved code registered again and with an INCOMPLETE code the table lacks
    When the registry check runs and an error that names a reason the registry does not hold under its code is rendered
    Then each fault is named and the check fails, and rendering an unregistered reason, an exit-1 code as a reason or the reason of another code fails

  # canon: {"schema":1,"at":"2026-11-02T09:40:02Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}
  @subject:atip-contract
  @case:happy
  @flow:canon-0-35-canon-stopped-midway.F1
  @atip.midway.03
  Scenario: canon init stops with INIT_INCOMPLETE and the reason SKILLS_SOURCE_NOT_FOUND when the skill install meets a refusal after its writes
    Given a fresh repository holding .pi and CANON_SKILLS_SOURCE naming a directory that does not exist
    When canon init --json runs
    Then it exits 1 with INIT_INCOMPLETE, reason SKILLS_SOURCE_NOT_FOUND, not retryable, and canon.yaml, the policy and AGENTS.md stay written

  # canon: {"schema":1,"at":"2026-11-02T09:40:07Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-canon-stopped-midway.F1
  @atip.midway.04
  Scenario: A skill directory that cannot be written after the writes is WRITE_FAILED and keeps its name
    Given a fresh repository holding a read-only .pi directory and a valid skill source
    When canon init --json runs
    Then it exits 1 with WRITE_FAILED and no reason, canon.yaml stays written and meta.next leads with canon status

  # canon: {"schema":1,"at":"2026-11-02T09:40:13Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-canon-stopped-midway.F1
  @atip.midway.05
  Scenario: A skill install failure that is not a refusal keeps SKILLS_FAILED and no reason
    Given a fresh repository whose .pi/skills is a symlink and a valid skill source
    When canon init --json runs
    Then it exits 1 with SKILLS_FAILED and no reason, canon.yaml stays written and meta.next leads with canon status

  # canon: {"schema":1,"at":"2026-11-02T09:40:18Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-canon-stopped-midway.F1
  @atip.midway.06
  Scenario: A refusal with no effect is exit 2 with the refusal's own code, never INIT_INCOMPLETE
    Given a repository holding .pi and a skill source that does not exist
    When canon init --dry-run --json runs, and canon init --providers with an unknown harness runs
    Then the first exits 2 with SKILLS_SOURCE_NOT_FOUND and the second with INIT_ARGUMENT_INVALID, neither is INIT_INCOMPLETE and nothing is written

  # canon: {"schema":1,"at":"2026-11-02T09:40:24Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-canon-stopped-midway.F2
  @atip.midway.07
  Scenario: The first meta.next step of a stopped command reads what was done and the repeat comes after it
    Given the failure envelope of the stopped canon init, and envelopes with no next, an empty next, a first step that repeats the command and a first step that writes
    When the first step is judged and run
    Then it is canon status --json, a read command that runs, the repeat of the install comes second, and each bad envelope fails the check

  # canon: {"schema":1,"at":"2026-11-02T09:40:30Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-canon-stopped-midway.F1
  @atip.midway.08
  Scenario: A batch element that stops part-way is failed with its INCOMPLETE code and its reason, the elements after it are skipped and the batch is BATCH_INCOMPLETE
    Given a batch of two canon init elements in a fresh repository whose skill source does not exist, and a child failure envelope carrying INIT_INCOMPLETE and a reason
    When the batch runs and the child's error becomes the item error
    Then the first element is failed with INIT_INCOMPLETE and the reason SKILLS_SOURCE_NOT_FOUND in its item, the second is skipped, the batch is BATCH_INCOMPLETE with exit 1 and what the first element wrote stays

  # canon: {"schema":1,"at":"2026-11-02T09:40:37Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}
  @subject:evidence-attestation
  @case:happy
  @flow:canon-0-35-canon-stopped-midway.F3
  @atip.midway.09
  Scenario: The audit lists every exit-1 code in its class and each code exists where the table says
    Given the registry, the sources and the audit table of the exit-1 codes
    When each code is classed INCOMPLETE, FAILED, answer, internal or batch and checked against the registry and the sources
    Then every exit-1 code has a line, each audited code is registered and built by a source, each INCOMPLETE code ends in _INCOMPLETE and no other code does, and the codes this change replaced are in neither the registry nor a source

  # canon: {"schema":1,"at":"2026-11-02T09:40:43Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}
  @subject:tier-storage
  @case:happy
  @flow:canon-0-35-canon-stopped-midway.F4
  @atip.midway.10
  Scenario: The failure codes of the work keep their names, exits and retryability
    Given the failure codes of the work: WRITE_FAILED, ARTIFACTS_STORE_FAILED, PROMOTION_FAILED, TIER_FAILED and the other *_FAILED codes that write
    When the registry is read
    Then each keeps its name, exit 1 and retryability, has no reason, and is no reason of an INCOMPLETE code

  # canon: {"schema":1,"at":"2026-11-02T09:40:49Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}
  @subject:atip-contract
  @case:failure
  @flow:canon-0-35-canon-stopped-midway.F5
  @atip.midway.11
  Scenario: No consumer spells a code that moved to the reserved midway code or was replaced by an INCOMPLETE code, and the check catches one that comes back
    Given the guides, the website, the dashboard, the eval scripts, the design documents, the scenarios outside area=atip, the sources, the conformance cases and the generated spec
    When they are scanned for INIT_SKILL_INSTALL_FAILED, the reserved midway code and the names the INCOMPLETE codes replaced
    Then none spells one, and a text that spells one, or a longer name that contains one, is judged as the check says

  # canon: {"schema":1,"at":"2026-11-02T09:40:57Z","actor":{"agent_id":"impl-stopped-midway","role":"implementer","session_id":"s-0-35-stopped"}}
  @subject:subject-lifecycle
  @case:failure
  @flow:canon-0-35-canon-stopped-midway.F3
  @atip.midway.12
  Scenario: A half-written adoption keeps CHANGE_ADOPTION_INCOMPLETE and its repair completes the link
    Given a change store that cannot be written after the subject record was written
    When canon subject adopt runs, then its repair runs
    Then the first exits 1 with CHANGE_ADOPTION_INCOMPLETE, no reason, naming the repair, and the repair completes the link and a third run writes nothing

  # canon: {"schema":1,"at":"2026-11-03T04:59:34Z","actor":{"agent_id":"impl-flow-failures","role":"implementer","session_id":"s-0-35-flow-failures"}}
  @subject:tier-storage
  @case:failure
  @flow:canon-0-35-canon-stopped-midway.F4
  @atip.midway.13
  Scenario: A failure code of the work that is made a reason of an INCOMPLETE code, given a reason, a refusal's exit or another retryability fails the check of the failure codes
    Given the registry with a failure code removed, moved to exit 2, made retryable or not, given a reason, or named as a reason of an INCOMPLETE code
    When the check of the failure codes runs on each
    Then each fault is named with its own message, and the registry as it stands passes

  # canon: {"schema":1,"at":"2026-11-03T18:11:39Z","actor":{"agent_id":"impl-shipfix","role":"implementer","session_id":"s-0-35-shipfix"}}
  @subject:evidence-attestation
  @case:failure
  @flow:canon-0-35-canon-stopped-midway.F3
  @atip.midway.14
  Scenario: An exit-1 code the audit table lacks, an audited code no source builds, a replaced code that returns and a class that disagrees with the name each fail the audit check
    Given the audit table of the exit-1 codes with a code the registry holds but the table lacks, a source text no source builds an audited code in, a replaced code put back in the registry or in a source, an INCOMPLETE code classed as another class and another code named INCOMPLETE
    When the audit check runs on each
    Then each fault is named with its own message (not in the audit, no source builds it, still in the registry, a source still builds it, not classed INCOMPLETE, name does not end in _INCOMPLETE), and the audit as it stands passes
